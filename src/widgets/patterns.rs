use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use super::simple_type_name;

pub(super) fn object_pattern_field_reads_by_type(
    root: Node<'_>,
    source: &str,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut reads = BTreeMap::<String, BTreeSet<String>>::new();
    visit_named(root, &mut |node| {
        if node.kind() != "object_pattern" {
            return;
        }
        let Some(type_name) = object_pattern_type_name(node, source) else {
            return;
        };
        let fields = object_pattern_field_names(node, source);
        if fields.is_empty() {
            return;
        }
        reads.entry(type_name).or_default().extend(fields);
    });
    reads
}

pub(super) fn pattern_binds_name(node: Node<'_>, name: &str, source: &str) -> bool {
    if node.kind() == "variable_pattern"
        && field_text(node, "name", source).as_deref() == Some(name)
    {
        return true;
    }
    if node.kind() == "object_pattern" {
        return object_pattern_binds_name(node, name, source);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| pattern_binds_name(child, name, source))
}

fn object_pattern_binds_name(node: Node<'_>, name: &str, source: &str) -> bool {
    let mut saw_type = false;
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if !saw_type && is_type_child(child) {
            saw_type = true;
            continue;
        }
        if child.kind() == "type_arguments" {
            continue;
        }
        if child.kind() == "label" {
            continue;
        }
        if !is_pattern_node(child) {
            continue;
        }
        if shorthand_pattern_name(child, source).as_deref() == Some(name)
            || pattern_binds_name(child, name, source)
        {
            return true;
        }
    }
    false
}

fn object_pattern_type_name(node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| is_type_child(*child))
        .and_then(|child| child.utf8_text(source.as_bytes()).ok())
        .map(|text| simple_type_name(text.split('<').next().unwrap_or(text)))
}

fn object_pattern_field_names(node: Node<'_>, source: &str) -> BTreeSet<String> {
    let mut fields = BTreeSet::new();
    let mut saw_type = false;
    let mut pending_label = None;
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if !saw_type && is_type_child(child) {
            saw_type = true;
            continue;
        }
        if child.kind() == "type_arguments" {
            continue;
        }
        if child.kind() == "label" {
            pending_label = label_name(child, source);
            continue;
        }
        if !is_pattern_node(child) {
            continue;
        }
        if let Some(field) = pending_label.take() {
            fields.insert(field);
        } else if let Some(field) = shorthand_pattern_name(child, source) {
            fields.insert(field);
        }
    }
    fields
}

fn label_name(node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| is_identifier(*child))
        .and_then(|child| child.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn shorthand_pattern_name(node: Node<'_>, source: &str) -> Option<String> {
    if node.kind() == "variable_pattern" {
        return field_text(node, "name", source).filter(|name| name != "_");
    }
    let text = node.utf8_text(source.as_bytes()).ok()?.trim();
    if is_identifier_text(text) && text != "_" {
        return Some(text.to_owned());
    }
    None
}

fn field_text(node: Node<'_>, field_name: &str, source: &str) -> Option<String> {
    node.child_by_field_name(field_name)
        .and_then(|field| field.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn is_type_child(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "type_identifier" | "identifier" | "qualified" | "type"
    )
}

fn is_pattern_node(node: Node<'_>) -> bool {
    node.kind().ends_with("_pattern") || node.kind() == "pattern"
}

fn is_identifier(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "identifier" | "identifier_dollar_escaped" | "type_identifier"
    )
}

fn is_identifier_text(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|character| {
            character == '_' || character == '$' || character.is_ascii_alphanumeric()
        })
}

fn visit_named(node: Node<'_>, visitor: &mut impl FnMut(Node<'_>)) {
    visitor(node);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visit_named(child, visitor);
    }
}
