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
    let mut parent = node.parent();
    while let Some(ancestor) = parent {
        if matches!(ancestor.kind(), "class_body" | "class_declaration") {
            return false;
        }
        if enclosing_parameters_bind_name(ancestor, name, source)
            || earlier_local_binding_exists(ancestor, node.start_byte(), name, source)
        {
            return true;
        }
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
    let mut found = false;
    visit_named(node, &mut |candidate| {
        if found {
            return;
        }
        if candidate.kind() == "formal_parameter"
            && formal_parameter_name(candidate, source).as_deref() == Some(name)
        {
            found = true;
            return;
        }
        if matches!(
            candidate.kind(),
            "initialized_identifier" | "typed_identifier"
        ) && binding_name(candidate, source).as_deref() == Some(name)
        {
            found = true;
        }
    });
    found
}

fn earlier_local_binding_exists(
    scope: Node<'_>,
    usage_start: usize,
    name: &str,
    source: &str,
) -> bool {
    let mut found = false;
    visit_named(scope, &mut |candidate| {
        if found || candidate.start_byte() >= usage_start || candidate.end_byte() > usage_start {
            return;
        }
        if matches!(
            candidate.kind(),
            "local_variable_declaration"
                | "object_pattern"
                | "pattern_variable_declaration"
                | "record_pattern"
                | "switch_expression_case"
                | "switch_statement_case"
        ) && node_contains_binding_name(candidate, name, source)
        {
            found = true;
        }
    });
    found
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
