use std::collections::BTreeMap;

use tree_sitter::Node;

use crate::Location;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WidgetParamCandidate {
    pub(super) name: String,
    pub(super) field_name: String,
    pub(super) location: Location,
}

pub(super) fn constructor_params(
    class: Node<'_>,
    widget_class: &str,
    source: &str,
) -> Vec<WidgetParamCandidate> {
    let mut declarations = Vec::new();
    collect_nodes_in(class, &["declaration"], &mut declarations);
    let mut params = BTreeMap::<String, WidgetParamCandidate>::new();
    for declaration in declarations {
        let Some(signature) = constructor_signature(declaration) else {
            continue;
        };
        if constructor_owner(signature, source).as_deref() != Some(widget_class) {
            continue;
        }
        let Some(parameters) = signature.child_by_field_name("parameters") else {
            continue;
        };
        let mut constructor_params = Vec::new();
        collect_nodes(parameters, "formal_parameter", &mut constructor_params);
        for param in constructor_params {
            if !is_named_parameter(param, signature, source) {
                continue;
            }
            let Some(candidate) = field_formal_candidate(param, source)
                .or_else(|| initializer_param_candidate(param, declaration, source))
            else {
                continue;
            };
            if candidate.name == "key" {
                continue;
            }
            params.entry(candidate.name.clone()).or_insert(candidate);
        }
    }
    params.into_values().collect()
}

pub(super) fn constructor_initializers_use_param(
    class: Node<'_>,
    widget_class: &str,
    param_name: &str,
    source: &str,
) -> bool {
    let mut declarations = Vec::new();
    collect_nodes_in(class, &["declaration"], &mut declarations);
    declarations.into_iter().any(|declaration| {
        let Some(signature) = constructor_signature(declaration) else {
            return false;
        };
        if constructor_owner(signature, source).as_deref() != Some(widget_class) {
            return false;
        }
        let mut initializers = Vec::new();
        collect_nodes(declaration, "field_initializer", &mut initializers);
        initializers.into_iter().any(|initializer| {
            let mut cursor = initializer.walk();
            initializer
                .children_by_field_name("value", &mut cursor)
                .any(|value| {
                    initializer_value_uses_param(value, param_name, source)
                        && !initializer_is_direct_assignment(value, param_name, source)
                })
        })
    })
}

fn initializer_is_direct_assignment(node: Node<'_>, param_name: &str, source: &str) -> bool {
    node.utf8_text(source.as_bytes())
        .ok()
        .is_some_and(|value| value.trim() == param_name)
}

const CONSTRUCTOR_SIGNATURES: &[&str] =
    &["constructor_signature", "constant_constructor_signature"];

fn constructor_signature(declaration: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = declaration.walk();
    declaration
        .named_children(&mut cursor)
        .find(|child| CONSTRUCTOR_SIGNATURES.contains(&child.kind()))
}

fn constructor_owner(signature: Node<'_>, source: &str) -> Option<String> {
    field_text(signature, "name", source).and_then(|name| name.split('.').next().map(str::to_owned))
}

fn is_named_parameter(param: Node<'_>, signature: Node<'_>, source: &str) -> bool {
    let mut parent = param.parent();
    while let Some(node) = parent {
        if same_node(node, signature) {
            return false;
        }
        if node.kind() == "optional_formal_parameters" {
            return node
                .utf8_text(source.as_bytes())
                .ok()
                .is_some_and(|text| text.trim_start().starts_with('{'));
        }
        parent = node.parent();
    }
    false
}

fn field_formal_candidate(param: Node<'_>, source: &str) -> Option<WidgetParamCandidate> {
    let text = param.utf8_text(source.as_bytes()).ok()?;
    let offset = text.find("this.")? + "this.".len();
    let name = text[offset..]
        .chars()
        .take_while(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '$'))
        .collect::<String>();
    if name.is_empty() {
        return None;
    }
    let min_byte = param.start_byte() + offset;
    let location = identifier_after(param, &name, min_byte, source).map_or_else(
        || param.start_position().into(),
        |node| node.start_position().into(),
    );
    Some(WidgetParamCandidate {
        field_name: name.clone(),
        name,
        location,
    })
}

