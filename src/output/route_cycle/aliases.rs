use std::collections::BTreeSet;

use tree_sitter::Node;

use super::{
    direct_constructor_call_text, direct_named_child, is_identifier_character, strip_whitespace,
    unwrap_parenthesized_text,
};

pub(super) fn route_alias_receiver_node(
    node: Node<'_>,
    aliases: &BTreeSet<String>,
    source: &str,
) -> bool {
    node.utf8_text(source.as_bytes())
        .ok()
        .is_some_and(|text| route_alias_receiver_text(text, aliases))
}

pub(super) fn route_alias_receiver_text(text: &str, aliases: &BTreeSet<String>) -> bool {
    let compact = strip_whitespace(text);
    let receiver = unwrap_parenthesized_text(&compact)
        .unwrap_or(&compact)
        .trim_end_matches(['?', '!']);
    aliases.contains(receiver)
}

pub(super) fn route_aliases_at(
    root: Node<'_>,
    site: Node<'_>,
    route_classes: &BTreeSet<String>,
    source: &str,
) -> BTreeSet<String> {
    let mut aliases = BTreeSet::new();
    let mut steps = Vec::new();
    let mut path_child = site;
    let mut parent = site.parent();
    while let Some(scope) = parent {
        steps.push((scope, path_child));
        if same_node(scope, root) {
            break;
        }
        path_child = scope;
        parent = scope.parent();
    }

    for (scope, child) in steps.into_iter().rev() {
        if scope.kind() == "class_body" {
            collect_route_member_aliases(scope, &mut aliases, route_classes, source);
        }
        remove_aliases_shadowed_by_callable_parameters(scope, &mut aliases, source);
        collect_prior_route_aliases(scope, child, &mut aliases, route_classes, source);
    }
    aliases
}

fn collect_route_member_aliases(
    class_body: Node<'_>,
    aliases: &mut BTreeSet<String>,
    route_classes: &BTreeSet<String>,
    source: &str,
) {
    let mut cursor = class_body.walk();
    for member in class_body.named_children(&mut cursor) {
        collect_route_aliases_from_member(member, aliases, route_classes, source);
    }
}

fn collect_route_aliases_from_member(
    node: Node<'_>,
    aliases: &mut BTreeSet<String>,
    route_classes: &BTreeSet<String>,
    source: &str,
) {
    if is_callable_node(node.kind()) || node.kind() == "function_body" {
        return;
    }
    collect_route_alias_from_initialized_node(node, aliases, route_classes, true, source);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_route_aliases_from_member(child, aliases, route_classes, source);
    }
}

fn remove_aliases_shadowed_by_callable_parameters(
    scope: Node<'_>,
    aliases: &mut BTreeSet<String>,
    source: &str,
) {
    if !is_callable_node(scope.kind()) {
        return;
    }
    for parameter in callable_direct_parameter_names(scope, source) {
        aliases.remove(&parameter);
    }
}

fn collect_prior_route_aliases(
    scope: Node<'_>,
    path_child: Node<'_>,
    aliases: &mut BTreeSet<String>,
    route_classes: &BTreeSet<String>,
    source: &str,
) {
    let mut cursor = scope.walk();
    for sibling in scope.named_children(&mut cursor) {
        if same_node(sibling, path_child) || sibling.start_byte() >= path_child.start_byte() {
            break;
        }
        collect_route_aliases_from_lexical_declaration(sibling, aliases, route_classes, source);
    }
}

fn collect_route_aliases_from_lexical_declaration(
    node: Node<'_>,
    aliases: &mut BTreeSet<String>,
    route_classes: &BTreeSet<String>,
    source: &str,
) {
    match node.kind() {
        "local_variable_declaration" | "pattern_variable_declaration" => {
            collect_route_aliases_from_declaration_shape(node, aliases, route_classes, source);
        }
        "expression_statement" | "statement" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if matches!(
                    child.kind(),
                    "local_variable_declaration" | "pattern_variable_declaration"
                ) {
                    collect_route_aliases_from_declaration_shape(
                        child,
                        aliases,
                        route_classes,
                        source,
                    );
                }
            }
        }
        _ => {}
    }
}

fn collect_route_aliases_from_declaration_shape(
    node: Node<'_>,
    aliases: &mut BTreeSet<String>,
    route_classes: &BTreeSet<String>,
    source: &str,
) {
    let shadowed = aliases
        .iter()
        .filter(|alias| !alias.contains('.') && declaration_binds_name(node, alias, source))
        .cloned()
        .collect::<Vec<_>>();
    for alias in shadowed {
        aliases.remove(&alias);
    }
    collect_route_alias_from_initialized_node(node, aliases, route_classes, false, source);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if matches!(
            child.kind(),
            "initialized_identifier"
                | "initialized_identifier_list"
                | "initialized_variable_definition"
                | "variable_declaration_list"
        ) {
            collect_route_aliases_from_declaration_shape(child, aliases, route_classes, source);
        }
    }
}

