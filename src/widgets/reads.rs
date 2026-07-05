use tree_sitter::Node;

use super::patterns::pattern_binds_name;

pub(super) fn widget_body_uses_param(body: Node<'_>, name: &str, source: &str) -> bool {
    let mut found = false;
    visit_named(body, &mut |node| {
        if !found && is_this_member_access(node, name, source) {
            found = true;
        }
        if !found && is_body_identifier_use(node, name, source) {
            found = true;
        }
    });
    found
}

fn is_body_identifier_use(node: Node<'_>, name: &str, source: &str) -> bool {
    if !matches!(node.kind(), "identifier" | "identifier_dollar_escaped") {
        return false;
    }
    if node.utf8_text(source.as_bytes()).ok() != Some(name) {
        return false;
    }
    if has_ancestor_kind(node, BODY_USAGE_SKIP_ANCESTORS) {
        return false;
    }
    let Some(parent) = node.parent() else {
        return true;
    };
    if parent.kind() == "label" || name_field_of(parent, node) {
        return false;
    }
    if property_field_of(parent, node) || has_ancestor_kind(node, PATTERN_ANCESTORS) {
        return false;
    }
    if parent.kind() == "identifier_list" && has_ancestor_kind(parent, &["declaration"]) {
        return false;
    }
    !direct_identifier_shadowed(node, name, source)
}

const BODY_USAGE_SKIP_ANCESTORS: &[&str] = &[
    "constructor_signature",
    "constant_constructor_signature",
    "factory_constructor_signature",
    "redirecting_factory_constructor_signature",
    "constructor_param",
    "super_formal_parameter",
    "initializers",
    "initializer_list_entry",
    "field_initializer",
    "type",
    "typed_identifier",
];

const PATTERN_ANCESTORS: &[&str] = &[
    "cast_pattern",
    "constant_pattern",
    "list_pattern",
    "map_pattern",
    "null_assert_pattern",
    "null_check_pattern",
    "object_pattern",
    "pattern_variable_declaration",
    "record_pattern",
    "rest_pattern",
    "variable_pattern",
];

pub(super) fn state_body_uses_param(body: Node<'_>, name: &str, source: &str) -> bool {
    let mut found = false;
    visit_named(body, &mut |node| {
        if !found && is_widget_member_access(node, name, source) {
            found = true;
        }
    });
    found
}

fn is_widget_member_access(node: Node<'_>, name: &str, source: &str) -> bool {
    if !matches!(
        node.kind(),
        "member_expression" | "null_aware_member_expression" | "assignable_expression"
    ) {
        return false;
    }
    let Some(property) = node.child_by_field_name("property") else {
        return false;
    };
    if property.utf8_text(source.as_bytes()).ok() != Some(name) {
        return false;
    }
    let Some(object) = node.child_by_field_name("object") else {
        return false;
    };
    matches!(
        object.utf8_text(source.as_bytes()).ok(),
        Some("widget" | "oldWidget")
    )
}

fn is_this_member_access(node: Node<'_>, name: &str, source: &str) -> bool {
    if !matches!(
        node.kind(),
        "member_expression" | "null_aware_member_expression" | "assignable_expression"
    ) {
        return false;
    }
    let Some(property) = node.child_by_field_name("property") else {
        return false;
    };
    if property.utf8_text(source.as_bytes()).ok() != Some(name) {
        return false;
    }
    let Some(object) = node.child_by_field_name("object") else {
        return false;
    };
    object.utf8_text(source.as_bytes()).ok() == Some("this")
}

fn direct_identifier_shadowed(node: Node<'_>, name: &str, source: &str) -> bool {
    let mut child = node;
    let mut parent = node.parent();
    while let Some(ancestor) = parent {
        if matches!(ancestor.kind(), "class_body" | "class_declaration") {
            return false;
        }
        if enclosing_parameters_bind_name(ancestor, name, source)
            || earlier_local_binding_exists(ancestor, child, node.start_byte(), name, source)
        {
            return true;
        }
        child = ancestor;
        parent = ancestor.parent();
    }
    false
}

fn enclosing_parameters_bind_name(node: Node<'_>, name: &str, source: &str) -> bool {
    if !matches!(
        node.kind(),
        "declaration" | "function_expression" | "local_function_declaration" | "method_declaration"
    ) {
        return false;
    }
    let parameter_lists = enclosing_parameter_lists(node);
    if parameter_lists.is_empty() {
        return false;
    }
    for parameters in parameter_lists {
        if parameter_list_directly_binds_name(parameters, name, source) {
            return true;
        }
    }
    false
}

fn parameter_list_directly_binds_name(parameters: Node<'_>, name: &str, source: &str) -> bool {
    let mut cursor = parameters.walk();
    parameters.named_children(&mut cursor).any(|candidate| {
        if candidate.kind() == "formal_parameter" {
            return formal_parameter_name(candidate, source).as_deref() == Some(name);
        }
        candidate.kind() == "optional_formal_parameters"
            && optional_parameters_directly_bind_name(candidate, name, source)
    })
}

