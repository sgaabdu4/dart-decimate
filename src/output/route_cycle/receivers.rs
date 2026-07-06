use tree_sitter::Node;

use super::{direct_named_child, strip_method_type_arguments};

pub(super) fn route_extension_receiver_node<'tree>(
    site: Node<'tree>,
    navigation_name: &str,
    source: &str,
) -> Option<Node<'tree>> {
    let function = site
        .child_by_field_name("function")
        .or_else(|| direct_named_child(site, "member_expression"))
        .or_else(|| direct_named_child(site, "null_aware_member_expression"))
        .or_else(|| direct_named_child(site, "instantiation_expression"))?;
    navigation_function_receiver(function, navigation_name, source)
}

fn navigation_function_receiver<'tree>(
    function: Node<'tree>,
    navigation_name: &str,
    source: &str,
) -> Option<Node<'tree>> {
    if function.kind() == "instantiation_expression" {
        return function
            .child_by_field_name("function")
            .or_else(|| direct_named_child(function, "member_expression"))
            .and_then(|inner| navigation_function_receiver(inner, navigation_name, source));
    }
    if !matches!(
        function.kind(),
        "member_expression" | "null_aware_member_expression" | "assignable_expression"
    ) {
        return None;
    }
    let property = function.child_by_field_name("property")?;
    let property_text = property.utf8_text(source.as_bytes()).ok()?;
    if strip_method_type_arguments(property_text.trim()) != navigation_name {
        return None;
    }
    function.child_by_field_name("object")
}
