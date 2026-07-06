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
        remove_aliases_shadowed_by_header_scope(
            scope,
            child,
            site.start_byte(),
            &mut aliases,
            source,
        );
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

fn remove_aliases_shadowed_by_header_scope(
    scope: Node<'_>,
    path_child: Node<'_>,
    usage_start: usize,
    aliases: &mut BTreeSet<String>,
    source: &str,
) {
    let shadowed = aliases
        .iter()
        .filter(|alias| {
            !alias.contains('.')
                && scoped_header_binding_exists(scope, path_child, usage_start, alias, source)
        })
        .cloned()
        .collect::<Vec<_>>();
    for alias in shadowed {
        aliases.remove(&alias);
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
    remove_aliases_shadowed_by_lexical_sibling(node, aliases, source);
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

fn remove_aliases_shadowed_by_lexical_sibling(
    node: Node<'_>,
    aliases: &mut BTreeSet<String>,
    source: &str,
) {
    if node.kind() == "local_function_declaration"
        && let Some(name) = local_function_name(node, source)
    {
        aliases.remove(&name);
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
    let unwrapped = unwrap_parenthesized_text(expression).unwrap_or(expression);
    let without_keyword = strip_constructor_keyword_prefix(unwrapped).unwrap_or(unwrapped);
    let receiver = strip_whitespace(without_keyword);
    route_classes
        .iter()
        .any(|route_class| direct_constructor_call_text(&receiver, route_class))
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

fn scoped_header_binding_exists(
    scope: Node<'_>,
    path_child: Node<'_>,
    usage_start: usize,
    name: &str,
    source: &str,
) -> bool {
    if catch_header_binding_exists(scope, path_child, usage_start, name, source) {
        return true;
    }
    if !matches!(
        scope.kind(),
        "for_element"
            | "for_statement"
            | "if_element"
            | "if_statement"
            | "switch_expression_case"
            | "switch_statement_case"
    ) {
        return false;
    }
    if header_field_binding_exists(scope, path_child, usage_start, name, source) {
        return true;
    }
    let mut cursor = scope.walk();
    for child in scope.named_children(&mut cursor) {
        if same_node(child, path_child) || child.start_byte() >= path_child.start_byte() {
            break;
        }
        if header_binding_child(scope.kind(), child)
            && child.end_byte() <= usage_start
            && declaration_binds_name(child, name, source)
        {
            return true;
        }
    }
    false
}

fn catch_header_binding_exists(
    scope: Node<'_>,
    path_child: Node<'_>,
    usage_start: usize,
    name: &str,
    source: &str,
) -> bool {
    if scope.kind() == "catch_clause" {
        return scope.start_byte() < path_child.start_byte()
            && catch_clause_binds_name(scope, name, source);
    }
    if scope.kind() == "try_statement" {
        let mut previous: Option<Node<'_>> = None;
        let mut cursor = scope.walk();
        for child in scope.named_children(&mut cursor) {
            if same_node(child, path_child) || child.start_byte() >= path_child.start_byte() {
                return previous.is_some_and(|header| {
                    header.kind() == "catch_clause"
                        && catch_body_child(path_child)
                        && header.end_byte() <= usage_start
                        && !source_between_contains_finally(header, path_child, source)
                        && catch_clause_binds_name(header, name, source)
                });
            }
            previous = Some(child);
        }
    }
    false
}

fn catch_body_child(node: Node<'_>) -> bool {
    matches!(node.kind(), "block" | "function_body")
}

fn source_between_contains_finally(left: Node<'_>, right: Node<'_>, source: &str) -> bool {
    source
        .get(left.end_byte()..right.start_byte())
        .is_some_and(|text| text.contains("finally"))
}

fn catch_clause_binds_name(node: Node<'_>, name: &str, source: &str) -> bool {
    field_text(node, "exception", source).as_deref() == Some(name)
        || field_text(node, "stack_trace", source).as_deref() == Some(name)
}

fn header_field_binding_exists(
    scope: Node<'_>,
    path_child: Node<'_>,
    usage_start: usize,
    name: &str,
    source: &str,
) -> bool {
    if !matches!(scope.kind(), "for_element" | "for_statement") {
        return false;
    }
    let mut cursor = scope.walk();
    scope
        .children_by_field_name("name", &mut cursor)
        .any(|field| {
            field.end_byte() <= usage_start
                && field.start_byte() < path_child.start_byte()
                && field.utf8_text(source.as_bytes()).ok() == Some(name)
        })
}

fn header_binding_child(scope_kind: &str, child: Node<'_>) -> bool {
    if matches!(scope_kind, "for_element" | "for_statement") {
        return !is_body_statement_child(child.kind())
            || child.kind() == "local_variable_declaration";
    }
    !is_body_statement_child(child.kind()) && node_can_contain_pattern_binding(child)
}

fn node_can_contain_pattern_binding(node: Node<'_>) -> bool {
    let mut found = false;
    visit_named(node, &mut |candidate| {
        if !found
            && matches!(
                candidate.kind(),
                "list_pattern"
                    | "local_variable_declaration"
                    | "map_pattern"
                    | "object_pattern"
                    | "pattern_variable_declaration"
                    | "record_pattern"
                    | "variable_pattern"
            )
        {
            found = true;
        }
    });
    found
}

fn is_body_statement_child(kind: &str) -> bool {
    matches!(
        kind,
        "assert_statement"
            | "block"
            | "break_statement"
            | "continue_statement"
            | "declaration"
            | "do_statement"
            | "empty_statement"
            | "expression_statement"
            | "for_statement"
            | "if_statement"
            | "local_function_declaration"
            | "local_variable_declaration"
            | "return_statement"
            | "switch_statement"
            | "try_statement"
            | "while_statement"
            | "yield_statement"
    )
}

fn local_function_name(node: Node<'_>, source: &str) -> Option<String> {
    direct_named_child(node, "function_signature")
        .and_then(|signature| signature.child_by_field_name("name"))
        .or_else(|| node.child_by_field_name("name"))
        .and_then(|name| name.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn declaration_binds_name(node: Node<'_>, name: &str, source: &str) -> bool {
    if binding_name(node, source).as_deref() == Some(name) {
        return true;
    }
    if is_callable_node(node.kind()) || node.kind() == "function_body" {
        return false;
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

fn visit_named(node: Node<'_>, visitor: &mut impl FnMut(Node<'_>)) {
    visitor(node);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visit_named(child, visitor);
    }
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
