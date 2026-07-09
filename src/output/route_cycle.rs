use std::collections::BTreeSet;

use tree_sitter::Node;

mod aliases;
mod classification;
mod navigation;
mod receivers;
mod registry_api;
mod state_context;
#[cfg(test)]
mod tests;

use aliases::{route_alias_receiver_node, route_alias_receiver_text, route_aliases_at};
pub(super) use classification::{
    ResidualCycle, TypedGoRouterCycle, decompose_typed_go_router_cycle,
};
use navigation::{
    navigation_receiver_accepts_route_location, route_extension_navigation_has_context_argument,
};
use receivers::route_extension_receiver_node;
use state_context::class_extends_state;

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
    if route_extension_receiver_node(site, navigation_name, source)
        .is_some_and(|receiver| route_constructor_receiver(root, receiver, route_classes, source))
    {
        return true;
    }
    if route_classes.iter().any(|route_class| {
        direct_constructor_call_matches_route(root, receiver, route_class, source)
    }) {
        return true;
    }
    let Some(raw_receiver) = raw_receiver_before_navigation(prefix, navigation_name) else {
        return false;
    };
    let constructor_receiver = constructor_receiver_text(raw_receiver);
    route_classes.iter().any(|route_class| {
        direct_constructor_call_matches_route(root, &constructor_receiver, route_class, source)
    })
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
    route_location_argument(arguments).is_some_and(|argument| {
        route_location_expression(argument, root, site, route_classes, source)
    })
}

fn route_location_argument(arguments: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = arguments.walk();
    arguments
        .named_children(&mut cursor)
        .find(|argument| argument.kind() != "named_argument")
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
    route_constructor_receiver(root, object, route_classes, source)
        || route_alias_receiver_node(
            object,
            &route_aliases_at(root, site, route_classes, source),
            source,
        )
}

pub(super) fn route_constructor_receiver(
    root: Node<'_>,
    node: Node<'_>,
    route_classes: &BTreeSet<String>,
    source: &str,
) -> bool {
    if matches!(node.kind(), "parenthesized_expression" | "expression") {
        let mut cursor = node.walk();
        return node
            .named_children(&mut cursor)
            .any(|child| route_constructor_receiver(root, child, route_classes, source));
    }
    if matches!(
        node.kind(),
        "constructor_invocation" | "const_object_expression" | "new_expression"
    ) && constructor_receiver_matches_route(root, node, route_classes, source)
    {
        return true;
    }
    false
}

fn constructor_receiver_matches_route(
    root: Node<'_>,
    node: Node<'_>,
    route_classes: &BTreeSet<String>,
    source: &str,
) -> bool {
    let Some(type_text) = node
        .child_by_field_name("type")
        .and_then(|type_node| type_node.utf8_text(source.as_bytes()).ok())
    else {
        return false;
    };
    let type_text = strip_whitespace(type_text);
    if let Some(constructor) = node
        .child_by_field_name("constructor")
        .and_then(|constructor| constructor.utf8_text(source.as_bytes()).ok())
    {
        let suffix = format!(".{constructor}");
        if let Some(owner) = type_text.strip_suffix(&suffix) {
            return route_classes.iter().any(|route_class| {
                direct_route_type_text_matches(root, owner, route_class, source)
            });
        }
    }
    route_classes
        .iter()
        .any(|route_class| direct_route_type_text_matches(root, &type_text, route_class, source))
        || route_classes.iter().any(|route_class| {
            direct_named_constructor_type_text_matches_route(root, &type_text, route_class, source)
        })
}

fn direct_route_type_text_matches(
    root: Node<'_>,
    type_text: &str,
    route_class: &str,
    source: &str,
) -> bool {
    let type_text = type_text.trim_end_matches('?');
    let base = type_text.split('<').next().unwrap_or(type_text);
    !base.contains('.')
        && base == route_class
        && !route_class_locally_shadowed(root, route_class, source)
}

fn direct_named_constructor_type_text_matches_route(
    root: Node<'_>,
    type_text: &str,
    route_class: &str,
    source: &str,
) -> bool {
    if route_class_locally_shadowed(root, route_class, source) {
        return false;
    }
    let Some(rest) = type_text.strip_prefix(route_class) else {
        return false;
    };
    let after_type_arguments = if rest.starts_with('<') {
        let Some(end) = matching_enclosed_end(rest, '<', '>') else {
            return false;
        };
        rest.get(end + 1..).unwrap_or("")
    } else {
        rest
    };
    let Some(constructor) = after_type_arguments.strip_prefix('.') else {
        return false;
    };
    is_identifier_text(constructor)
}

fn direct_constructor_call_matches_route(
    root: Node<'_>,
    text: &str,
    route_class: &str,
    source: &str,
) -> bool {
    direct_constructor_call_text(text, route_class)
        && !route_class_locally_shadowed(root, route_class, source)
}

pub(super) fn route_class_locally_shadowed(
    root: Node<'_>,
    route_class: &str,
    source: &str,
) -> bool {
    let mut cursor = root.walk();
    root.named_children(&mut cursor).any(|child| {
        local_type_declaration_name(child, source).as_deref() == Some(route_class)
            || import_alias_name(child, source).as_deref() == Some(route_class)
    })
}

fn local_type_declaration_name(node: Node<'_>, source: &str) -> Option<String> {
    match node.kind() {
        "class_declaration"
        | "mixin_declaration"
        | "enum_declaration"
        | "extension_type_declaration" => field_text(node, "name", source),
        "type_alias" => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|child| matches!(child.kind(), "identifier" | "type_identifier"))
                .and_then(|child| child.utf8_text(source.as_bytes()).ok())
                .map(str::to_owned)
        }
        _ => None,
    }
}

fn import_alias_name(node: Node<'_>, source: &str) -> Option<String> {
    let import = if node.kind() == "library_import" {
        Some(node)
    } else {
        find_first_named_descendant(node, "library_import")
    }?;
    find_first_named_descendant(import, "import_specification")
        .and_then(|specification| field_text(specification, "alias", source))
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

fn direct_static_member_call_text(text: &str, type_name: &str, member_name: &str) -> bool {
    let Some(receiver) = unwrap_parenthesized_text(text.trim()) else {
        return false;
    };
    let Some(after_type) = receiver_after_route_type(receiver, type_name) else {
        return false;
    };
    let Some(after_dot) = after_type.strip_prefix('.') else {
        return false;
    };
    let Some(args_start) = after_dot.find('(') else {
        return false;
    };
    let method = after_dot[..args_start].split('<').next().unwrap_or("");
    let mut chars = method.chars();
    chars
        .next()
        .is_some_and(|first| first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(is_identifier_character)
        && method == member_name
        && balanced_enclosed_text(&after_dot[args_start..], '(', ')')
}

fn receiver_after_route_type<'source>(
    receiver: &'source str,
    route_class: &str,
) -> Option<&'source str> {
    receiver.strip_prefix(route_class)
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

fn is_identifier_character(character: char) -> bool {
    character == '_' || character == '$' || character.is_ascii_alphanumeric()
}

fn is_identifier_text(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(is_identifier_character)
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

fn direct_named_child<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == kind)
}

fn find_first_named_descendant<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    if node.kind() == kind {
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(found) = find_first_named_descendant(child, kind) {
            return Some(found);
        }
    }
    None
}

fn field_text(node: Node<'_>, field: &str, source: &str) -> Option<String> {
    node.child_by_field_name(field)
        .and_then(|child| child.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
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
