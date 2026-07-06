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
    if matches!(
        simple_type_name(&superclass).as_str(),
        "State" | "ConsumerState"
    ) {
        return true;
    }
    if superclass.contains('.') {
        return false;
    }
    let superclass = simple_type_name(&superclass);
    if !visited.insert(superclass.clone()) {
        return false;
    }
    find_class_declaration(root, &superclass, source).is_some_and(|declaration| {
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

fn root_node(mut node: Node<'_>) -> Node<'_> {
    while let Some(parent) = node.parent() {
        node = parent;
    }
    node
}

fn field_text(node: Node<'_>, field: &str, source: &str) -> Option<String> {
    node.child_by_field_name(field)
        .and_then(|child| child.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}
