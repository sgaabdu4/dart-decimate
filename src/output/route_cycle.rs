use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use tree_sitter::Node;

use crate::{DartFile, DependencyCycle, DependencyKind, ResolvedDependency, scan::ScannedProject};

mod aliases;
mod navigation;

use aliases::{route_alias_receiver_node, route_alias_receiver_text, route_aliases_at};
use navigation::{
    navigation_receiver_accepts_route_location, route_extension_navigation_has_context_argument,
};

pub(super) fn is_typed_go_router_registry_cycle(
    project: &ScannedProject,
    cycle: &DependencyCycle,
) -> bool {
    let cycle_files = cycle.files.iter().cloned().collect::<BTreeSet<_>>();
    let route_files = project
        .files
        .iter()
        .filter(|file| cycle_files.contains(&file.path) && is_typed_go_router_registry(file))
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    if route_files.is_empty() {
        return false;
    }

    let dependencies = project.graph.dependencies();
    let internal_dependencies = dependencies
        .iter()
        .filter(|dependency| {
            cycle_files.contains(&dependency.from_path)
                && cycle_files.contains(&dependency.to_path)
                && dependency.from_path != dependency.to_path
        })
        .collect::<Vec<_>>();
    let files_by_path = project
        .files
        .iter()
        .map(|file| (file.path.clone(), file))
        .collect::<BTreeMap<_, _>>();
    if internal_dependencies.is_empty()
        || internal_dependencies.iter().any(|dependency| {
            !is_typed_go_router_registry_edge(dependency, &route_files, &files_by_path)
        })
    {
        return false;
    }

    let mut route_helpers = BTreeMap::<PathBuf, BTreeSet<PathBuf>>::new();
    for dependency in &internal_dependencies {
        if route_files.contains(&dependency.from_path) {
            route_helpers
                .entry(dependency.from_path.clone())
                .or_default()
                .insert(dependency.to_path.clone());
        }
    }

    let helper_files = cycle_files
        .difference(&route_files)
        .cloned()
        .collect::<BTreeSet<_>>();
    !helper_files.is_empty()
        && route_files.iter().all(|route_file| {
            route_helpers
                .get(route_file)
                .is_some_and(|helpers| !helpers.is_empty())
        })
        && helper_files.iter().all(|helper_file| {
            route_files.iter().any(|route_file| {
                route_helpers
                    .get(route_file)
                    .is_some_and(|helpers| helpers.contains(helper_file))
                    && internal_dependencies.iter().any(|dependency| {
                        dependency.from_path == *helper_file && dependency.to_path == *route_file
                    })
            })
        })
}

fn is_typed_go_router_registry_edge(
    dependency: &ResolvedDependency,
    route_files: &BTreeSet<PathBuf>,
    files_by_path: &BTreeMap<PathBuf, &DartFile>,
) -> bool {
    if dependency.kind != DependencyKind::Import {
        return false;
    }
    let from_is_route = route_files.contains(&dependency.from_path);
    let to_is_route = route_files.contains(&dependency.to_path);
    if from_is_route == to_is_route {
        return false;
    }
    let route_path = if from_is_route {
        &dependency.from_path
    } else {
        &dependency.to_path
    };
    let helper_path = if from_is_route {
        &dependency.to_path
    } else {
        &dependency.from_path
    };
    let Some(route_file) = files_by_path.get(route_path) else {
        return false;
    };
    let Some(helper_file) = files_by_path.get(helper_path) else {
        return false;
    };
    is_typed_go_router_navigation_helper(helper_file, route_file)
}

fn is_typed_go_router_navigation_helper(helper_file: &DartFile, route_file: &DartFile) -> bool {
    let route_classes = route_file
        .routes
        .iter()
        .map(|route| route.route_class.clone())
        .collect::<BTreeSet<_>>();
    if route_classes.is_empty() {
        return false;
    }
    let Ok(source) = fs::read_to_string(&helper_file.path) else {
        return false;
    };
    let Ok(parsed) = crate::dart_parser::parse_dart_source_lossy(&helper_file.path, &source) else {
        return false;
    };
    helper_has_typed_route_navigation_call(
        parsed.tree().root_node(),
        parsed.source(),
        &route_classes,
    )
}