fn collect_route_alias_from_initialized_node(
    node: Node<'_>,
    aliases: &mut BTreeSet<String>,
    route_classes: &BTreeSet<String>,
    include_this: bool,
    source: &str,
) {
    let Some(alias) = route_alias_from_initialized_node(node, route_classes, source) else {
        return;
    };
    if include_this {
        aliases.insert(format!("this.{alias}"));
    }
    aliases.insert(alias);
}

fn route_alias_from_initialized_node(
    node: Node<'_>,
    route_classes: &BTreeSet<String>,
    source: &str,
) -> Option<String> {
    if !matches!(
        node.kind(),
        "initialized_identifier" | "initialized_variable_definition"
    ) {
        return None;
    }
    let text = node.utf8_text(source.as_bytes()).ok()?;
    let (left, right) = text.split_once('=')?;
    if !route_constructor_expression_text(right, route_classes) {
        return None;
    }
    field_text(node, "name", source).or_else(|| identifier_before_equals(left))
}

fn route_constructor_expression_text(text: &str, route_classes: &BTreeSet<String>) -> bool {
    let expression = text.trim().trim_end_matches([',', ';']).trim();
    let compact = strip_whitespace(expression);
    let unwrapped = unwrap_parenthesized_text(&compact).unwrap_or(&compact);
    let receiver = unwrapped
        .strip_prefix("const")
        .or_else(|| unwrapped.strip_prefix("new"))
        .unwrap_or(unwrapped);
    route_classes
        .iter()
        .any(|route_class| direct_constructor_call_text(receiver, route_class))
}

fn declaration_binds_name(node: Node<'_>, name: &str, source: &str) -> bool {
    if binding_name(node, source).as_deref() == Some(name) {
        return true;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| declaration_binds_name(child, name, source))
}

fn binding_name(node: Node<'_>, source: &str) -> Option<String> {
    if !matches!(
        node.kind(),
        "formal_parameter"
            | "initialized_identifier"
            | "initialized_variable_definition"
            | "typed_identifier"
            | "variable_pattern"
    ) {
        return None;
    }
    field_text(node, "name", source)
}

fn callable_direct_parameter_names(node: Node<'_>, source: &str) -> Vec<String> {
    direct_parameter_lists(node)
        .into_iter()
        .flat_map(|parameters| direct_parameter_names(parameters, source))
        .collect()
}

fn direct_parameter_lists(node: Node<'_>) -> Vec<Node<'_>> {
    match node.kind() {
        "function_expression" => parameter_field_lists(node),
        "function_declaration" | "local_function_declaration" | "method_declaration" => {
            helper_signature(node)
                .and_then(parameter_list)
                .into_iter()
                .collect()
        }
        _ => Vec::new(),
    }
}

fn parameter_field_lists(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.children_by_field_name("parameters", &mut cursor)
        .filter_map(formal_parameter_list)
        .collect()
}

fn helper_signature(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("signature")
        .or_else(|| direct_named_child(node, "function_signature"))
}

fn parameter_list(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("parameters")
        .or_else(|| direct_named_child(node, "formal_parameter_list"))
}

fn formal_parameter_list(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "formal_parameter_list" {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == "formal_parameter_list")
}

fn direct_parameter_names(parameters: Node<'_>, source: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor = parameters.walk();
    for candidate in parameters.named_children(&mut cursor) {
        match candidate.kind() {
            "formal_parameter" => {
                if let Some(name) = formal_parameter_name(candidate, source) {
                    names.push(name);
                }
            }
            "optional_formal_parameters" => {
                let mut optional_cursor = candidate.walk();
                names.extend(
                    candidate
                        .named_children(&mut optional_cursor)
                        .filter(|child| child.kind() == "formal_parameter")
                        .filter_map(|child| formal_parameter_name(child, source)),
                );
            }
            _ => {}
        }
    }
    names
}

fn formal_parameter_name(param: Node<'_>, source: &str) -> Option<String> {
    field_text(param, "name", source).or_else(|| last_identifier_child(param, source))
}

fn last_identifier_child(node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| {
            matches!(
                child.kind(),
                "identifier" | "identifier_dollar_escaped" | "type_identifier"
            ) && child.utf8_text(source.as_bytes()).ok() != Some("key")
        })
        .filter_map(|child| child.utf8_text(source.as_bytes()).ok())
        .last()
        .map(str::to_owned)
}

fn field_text(node: Node<'_>, field: &str, source: &str) -> Option<String> {
    node.child_by_field_name(field)
        .and_then(|child| child.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn identifier_before_equals(text: &str) -> Option<String> {
    text.split(|character: char| !is_identifier_character(character))
        .rev()
        .find(|part| !part.is_empty())
        .map(str::to_owned)
}

fn is_callable_node(kind: &str) -> bool {
    matches!(
        kind,
        "function_declaration"
            | "function_expression"
            | "local_function_declaration"
            | "method_declaration"
    )
}

fn same_node(left: Node<'_>, right: Node<'_>) -> bool {
    left.kind() == right.kind()
        && left.start_byte() == right.start_byte()
        && left.end_byte() == right.end_byte()
}