fn initializer_param_candidate(
    param: Node<'_>,
    declaration: Node<'_>,
    source: &str,
) -> Option<WidgetParamCandidate> {
    let text = param.utf8_text(source.as_bytes()).ok()?;
    if text.contains("this.") || text.contains("super.") {
        return None;
    }
    let name_node = explicit_param_name(param, source)?;
    let name = name_node.utf8_text(source.as_bytes()).ok()?.to_owned();
    let field_name = initialized_field_from_param(declaration, &name, source)?;
    Some(WidgetParamCandidate {
        name,
        field_name,
        location: name_node.start_position().into(),
    })
}

fn explicit_param_name<'tree>(param: Node<'tree>, source: &str) -> Option<Node<'tree>> {
    if matches!(param.kind(), "typed_identifier" | "initialized_identifier") {
        return param.child_by_field_name("name");
    }
    if param.kind() == "formal_parameter" {
        return param
            .child_by_field_name("name")
            .or_else(|| last_identifier_child(param, source));
    }
    let mut cursor = param.walk();
    param
        .named_children(&mut cursor)
        .find_map(|child| explicit_param_name(child, source))
        .filter(|node| node.utf8_text(source.as_bytes()).ok() != Some("key"))
}

fn last_identifier_child<'tree>(node: Node<'tree>, source: &str) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| {
            matches!(child.kind(), "identifier" | "identifier_dollar_escaped")
                && child.utf8_text(source.as_bytes()).ok() != Some("key")
        })
        .last()
}

fn initialized_field_from_param(
    declaration: Node<'_>,
    param_name: &str,
    source: &str,
) -> Option<String> {
    let mut initializers = Vec::new();
    collect_nodes(declaration, "field_initializer", &mut initializers);
    initializers.into_iter().find_map(|initializer| {
        let field = field_text(initializer, "name", source)?;
        let mut cursor = initializer.walk();
        initializer
            .children_by_field_name("value", &mut cursor)
            .any(|value| initializer_value_uses_param(value, param_name, source))
            .then_some(field)
    })
}

fn initializer_value_uses_param(node: Node<'_>, name: &str, source: &str) -> bool {
    if matches!(node.kind(), "identifier" | "identifier_dollar_escaped")
        && node.utf8_text(source.as_bytes()).ok() == Some(name)
        && node.parent().is_none_or(|parent| {
            !field_contains_node(parent, "name", node)
                && !field_contains_node(parent, "property", node)
        })
    {
        return true;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| initializer_value_uses_param(child, name, source))
}

fn collect_nodes_in<'tree>(node: Node<'tree>, kinds: &[&str], nodes: &mut Vec<Node<'tree>>) {
    if kinds.contains(&node.kind()) {
        nodes.push(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_nodes_in(child, kinds, nodes);
    }
}

fn collect_nodes<'tree>(node: Node<'tree>, kind: &str, nodes: &mut Vec<Node<'tree>>) {
    if node.kind() == kind {
        nodes.push(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_nodes(child, kind, nodes);
    }
}

fn identifier_after<'tree>(
    node: Node<'tree>,
    name: &str,
    min_byte: usize,
    source: &str,
) -> Option<Node<'tree>> {
    if node.kind() == "identifier"
        && node.start_byte() >= min_byte
        && node.utf8_text(source.as_bytes()).ok() == Some(name)
    {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(|child| identifier_after(child, name, min_byte, source))
}

fn field_text(node: Node<'_>, field_name: &str, source: &str) -> Option<String> {
    node.child_by_field_name(field_name)
        .and_then(|child| child.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn field_contains_node(parent: Node<'_>, field_name: &str, child: Node<'_>) -> bool {
    let mut cursor = parent.walk();
    parent
        .children_by_field_name(field_name, &mut cursor)
        .any(|field| same_node(field, child))
}

fn same_node(left: Node<'_>, right: Node<'_>) -> bool {
    left.kind() == right.kind()
        && left.start_byte() == right.start_byte()
        && left.end_byte() == right.end_byte()
}