fn helper_has_typed_route_navigation_call(
    root: Node<'_>,
    source: &str,
    route_classes: &BTreeSet<String>,
) -> bool {
    let mut found = false;
    visit_named(root, &mut |node| {
        if found {
            return;
        }
        if typed_route_navigation_call(root, node, source, route_classes) {
            found = true;
        }
    });
    found
}

fn typed_route_navigation_call(
    root: Node<'_>,
    node: Node<'_>,
    source: &str,
    route_classes: &BTreeSet<String>,
) -> bool {
    if !matches!(
        node.kind(),
        "call_expression" | "function_expression_invocation"
    ) {
        return false;
    }
    let Some(arguments) = argument_list(node) else {
        return false;
    };
    let Some(prefix) = source.get(node.start_byte()..arguments.start_byte()) else {
        return false;
    };
    let Some(navigation_name) = navigation_call_name(prefix) else {
        return false;
    };
    route_extension_navigation_call(
        root,
        node,
        prefix,
        &navigation_name,
        arguments,
        route_classes,
        source,
    ) || route_location_navigation_call(
        root,
        node,
        prefix,
        &navigation_name,
        arguments,
        route_classes,
        source,
    )
}

fn argument_list(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("arguments")
        .or_else(|| direct_named_child(node, "arguments"))
        .or_else(|| direct_named_child(node, "argument_part"))
}

fn navigation_call_name(prefix: &str) -> Option<String> {
    let compact = strip_whitespace(prefix);
    let name = compact.rsplit('.').next().unwrap_or(compact.as_str());
    let name = strip_method_type_arguments(name);
    is_typed_route_navigation_reference(name).then(|| name.to_owned())
}

fn route_extension_navigation_call(
    root: Node<'_>,
    site: Node<'_>,
    prefix: &str,
    navigation_name: &str,
    arguments: Node<'_>,
    route_classes: &BTreeSet<String>,
    source: &str,
) -> bool {
    if !route_extension_navigation_has_context_argument(root, site, arguments, source) {
        return false;
    }
    let compact = strip_whitespace(prefix);
    let Some(receiver) = strip_navigation_suffix(&compact, ".", navigation_name) else {
        return false;
    };
    let aliases = route_aliases_at(root, site, route_classes, source);
    if route_alias_receiver_text(receiver, &aliases) {
        return true;
    }
    if route_classes
        .iter()
        .any(|route_class| direct_constructor_call_text(receiver, route_class))
    {
        return true;
    }
    let Some(raw_receiver) = raw_receiver_before_navigation(prefix, navigation_name) else {
        return false;
    };
    let constructor_receiver = constructor_receiver_text(raw_receiver);
    route_classes
        .iter()
        .any(|route_class| direct_constructor_call_text(&constructor_receiver, route_class))
}

fn route_location_navigation_call(
    root: Node<'_>,
    site: Node<'_>,
    prefix: &str,
    navigation_name: &str,
    arguments: Node<'_>,
    route_classes: &BTreeSet<String>,
    source: &str,
) -> bool {
    navigation_receiver(prefix, navigation_name).is_some_and(|receiver| {
        navigation_receiver_accepts_route_location(root, site, &receiver, source)
    }) && arguments_contain_route_location(arguments, root, site, route_classes, source)
}

fn navigation_receiver(prefix: &str, navigation_name: &str) -> Option<String> {
    let compact = strip_whitespace(prefix);
    for separator in [".", "?."] {
        if let Some(receiver) = strip_navigation_suffix(&compact, separator, navigation_name) {
            return (!receiver.is_empty()).then(|| receiver.to_owned());
        }
    }
    None
}

fn strip_navigation_suffix<'source>(
    compact: &'source str,
    separator: &str,
    navigation_name: &str,
) -> Option<&'source str> {
    let suffix = format!("{separator}{navigation_name}");
    if let Some(receiver) = compact.strip_suffix(&suffix) {
        return Some(receiver);
    }
    let typed_suffix = format!("{suffix}<");
    let start = compact.rfind(&typed_suffix)?;
    let type_arguments = compact.get(start + suffix.len()..)?;
    balanced_enclosed_text(type_arguments, '<', '>').then(|| &compact[..start])
}

