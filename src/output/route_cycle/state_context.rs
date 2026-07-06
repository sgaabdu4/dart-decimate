use std::collections::BTreeSet;

use tree_sitter::Node;

use super::{is_identifier_character, is_identifier_text, simple_type_name};

pub(super) fn class_extends_state(class_body: Node<'_>, source: &str) -> bool {
    let Some(class) = class_body
        .parent()
        .filter(|parent| parent.kind() == "class_declaration")
    else {
        return false;
    };
    let root = root_node(class);
    let mut visited = BTreeSet::new();
    class_or_local_superclass_extends_state(root, class, source, &mut visited)
}

fn class_or_local_superclass_extends_state(
    root: Node<'_>,
    class: Node<'_>,
    source: &str,
    visited: &mut BTreeSet<String>,
) -> bool {
    let Some(superclass) = direct_superclass_type_name(class, source) else {
        return false;
    };
    if framework_state_type_reference(root, &superclass, source) {
        return true;
    }
    let simple_superclass = simple_type_name(&superclass);
    if matches!(simple_superclass.as_str(), "State" | "ConsumerState")
        && let Some(declaration) = find_class_declaration(root, &simple_superclass, source)
    {
        if !visited.insert(simple_superclass) {
            return false;
        }
        return class_or_local_superclass_extends_state(root, declaration, source, visited);
    }
    if unprefixed_framework_state_import(root, &simple_superclass, source) {
        return true;
    }
    if superclass.contains('.') {
        return false;
    }
    if !visited.insert(simple_superclass.clone()) {
        return false;
    }
    find_class_declaration(root, &simple_superclass, source).is_some_and(|declaration| {
        class_or_local_superclass_extends_state(root, declaration, source, visited)
    })
}

fn direct_superclass_type_name(class: Node<'_>, source: &str) -> Option<String> {
    let superclass = class.child_by_field_name("superclass")?;
    let text = superclass.utf8_text(source.as_bytes()).ok()?;
    inherited_type_name(text.trim().strip_prefix("extends").unwrap_or(text))
}

fn inherited_type_name(text: &str) -> Option<String> {
    let mut name = String::new();
    for character in text.trim().chars() {
        if character == '<' {
            break;
        }
        if is_identifier_character(character) || character == '.' {
            name.push(character);
            continue;
        }
        if !name.is_empty() {
            break;
        }
    }
    is_qualified_identifier_text(&name).then_some(name)
}

fn find_class_declaration<'tree>(
    node: Node<'tree>,
    name: &str,
    source: &str,
) -> Option<Node<'tree>> {
    if node.kind() == "class_declaration"
        && field_text(node, "name", source).as_deref() == Some(name)
    {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(|child| find_class_declaration(child, name, source))
}

fn is_qualified_identifier_text(text: &str) -> bool {
    !text.is_empty() && text.split('.').all(is_identifier_text)
}

fn framework_state_type_reference(root: Node<'_>, type_name: &str, source: &str) -> bool {
    let simple = simple_type_name(type_name);
    if !matches!(simple.as_str(), "State" | "ConsumerState") {
        return false;
    }
    let Some((prefix, _)) = type_name.split_once('.') else {
        return false;
    };
    import_prefix_resolves_to_framework(root, prefix, &simple, source)
}

fn unprefixed_framework_state_import(root: Node<'_>, type_name: &str, source: &str) -> bool {
    if !matches!(type_name, "State" | "ConsumerState") {
        return false;
    }
    let mut found = false;
    visit_named(root, &mut |node| {
        if found || node.kind() != "library_import" {
            return;
        }
        if import_alias(node, source).is_some() {
            return;
        }
        if import_uri(node, source).is_some_and(|uri| state_framework_import_uri(&uri, type_name)) {
            found = true;
        }
    });
    found
}

fn import_prefix_resolves_to_framework(
    root: Node<'_>,
    prefix: &str,
    type_name: &str,
    source: &str,
) -> bool {
    let mut found = false;
    visit_named(root, &mut |node| {
        if found || node.kind() != "library_import" {
            return;
        }
        if import_alias(node, source).as_deref() != Some(prefix) {
            return;
        }
        if import_uri(node, source).is_some_and(|uri| state_framework_import_uri(&uri, type_name)) {
            found = true;
        }
    });
    found
}

fn import_alias(node: Node<'_>, source: &str) -> Option<String> {
    find_first_named_descendant(node, "import_specification")
        .and_then(|specification| field_text(specification, "alias", source))
}

fn import_uri(node: Node<'_>, source: &str) -> Option<String> {
    find_first_named_descendant(node, "uri")
        .and_then(|uri| uri.utf8_text(source.as_bytes()).ok())
        .and_then(unquote_dart_string)
}

fn unquote_dart_string(text: &str) -> Option<String> {
    let trimmed = text.trim().trim_start_matches('r');
    let quote = trimmed.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    trimmed
        .strip_prefix(quote)
        .and_then(|rest| rest.strip_suffix(quote))
        .map(str::to_owned)
}

fn state_framework_import_uri(uri: &str, type_name: &str) -> bool {
    match type_name {
        "State" => uri.starts_with("package:flutter/"),
        "ConsumerState" => {
            uri.starts_with("package:flutter_riverpod/")
                || uri.starts_with("package:hooks_riverpod/")
        }
        _ => false,
    }
}

fn find_first_named_descendant<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    if node.kind() == kind {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(|child| find_first_named_descendant(child, kind))
}

fn root_node(mut node: Node<'_>) -> Node<'_> {
    while let Some(parent) = node.parent() {
        node = parent;
    }
    node
}

fn visit_named(node: Node<'_>, visitor: &mut impl FnMut(Node<'_>)) {
    visitor(node);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visit_named(child, visitor);
    }
}

fn field_text(node: Node<'_>, field: &str, source: &str) -> Option<String> {
    node.child_by_field_name(field)
        .and_then(|child| child.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}