fn optional_parameters_directly_bind_name(parameters: Node<'_>, name: &str, source: &str) -> bool {
    let mut cursor = parameters.walk();
    parameters
        .named_children(&mut cursor)
        .filter(|candidate| candidate.kind() == "formal_parameter")
        .any(|candidate| formal_parameter_name(candidate, source).as_deref() == Some(name))
}

fn enclosing_parameter_lists(node: Node<'_>) -> Vec<Node<'_>> {
    match node.kind() {
        "function_expression" => parameter_field_lists(node),
        "method_declaration" => node
            .child_by_field_name("signature")
            .map_or_else(Vec::new, collect_parameter_lists),
        "local_function_declaration" | "declaration" => {
            direct_named_child(node, "function_signature")
                .map_or_else(Vec::new, collect_parameter_lists)
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

fn collect_parameter_lists(node: Node<'_>) -> Vec<Node<'_>> {
    own_parameter_list(node).into_iter().collect()
}

fn own_parameter_list(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "formal_parameter_list" {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(own_parameter_list)
}

fn formal_parameter_list(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "formal_parameter_list" {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == "formal_parameter_list")
}

fn earlier_local_binding_exists(
    scope: Node<'_>,
    path_child: Node<'_>,
    usage_start: usize,
    name: &str,
    source: &str,
) -> bool {
    if scoped_header_binding_exists(scope, path_child, usage_start, name, source) {
        return true;
    }

    let mut cursor = scope.walk();
    for sibling in scope.named_children(&mut cursor) {
        if same_node(sibling, path_child) || sibling.start_byte() >= path_child.start_byte() {
            break;
        }
        if lexical_sibling_binds_name(sibling, name, source) {
            return true;
        }
    }
    false
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
            && node_contains_binding_name(child, name, source)
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

fn lexical_sibling_binds_name(node: Node<'_>, name: &str, source: &str) -> bool {
    if node.kind() == "local_function_declaration" {
        return local_function_name(node, source).as_deref() == Some(name);
    }
    matches!(
        node.kind(),
        "local_variable_declaration" | "pattern_variable_declaration"
    ) && node_contains_binding_name(node, name, source)
}

fn local_function_name(node: Node<'_>, source: &str) -> Option<String> {
    direct_named_child(node, "function_signature")
        .and_then(|signature| signature.child_by_field_name("name"))
        .or_else(|| node.child_by_field_name("name"))
        .and_then(|name| name.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn pattern_or_local_binding_owner(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "list_pattern"
            | "local_variable_declaration"
            | "map_pattern"
            | "object_pattern"
            | "pattern_variable_declaration"
            | "record_pattern"
            | "variable_pattern"
    )
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
        if !found && pattern_or_local_binding_owner(candidate) {
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

fn node_contains_binding_name(node: Node<'_>, name: &str, source: &str) -> bool {
    let mut found = false;
    visit_named(node, &mut |candidate| {
        if found {
            return;
        }
        if binding_name(candidate, source).as_deref() == Some(name)
            || pattern_binds_name(candidate, name, source)
        {
            found = true;
        }
    });
    found
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
    node.child_by_field_name("name")
        .and_then(|name| name.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn field_text(node: Node<'_>, field_name: &str, source: &str) -> Option<String> {
    node.child_by_field_name(field_name)
        .and_then(|field| field.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn formal_parameter_name(param: Node<'_>, source: &str) -> Option<String> {
    param
        .child_by_field_name("name")
        .or_else(|| last_identifier_child(param, source))
        .and_then(|name| name.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn last_identifier_child<'tree>(node: Node<'tree>, source: &str) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| {
            matches!(
                child.kind(),
                "identifier" | "identifier_dollar_escaped" | "type_identifier"
            ) && child.utf8_text(source.as_bytes()).ok() != Some("key")
        })
        .last()
}

fn visit_named(node: Node<'_>, visitor: &mut impl FnMut(Node<'_>)) {
    visitor(node);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visit_named(child, visitor);
    }
}

fn direct_named_child<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == kind)
}

fn name_field_of(parent: Node<'_>, child: Node<'_>) -> bool {
    let mut cursor = parent.walk();
    parent
        .children_by_field_name("name", &mut cursor)
        .any(|field| same_node(field, child))
}

fn property_field_of(parent: Node<'_>, child: Node<'_>) -> bool {
    let mut cursor = parent.walk();
    parent
        .children_by_field_name("property", &mut cursor)
        .any(|field| same_node(field, child))
}

fn has_ancestor_kind(node: Node<'_>, kinds: &[&str]) -> bool {
    let mut parent = node.parent();
    while let Some(ancestor) = parent {
        if kinds.contains(&ancestor.kind()) {
            return true;
        }
        parent = ancestor.parent();
    }
    false
}

fn same_node(left: Node<'_>, right: Node<'_>) -> bool {
    left.kind() == right.kind()
        && left.start_byte() == right.start_byte()
        && left.end_byte() == right.end_byte()
}