fn raw_receiver_before_navigation<'source>(
    prefix: &'source str,
    navigation_name: &str,
) -> Option<&'source str> {
    let trimmed = prefix.trim_end();
    let separator = trimmed.rfind('.')?;
    let method = strip_whitespace(trimmed.get(separator + 1..)?.trim());
    (strip_method_type_arguments(&method) == navigation_name)
        .then(|| trimmed[..separator].trim_end())
}

fn constructor_receiver_text(receiver: &str) -> String {
    let trimmed = receiver.trim();
    let unwrapped = unwrap_parenthesized_text(trimmed).unwrap_or(trimmed);
    let without_keyword = strip_constructor_keyword_prefix(unwrapped).unwrap_or(unwrapped);
    strip_whitespace(without_keyword)
}

fn strip_constructor_keyword_prefix(text: &str) -> Option<&str> {
    for keyword in ["const", "new"] {
        let Some(rest) = text.strip_prefix(keyword) else {
            continue;
        };
        if rest.chars().next().is_some_and(char::is_whitespace) {
            return Some(rest.trim_start());
        }
    }
    None
}

fn arguments_contain_route_location(
    arguments: Node<'_>,
    root: Node<'_>,
    site: Node<'_>,
    route_classes: &BTreeSet<String>,
    source: &str,
) -> bool {
    route_location_argument(arguments, source).is_some_and(|argument| {
        route_location_expression(argument, root, site, route_classes, source)
    })
}

fn route_location_argument<'tree>(arguments: Node<'tree>, source: &str) -> Option<Node<'tree>> {
    let mut cursor = arguments.walk();
    for argument in arguments.named_children(&mut cursor) {
        if argument.kind() == "named_argument" {
            if named_argument_label(argument, source).as_deref() == Some("location") {
                return named_argument_value(argument);
            }
            continue;
        }
        return Some(argument);
    }
    None
}

fn named_argument_label(node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == "label")
        .and_then(|label| label.utf8_text(source.as_bytes()).ok())
        .map(str::trim)
        .and_then(|label| label.strip_suffix(':'))
        .map(str::to_owned)
}

fn named_argument_value(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() != "label")
}

fn route_location_expression(
    node: Node<'_>,
    root: Node<'_>,
    site: Node<'_>,
    route_classes: &BTreeSet<String>,
    source: &str,
) -> bool {
    route_location_member(
        unwrap_expression_node(node),
        root,
        site,
        route_classes,
        source,
    )
}

fn unwrap_expression_node(mut node: Node<'_>) -> Node<'_> {
    loop {
        if !matches!(node.kind(), "parenthesized_expression" | "expression") {
            return node;
        }
        let mut cursor = node.walk();
        let mut children = node.named_children(&mut cursor);
        let Some(child) = children.next() else {
            return node;
        };
        if children.next().is_some() {
            return node;
        }
        node = child;
    }
}

fn route_location_member(
    node: Node<'_>,
    root: Node<'_>,
    site: Node<'_>,
    route_classes: &BTreeSet<String>,
    source: &str,
) -> bool {
    if !matches!(
        node.kind(),
        "member_expression" | "null_aware_member_expression" | "assignable_expression"
    ) {
        return false;
    }
    let Some(property) = node.child_by_field_name("property") else {
        return false;
    };
    if property.utf8_text(source.as_bytes()).ok() != Some("location") {
        return false;
    }
    let Some(object) = node.child_by_field_name("object") else {
        return false;
    };
    route_constructor_receiver(object, route_classes, source)
        || route_alias_receiver_node(
            object,
            &route_aliases_at(root, site, route_classes, source),
            source,
        )
}

fn route_constructor_receiver(
    node: Node<'_>,
    route_classes: &BTreeSet<String>,
    source: &str,
) -> bool {
    if matches!(node.kind(), "parenthesized_expression" | "expression") {
        let mut cursor = node.walk();
        return node
            .named_children(&mut cursor)
            .any(|child| route_constructor_receiver(child, route_classes, source));
    }
    if matches!(
        node.kind(),
        "constructor_invocation" | "const_object_expression" | "new_expression"
    ) && node
        .child_by_field_name("type")
        .and_then(|type_node| type_node.utf8_text(source.as_bytes()).ok())
        .map(simple_type_name)
        .is_some_and(|type_name| route_classes.contains(&type_name))
    {
        return true;
    }
    false
}

fn direct_constructor_call_text(text: &str, route_class: &str) -> bool {
    let Some(receiver) = unwrap_parenthesized_text(text.trim()) else {
        return false;
    };
    let Some(after_type) = receiver_after_route_type(receiver, route_class) else {
        return false;
    };
    if after_type.starts_with('(') {
        return balanced_enclosed_text(after_type, '(', ')');
    }
    if after_type.starts_with('<')
        && let Some(arguments_start) =
            matching_enclosed_end(after_type, '<', '>').and_then(|end| after_type.get(end + 1..))
    {
        return arguments_start.starts_with('(')
            && balanced_enclosed_text(arguments_start, '(', ')');
    }
    false
}

fn receiver_after_route_type<'source>(
    receiver: &'source str,
    route_class: &str,
) -> Option<&'source str> {
    if let Some(after_type) = receiver.strip_prefix(route_class) {
        return Some(after_type);
    }
    let qualified_suffix = format!(".{route_class}");
    let route_start = receiver.find(&qualified_suffix)? + 1;
    receiver.get(route_start + route_class.len()..)
}

