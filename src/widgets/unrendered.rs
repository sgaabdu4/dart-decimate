use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use tree_sitter::Node;

use crate::graph::normalize_against;
use crate::{DartCombinatorKind, DependencyKind, ScannedProject, TopLevelDeclaration};

#[cfg(test)]
use super::parse_tree;
use super::resolution::{ClassKey, DeclarationResolver};
use super::{
    UnrenderedWidgetClass, WidgetFileFacts, has_ancestor_kind, inheritance::class_superclasses,
    simple_type_name, visit_named, widget_kind,
};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct FileReachabilityFacts {
    pub(super) path: PathBuf,
    pub(super) class_names: BTreeSet<String>,
    pub(super) widgets: Vec<UnrenderedWidgetClass>,
    pub(super) object_constructors: Vec<String>,
    contextual_constructors: Vec<ContextualConstructor>,
    typed_fields: Vec<TypedField>,
    pub(super) superclasses: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ContextualConstructor {
    NamedArgument {
        owner_constructor: String,
        argument: String,
    },
    TypedInitializer {
        target: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TypedField {
    owner: String,
    field: String,
    target: String,
}

pub(super) fn unrendered_widgets(
    project: &ScannedProject,
    files: &[WidgetFileFacts],
    resolver: &DeclarationResolver,
) -> Vec<UnrenderedWidgetClass> {
    let files = files
        .iter()
        .map(|file| &file.reachability)
        .collect::<Vec<_>>();
    let exported = public_reexported_declarations(project);
    let mut candidates = files
        .iter()
        .flat_map(|file| file.widgets.iter().cloned())
        .filter(|widget| !exported.contains(&(widget.path.clone(), widget.widget_class.clone())))
        .collect::<Vec<_>>();
    let candidate_keys = candidates
        .iter()
        .map(|widget| ClassKey {
            path: widget.path.clone(),
            name: widget.widget_class.clone(),
        })
        .collect::<BTreeSet<_>>();
    let render_counts = render_counts(&files, &candidate_keys, resolver);

    candidates.retain_mut(|widget| {
        widget.render_reference_count = render_counts
            .get(&ClassKey {
                path: widget.path.clone(),
                name: widget.widget_class.clone(),
            })
            .copied()
            .unwrap_or_default();
        widget.render_reference_count == 0
    });
    candidates.sort_by(|left, right| {
        (
            &left.path,
            left.location.line,
            left.location.column,
            &left.widget_class,
        )
            .cmp(&(
                &right.path,
                right.location.line,
                right.location.column,
                &right.widget_class,
            ))
    });
    candidates
}

pub(super) fn reachability_facts(
    path: &Path,
    root: Node<'_>,
    classes: &[Node<'_>],
    source: &str,
) -> FileReachabilityFacts {
    FileReachabilityFacts {
        path: path.to_path_buf(),
        class_names: classes
            .iter()
            .filter_map(|class| {
                class
                    .child_by_field_name("name")?
                    .utf8_text(source.as_bytes())
                    .ok()
                    .map(str::to_owned)
            })
            .collect(),
        widgets: widget_classes(path, classes, source),
        object_constructors: object_constructor_names(root, source),
        contextual_constructors: contextual_constructor_names(root, source),
        typed_fields: typed_fields(classes, source),
        superclasses: class_superclasses(classes, source),
    }
}

fn widget_classes(path: &Path, classes: &[Node<'_>], source: &str) -> Vec<UnrenderedWidgetClass> {
    classes
        .iter()
        .filter_map(|class| widget_class(path, *class, source))
        .collect()
}

fn widget_class(path: &Path, class: Node<'_>, source: &str) -> Option<UnrenderedWidgetClass> {
    if is_abstract_class(class, source) {
        return None;
    }
    let widget_kind = widget_kind(class, source)?;
    let name_node = class.child_by_field_name("name")?;
    let widget_class = name_node.utf8_text(source.as_bytes()).ok()?.to_owned();
    Some(UnrenderedWidgetClass {
        path: path.to_path_buf(),
        widget_class,
        widget_kind,
        location: name_node.start_position().into(),
        render_reference_count: 0,
    })
}

fn is_abstract_class(class: Node<'_>, source: &str) -> bool {
    let Some(name) = class.child_by_field_name("name") else {
        return false;
    };
    source
        .get(class.start_byte()..name.start_byte())
        .unwrap_or_default()
        .split_whitespace()
        .any(|token| token == "abstract")
}

fn object_constructor_names(root: Node<'_>, source: &str) -> Vec<String> {
    let mut constructors = Vec::new();
    visit_named(root, &mut |node| {
        if is_object_constructor(node) && !has_ancestor_kind(node, &["annotation"]) {
            if let Some(constructor) = constructor_type_name(node, source)
                && !constructor_name_candidates(&constructor).is_empty()
            {
                constructors.push(constructor);
            }
        } else if is_arrow_body_constructor_identifier(node, source)
            && let Ok(constructor) = node.utf8_text(source.as_bytes())
        {
            constructors.push(constructor.to_owned());
        }
    });
    constructors
}

fn contextual_constructor_names(root: Node<'_>, source: &str) -> Vec<ContextualConstructor> {
    let mut constructors = Vec::new();
    visit_named(root, &mut |node| {
        if node.kind() == "named_argument" && contains_dot_new(node, source) {
            let Some(argument) = node.named_child(0).and_then(|label| {
                label
                    .utf8_text(source.as_bytes())
                    .ok()
                    .map(|name| name.trim_end_matches(':').to_owned())
            }) else {
                return;
            };
            let Some(call) = node.parent().and_then(|arguments| arguments.parent()) else {
                return;
            };
            if let Some(owner_constructor) = constructor_type_name(call, source) {
                constructors.push(ContextualConstructor::NamedArgument {
                    owner_constructor,
                    argument,
                });
            }
            return;
        }

        if node.kind() == "local_variable_declaration" {
            let mut cursor = node.walk();
            for definition in node
                .named_children(&mut cursor)
                .filter(|child| child.kind() == "initialized_variable_definition")
            {
                if let Some(target) = contextual_initialized_variable_target(definition, source) {
                    constructors.push(ContextualConstructor::TypedInitializer { target });
                }
            }
            return;
        }

        if node.kind() == "declaration" {
            constructors.extend(
                contextual_declaration_targets(node, source)
                    .map(|target| ContextualConstructor::TypedInitializer { target }),
            );
        }
    });
    constructors
}

fn contextual_initialized_variable_target(definition: Node<'_>, source: &str) -> Option<String> {
    let type_node = direct_named_child(definition, "type")?;
    let value = definition.child_by_field_name("value")?;
    contextual_initializer_target(type_node, value, source)
}

fn contextual_declaration_targets<'tree>(
    declaration: Node<'tree>,
    source: &'tree str,
) -> impl Iterator<Item = String> + 'tree {
    let Some(type_node) = direct_named_child(declaration, "type") else {
        return Vec::new().into_iter();
    };
    let Some(target) = type_node
        .utf8_text(source.as_bytes())
        .ok()
        .and_then(contextual_target_type)
    else {
        return Vec::new().into_iter();
    };
    let mut cursor = declaration.walk();
    declaration
        .named_children(&mut cursor)
        .filter(|child| {
            matches!(
                child.kind(),
                "identifier_list" | "initialized_identifier_list"
            )
        })
        .flat_map(|list| initialized_identifiers(list))
        .filter(move |identifier| {
            identifier
                .child_by_field_name("value")
                .is_some_and(|value| is_contextual_initializer(value, source))
        })
        .map(move |_| target.clone())
        .collect::<Vec<_>>()
        .into_iter()
}

fn contextual_initializer_target(
    type_node: Node<'_>,
    value: Node<'_>,
    source: &str,
) -> Option<String> {
    is_contextual_initializer(value, source).then_some(())?;
    type_node
        .utf8_text(source.as_bytes())
        .ok()
        .and_then(contextual_target_type)
}

fn is_contextual_initializer(value: Node<'_>, source: &str) -> bool {
    let Ok(value_text) = value.utf8_text(source.as_bytes()) else {
        return false;
    };
    let value_text = value_text.trim_start();
    let is_direct_constructor =
        value_text.starts_with(".new") || value_text.starts_with("const .new");
    let is_collection_literal = value_text.starts_with('[')
        || value_text.starts_with('{')
        || value_text.starts_with("const [")
        || value_text.starts_with("const {")
        || value_text.starts_with("List.unmodifiable(")
        || value_text.starts_with("List.of(")
        || value_text.starts_with("Set.of(");
    (is_direct_constructor || is_collection_literal) && contains_dot_new(value, source)
}

fn contains_dot_new(node: Node<'_>, source: &str) -> bool {
    if node.kind() == "static_member_shorthand"
        && node.utf8_text(source.as_bytes()).ok() == Some(".new")
    {
        return true;
    }
    if is_object_constructor(node) && constructor_type_name(node, source).as_deref() == Some("New_")
    {
        return true;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| contains_dot_new(child, source))
}

fn typed_fields(classes: &[Node<'_>], source: &str) -> Vec<TypedField> {
    let mut fields = Vec::new();
    for class in classes {
        let Some(owner) = class
            .child_by_field_name("name")
            .and_then(|name| name.utf8_text(source.as_bytes()).ok())
        else {
            continue;
        };
        let Some(body) = class.child_by_field_name("body") else {
            continue;
        };
        let mut cursor = body.walk();
        for member in body.named_children(&mut cursor) {
            let Some(declaration) = direct_named_child(member, "declaration") else {
                continue;
            };
            for (field, target) in typed_fields_in_declaration(declaration, source) {
                fields.push(TypedField {
                    owner: owner.to_owned(),
                    field,
                    target,
                });
            }
        }
    }
    fields
}

fn typed_fields_in_declaration(declaration: Node<'_>, source: &str) -> Vec<(String, String)> {
    let Some(type_node) = direct_named_child(declaration, "type") else {
        return Vec::new();
    };
    let Some(type_text) = type_node.utf8_text(source.as_bytes()).ok() else {
        return Vec::new();
    };
    let Some(target) = contextual_target_type(type_text) else {
        return Vec::new();
    };
    let mut cursor = declaration.walk();
    declaration
        .named_children(&mut cursor)
        .filter(|child| {
            matches!(
                child.kind(),
                "identifier_list" | "initialized_identifier_list"
            )
        })
        .flat_map(initialized_identifiers)
        .filter_map(|identifier| {
            identifier
                .child_by_field_name("name")
                .or_else(|| (identifier.kind() == "identifier").then_some(identifier))
                .and_then(|name| name.utf8_text(source.as_bytes()).ok())
                .map(|name| (name.to_owned(), target.clone()))
        })
        .collect()
}

fn direct_named_child<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == kind)
}

fn initialized_identifiers(list: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = list.walk();
    list.named_children(&mut cursor).collect()
}

fn contextual_target_type(type_text: &str) -> Option<String> {
    let mut target = type_text.trim().trim_end_matches('?').trim();
    while let Some(open) = target.find('<') {
        let close = target.rfind('>')?;
        let inner = target.get(open + 1..close)?.trim();
        if has_top_level_comma(inner) {
            return None;
        }
        target = inner.trim_end_matches('?').trim();
    }
    (!target.is_empty()
        && target
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$' | b'.')))
    .then(|| target.to_owned())
}

fn has_top_level_comma(text: &str) -> bool {
    let mut depth = 0usize;
    for byte in text.bytes() {
        match byte {
            b'<' => depth += 1,
            b'>' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

fn is_object_constructor(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "call_expression"
            | "constructor_invocation"
            | "const_object_expression"
            | "function_expression_invocation"
            | "new_expression"
    )
}

fn constructor_type_name(node: Node<'_>, source: &str) -> Option<String> {
    if let Some(constructor) = function_expression_body_constructor_name(node, source) {
        return Some(constructor);
    }

    let arguments = node.child_by_field_name("arguments")?;
    let prefix = source.get(node.start_byte()..arguments.start_byte())?;
    let constructor = prefix
        .trim()
        .strip_prefix("const ")
        .or_else(|| prefix.trim().strip_prefix("new "))
        .unwrap_or(prefix.trim())
        .split('<')
        .next()
        .unwrap_or("")
        .replace(' ', "");
    (!constructor.is_empty()).then_some(constructor)
}

fn function_expression_body_constructor_name(node: Node<'_>, source: &str) -> Option<String> {
    if node.kind() != "call_expression" {
        return None;
    }
    let function = node.child_by_field_name("function")?;
    if function.kind() != "function_expression" {
        return None;
    }
    let body = function.child_by_field_name("body")?;
    let expression = body.named_child(0)?;
    constructor_call_type_name(expression, source)
        .or_else(|| split_closure_body_constructor_name(node, expression, source))
}

fn constructor_call_type_name(node: Node<'_>, source: &str) -> Option<String> {
    if !is_object_constructor(node) {
        return None;
    }
    let arguments = node.child_by_field_name("arguments")?;
    let prefix = source.get(node.start_byte()..arguments.start_byte())?;
    let constructor = prefix
        .trim()
        .strip_prefix("const ")
        .or_else(|| prefix.trim().strip_prefix("new "))
        .unwrap_or(prefix.trim())
        .split('<')
        .next()
        .unwrap_or("")
        .replace(' ', "");
    (!constructor_name_candidates(&constructor).is_empty()).then_some(constructor)
}

fn split_closure_body_constructor_name(
    call: Node<'_>,
    expression: Node<'_>,
    source: &str,
) -> Option<String> {
    call.child_by_field_name("arguments")?;
    if !expression_is_constructor_call_name(expression, source) {
        return None;
    }
    expression
        .utf8_text(source.as_bytes())
        .ok()
        .map(str::to_owned)
}

fn expression_is_constructor_call_name(expression: Node<'_>, source: &str) -> bool {
    if !matches!(expression.kind(), "identifier" | "type_identifier") {
        return false;
    }
    let Some(suffix) = source.get(expression.end_byte()..) else {
        return false;
    };
    let suffix = suffix.trim_start();
    if suffix.starts_with('(') {
        return true;
    }
    if suffix.starts_with('<')
        && let Some(after_type_arguments) =
            matching_enclosed_end(suffix, '<', '>').and_then(|end| suffix.get(end + 1..))
    {
        return after_type_arguments.trim_start().starts_with('(');
    }
    false
}

fn matching_enclosed_end(text: &str, open: char, close: char) -> Option<usize> {
    let mut chars = text.char_indices();
    if chars.next().is_none_or(|(_, character)| character != open) {
        return None;
    }
    let mut depth = 0usize;
    for (index, character) in text.char_indices() {
        if character == open {
            depth += 1;
        } else if character == close {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn is_arrow_body_constructor_identifier(node: Node<'_>, source: &str) -> bool {
    if !matches!(node.kind(), "identifier" | "type_identifier")
        || node
            .parent()
            .is_none_or(|parent| parent.kind() != "function_expression_body")
    {
        return false;
    }
    let Some(suffix) = source.get(node.end_byte()..) else {
        return false;
    };
    suffix.trim_start().starts_with("()")
}

fn constructor_name_candidates(constructor: &str) -> Vec<String> {
    constructor
        .split('.')
        .map(simple_type_name)
        .filter(|segment| {
            segment
                .chars()
                .next()
                .is_some_and(|first| first == '_' || first.is_ascii_uppercase())
        })
        .collect()
}

fn render_counts(
    files: &[&FileReachabilityFacts],
    candidate_keys: &BTreeSet<ClassKey>,
    resolver: &DeclarationResolver,
) -> BTreeMap<ClassKey, usize> {
    let superclasses = resolved_superclasses(files, resolver);
    let mut counts = BTreeMap::<ClassKey, usize>::new();
    let typed_fields = files
        .iter()
        .flat_map(|file| {
            file.typed_fields.iter().map(|field| {
                (
                    (
                        ClassKey {
                            path: file.path.clone(),
                            name: field.owner.clone(),
                        },
                        field.field.clone(),
                    ),
                    field.target.clone(),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    for file in files {
        for constructed in &file.object_constructors {
            increment_render_counts(
                resolver.resolve(&file.path, constructed),
                candidate_keys,
                &superclasses,
                &mut counts,
            );
        }
        for contextual in &file.contextual_constructors {
            match contextual {
                ContextualConstructor::NamedArgument {
                    owner_constructor,
                    argument,
                } => {
                    for owner in resolver.resolve(&file.path, owner_constructor) {
                        let Some(target) = typed_fields.get(&(owner.clone(), argument.clone()))
                        else {
                            continue;
                        };
                        increment_render_counts(
                            resolver.resolve(&owner.path, target),
                            candidate_keys,
                            &superclasses,
                            &mut counts,
                        );
                    }
                }
                ContextualConstructor::TypedInitializer { target } => increment_render_counts(
                    resolver.resolve(&file.path, target),
                    candidate_keys,
                    &superclasses,
                    &mut counts,
                ),
            }
        }
    }
    counts
}

fn increment_render_counts(
    classes: BTreeSet<ClassKey>,
    candidate_keys: &BTreeSet<ClassKey>,
    superclasses: &BTreeMap<ClassKey, BTreeSet<ClassKey>>,
    counts: &mut BTreeMap<ClassKey, usize>,
) {
    let mut pending = classes.into_iter().collect::<Vec<_>>();
    let mut visited = BTreeSet::new();
    while let Some(class) = pending.pop() {
        if !visited.insert(class.clone()) {
            continue;
        }
        if candidate_keys.contains(&class) {
            *counts.entry(class.clone()).or_default() += 1;
        }
        pending.extend(
            superclasses
                .get(&class)
                .into_iter()
                .flat_map(|parents| parents.iter().cloned()),
        );
    }
}

fn resolved_superclasses(
    files: &[&FileReachabilityFacts],
    resolver: &DeclarationResolver,
) -> BTreeMap<ClassKey, BTreeSet<ClassKey>> {
    let mut superclasses = BTreeMap::<ClassKey, BTreeSet<ClassKey>>::new();
    for file in files {
        for (class, parent) in &file.superclasses {
            let class = ClassKey {
                path: file.path.clone(),
                name: class.clone(),
            };
            superclasses
                .entry(class)
                .or_default()
                .extend(resolver.resolve(&file.path, parent));
        }
    }
    superclasses
}

fn public_reexported_declarations(project: &ScannedProject) -> BTreeSet<(PathBuf, String)> {
    let declarations = project
        .files
        .iter()
        .map(|file| {
            (
                normalize_against(&project.root, &file.path),
                file.declarations.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let export_edges = project
        .graph
        .dependencies()
        .into_iter()
        .filter(|edge| edge.kind == DependencyKind::Export)
        .collect::<Vec<_>>();
    let public_entries = project
        .files
        .iter()
        .map(|file| normalize_against(&project.root, &file.path))
        .filter(|path| is_public_library_entry(&project.root, path))
        .collect::<BTreeSet<_>>();
    let mut exported = BTreeSet::new();
    for entry in public_entries {
        collect_reexports(&declarations, &export_edges, &entry, &[], 0, &mut exported);
    }
    exported
}

fn collect_reexports(
    declarations: &BTreeMap<PathBuf, Vec<TopLevelDeclaration>>,
    export_edges: &[crate::ResolvedDependency],
    from_path: &Path,
    chain: &[crate::DependencyVisibility],
    depth: usize,
    exported: &mut BTreeSet<(PathBuf, String)>,
) {
    if depth > 8 {
        return;
    }

    for edge in export_edges
        .iter()
        .filter(|edge| edge.from_path == from_path)
    {
        let mut next_chain = chain.to_owned();
        next_chain.push(edge.visibility.clone());
        if let Some(declarations) = declarations.get(&edge.to_path) {
            exported.extend(
                declarations
                    .iter()
                    .filter(|declaration| is_visible(&declaration.name, &next_chain))
                    .map(|declaration| (edge.to_path.clone(), declaration.name.clone())),
            );
        }
        collect_reexports(
            declarations,
            export_edges,
            &edge.to_path,
            &next_chain,
            depth + 1,
            exported,
        );
    }
}

fn is_visible(name: &str, chain: &[crate::DependencyVisibility]) -> bool {
    chain
        .iter()
        .all(|visibility| is_visible_through_export(name, &visibility.combinators))
}

fn is_visible_through_export(name: &str, combinators: &[crate::DartCombinator]) -> bool {
    let mut visible = true;
    for combinator in combinators {
        match combinator.kind {
            DartCombinatorKind::Show => {
                visible = combinator.names.iter().any(|shown| shown == name);
            }
            DartCombinatorKind::Hide => {
                if combinator.names.iter().any(|hidden| hidden == name) {
                    visible = false;
                }
            }
        }
    }
    visible
}

fn is_public_library_entry(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root).is_ok_and(|relative| {
        let mut components = relative.components();
        components
            .next()
            .is_some_and(|component| component.as_os_str() == "lib")
            && components
                .next()
                .is_none_or(|component| component.as_os_str() != "src")
            && relative.extension().is_some_and(|ext| ext == "dart")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_name_candidates_handle_prefixes_and_named_constructors() {
        assert_eq!(constructor_name_candidates("DeadCard"), vec!["DeadCard"]);
        assert_eq!(constructor_name_candidates("ui.LiveCard"), vec!["LiveCard"]);
        assert_eq!(
            constructor_name_candidates("DeadCard.named"),
            vec!["DeadCard"]
        );
        assert_eq!(
            constructor_name_candidates("ui.LiveCard.named"),
            vec!["LiveCard"]
        );
    }

    #[test]
    fn constructor_name_candidates_ignore_lowercase_calls() {
        assert!(constructor_name_candidates("buildHeader").is_empty());
        assert!(constructor_name_candidates("context.watch").is_empty());
    }

    #[test]
    fn object_constructor_names_ignore_type_only_references()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r"
class Host extends StatefulWidget {
  State<DeadCard>? cachedState;
  Widget build(BuildContext context) => const LiveCard();
}
final Type marker = DeadCard;
List<DeadCard> cached = [];
void main() {
  new LegacyCard();
  ui.PrefixedCard();
  DeadCard.named();
  DeadCard.route;
  (() => BareIifeCard).call();
}
";
        let parsed = parse_tree(Path::new("lib/widgets.dart"), source)?;
        let names = object_constructor_names(parsed.tree().root_node(), parsed.source());

        assert_eq!(
            names,
            vec![
                "LiveCard",
                "LegacyCard",
                "ui.PrefixedCard",
                "DeadCard.named"
            ]
        );
        assert!(
            !names.iter().any(|name| name == "BareIifeCard"),
            "{names:?}"
        );
        Ok(())
    }

    #[test]
    fn split_closure_body_constructor_name_rejects_member_references()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r"
void main(BuildContext context) {
  print(DeadCard.route);
}
";
        let parsed = parse_tree(Path::new("lib/widgets.dart"), source)?;
        let root = parsed.tree().root_node();
        let call = first_named_node(root, "call_expression")
            .ok_or_else(|| std::io::Error::other("missing call_expression"))?;
        let member = first_named_node(root, "member_expression")
            .ok_or_else(|| std::io::Error::other("missing member_expression"))?;

        assert_eq!(
            split_closure_body_constructor_name(call, member, parsed.source()),
            None
        );
        Ok(())
    }

    fn first_named_node<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
        if node.kind() == kind {
            return Some(node);
        }
        let mut cursor = node.walk();
        node.named_children(&mut cursor)
            .find_map(|child| first_named_node(child, kind))
    }

    #[test]
    fn object_constructor_names_include_named_argument_arrow_closure_builders()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r"
class HomePage extends StatelessWidget {
  const HomePage({super.key});

  void _show(BuildContext context) {
    showModalBottomSheet<void>(
      context: context,
      builder: (context) => ClosureSheet(label: 'x'),
    );
  }
}
";
        let parsed = parse_tree(Path::new("lib/main.dart"), source)?;
        let names = object_constructor_names(parsed.tree().root_node(), parsed.source());

        assert!(names.iter().any(|name| name == "ClosureSheet"), "{names:?}");
        Ok(())
    }

    #[test]
    fn object_constructor_names_include_material_page_route_builders()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r"
void open(BuildContext context) {
  Navigator.push(
    context,
    MaterialPageRoute(
      builder: (BuildContext context) => CartScreen(),
    ),
  );
}
";
        let parsed = parse_tree(Path::new("lib/catalog.dart"), source)?;
        let names = object_constructor_names(parsed.tree().root_node(), parsed.source());

        assert!(names.iter().any(|name| name == "CartScreen"));
        Ok(())
    }

    #[test]
    fn object_constructor_names_include_generic_builder_children()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r"
class RoutineConceptFlow extends StatelessWidget {
  Widget build(BuildContext context) => BentoList<RoutineConceptItem>(
    items: const [],
    itemBuilder: (context, item, _) => RoutineConceptCard(
      title: item.title,
    ),
  );
}
";
        let parsed = parse_tree(Path::new("lib/routine_concept_flow.dart"), source)?;
        let names = object_constructor_names(parsed.tree().root_node(), parsed.source());

        assert!(names.iter().any(|name| name == "BentoList"), "{names:?}");
        assert!(
            names.iter().any(|name| name == "RoutineConceptCard"),
            "{names:?}"
        );
        Ok(())
    }
}
