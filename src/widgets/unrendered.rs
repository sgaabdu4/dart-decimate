use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use tree_sitter::Node;

use crate::graph::normalize_against;
use crate::{DartCombinatorKind, DependencyKind, ScannedProject, TopLevelDeclaration};

use super::{
    UnrenderedWidgetClass, WidgetAnalysisError, collect_class_declarations, has_ancestor_kind,
    inheritance::class_superclasses, parse_tree, simple_type_name, visit_named, widget_kind,
};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct FileReachabilityFacts {
    path: PathBuf,
    class_names: BTreeSet<String>,
    widgets: Vec<UnrenderedWidgetClass>,
    object_constructors: Vec<String>,
    superclasses: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ClassKey {
    path: PathBuf,
    name: String,
}

pub(super) fn unrendered_widgets(
    project: &ScannedProject,
    paths: &[PathBuf],
) -> Result<Vec<UnrenderedWidgetClass>, WidgetAnalysisError> {
    let files = paths
        .par_iter()
        .map(|path| reachability_facts(path))
        .collect::<Result<Vec<_>, _>>()?;
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
    let render_counts = render_counts(project, &files, &candidate_keys);

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
    Ok(candidates)
}

fn reachability_facts(path: &Path) -> Result<FileReachabilityFacts, WidgetAnalysisError> {
    let source = fs::read_to_string(path).map_err(|source| WidgetAnalysisError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let parsed = parse_tree(path, &source)?;
    let root = parsed.tree().root_node();
    let mut classes = Vec::new();
    collect_class_declarations(root, &mut classes);

    Ok(FileReachabilityFacts {
        path: path.to_path_buf(),
        class_names: classes
            .iter()
            .filter_map(|class| {
                class
                    .child_by_field_name("name")?
                    .utf8_text(parsed.source().as_bytes())
                    .ok()
                    .map(str::to_owned)
            })
            .collect(),
        widgets: widget_classes(path, &classes, parsed.source()),
        object_constructors: object_constructor_names(root, parsed.source()),
        superclasses: class_superclasses(&classes, parsed.source()),
    })
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
            if let Some(constructor) = constructor_type_name(node, source) {
                constructors.extend(constructor_name_candidates(&constructor));
            }
        } else if is_arrow_body_constructor_identifier(node, source) {
            if let Ok(constructor) = node.utf8_text(source.as_bytes()) {
                constructors.extend(constructor_name_candidates(constructor));
            }
        }
    });
    constructors
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
    project: &ScannedProject,
    files: &[FileReachabilityFacts],
    candidate_keys: &BTreeSet<ClassKey>,
) -> BTreeMap<ClassKey, usize> {
    let declarations = files
        .iter()
        .flat_map(|file| {
            file.class_names.iter().map(|name| ClassKey {
                path: file.path.clone(),
                name: name.clone(),
            })
        })
        .collect::<Vec<_>>();
    let dependencies = project
        .graph
        .dependencies()
        .into_iter()
        .map(|dependency| {
            (
                normalize_against(&project.root, &dependency.from_path),
                normalize_against(&project.root, &dependency.to_path),
            )
        })
        .collect::<BTreeSet<_>>();
    let superclasses = resolved_superclasses(files, &declarations, &dependencies);
    let mut counts = BTreeMap::<ClassKey, usize>::new();
    for file in files {
        for constructed in &file.object_constructors {
            let Some(constructed) =
                resolve_class(&file.path, constructed, &declarations, &dependencies)
            else {
                continue;
            };
            let mut current = Some(constructed);
            let mut visited = BTreeSet::new();
            while let Some(class) = current {
                if !visited.insert(class.clone()) {
                    break;
                }
                if candidate_keys.contains(class) {
                    *counts.entry(class.clone()).or_default() += 1;
                }
                current = superclasses.get(class);
            }
        }
    }
    counts
}

fn resolved_superclasses(
    files: &[FileReachabilityFacts],
    declarations: &[ClassKey],
    dependencies: &BTreeSet<(PathBuf, PathBuf)>,
) -> BTreeMap<ClassKey, ClassKey> {
    let mut resolved = BTreeMap::new();
    for file in files {
        for (class, parent) in &file.superclasses {
            let Some(parent) = resolve_class(&file.path, parent, declarations, dependencies) else {
                continue;
            };
            resolved.insert(
                ClassKey {
                    path: file.path.clone(),
                    name: class.clone(),
                },
                parent.clone(),
            );
        }
    }
    resolved
}

fn resolve_class<'a>(
    from: &Path,
    name: &str,
    declarations: &'a [ClassKey],
    dependencies: &BTreeSet<(PathBuf, PathBuf)>,
) -> Option<&'a ClassKey> {
    let mut local = declarations
        .iter()
        .filter(|class| class.name == name && class.path == from);
    if let Some(candidate) = local.next() {
        return local.next().is_none().then_some(candidate);
    }
    let mut imported = declarations.iter().filter(|class| {
        class.name == name && dependencies.contains(&(from.to_path_buf(), class.path.clone()))
    });
    let candidate = imported.next()?;
    imported.next().is_none().then_some(candidate)
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
            vec!["LiveCard", "LegacyCard", "PrefixedCard", "DeadCard"]
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