fn unwrap_parenthesized_text(text: &str) -> Option<&str> {
    let mut current = text;
    while current.starts_with('(') && current.ends_with(')') {
        let end = matching_enclosed_end(current, '(', ')')?;
        if end + 1 != current.len() {
            break;
        }
        current = current.get(1..end)?.trim();
    }
    Some(current)
}

fn balanced_enclosed_text(text: &str, open: char, close: char) -> bool {
    matching_enclosed_end(text, open, close).is_some_and(|end| end + 1 == text.len())
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

fn contains_identifier(text: &str, identifier: &str) -> bool {
    let mut cursor = 0usize;
    while let Some(relative) = text[cursor..].find(identifier) {
        let start = cursor + relative;
        let end = start + identifier.len();
        if !identifier_continues_before(text, start)
            && !text[end..]
                .chars()
                .next()
                .is_some_and(is_identifier_character)
        {
            return true;
        }
        cursor = end;
    }
    false
}

fn identifier_continues_before(text: &str, index: usize) -> bool {
    text[..index]
        .chars()
        .next_back()
        .is_some_and(is_identifier_character)
}

fn is_identifier_character(character: char) -> bool {
    character == '_' || character == '$' || character.is_ascii_alphanumeric()
}

fn simple_type_name(text: &str) -> String {
    text.trim_end_matches('?')
        .rsplit('.')
        .next()
        .unwrap_or(text)
        .split('<')
        .next()
        .unwrap_or(text)
        .to_owned()
}

fn is_typed_route_navigation_reference(name: &str) -> bool {
    matches!(name, "go" | "push" | "pushReplacement" | "replace")
}

fn strip_method_type_arguments(name: &str) -> &str {
    let Some(start) = name.find('<') else {
        return name;
    };
    let Some(type_arguments) = name.get(start..) else {
        return name;
    };
    if balanced_enclosed_text(type_arguments, '<', '>') {
        &name[..start]
    } else {
        name
    }
}

fn is_typed_go_router_registry(file: &DartFile) -> bool {
    if file.routes.is_empty()
        && !file.references.iter().any(|reference| {
            matches!(
                reference.name.as_str(),
                "TypedGoRoute"
                    | "TypedRelativeGoRoute"
                    | "TypedShellRoute"
                    | "TypedStatefulShellRoute"
                    | "TypedStatefulShellBranch"
            )
        })
    {
        return false;
    }

    file.parts.iter().any(|part| part.uri.ends_with(".g.dart"))
}

fn direct_named_child<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == kind)
}

fn strip_whitespace(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn visit_named(node: Node<'_>, visitor: &mut impl FnMut(Node<'_>)) {
    visitor(node);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visit_named(child, visitor);
    }
}
