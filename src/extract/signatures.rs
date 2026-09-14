use serde::{Deserialize, Serialize};
use tree_sitter::Node;

use super::{DeclarationKind, Location};

/// A type name referenced from a top-level declaration signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureReference {
    /// Top-level declaration containing the signature reference.
    pub declaration: String,
    /// Kind of the containing declaration.
    pub declaration_kind: DeclarationKind,
    /// Referenced type name.
    pub name: String,
    /// Location of the referenced type token.
    pub location: Location,
}

pub(super) fn extract_signature_references(
    root: Node<'_>,
    source: &str,
) -> Vec<SignatureReference> {
    let mut references = Vec::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        collect_top_level_signature_references(child, source, &mut references);
    }
    references
}

fn collect_top_level_signature_references(
    node: Node<'_>,
    source: &str,
    references: &mut Vec<SignatureReference>,
) {
    let identities = declaration_identities(node, source);
    if identities.is_empty() {
        return;
    }
    let Some(signature) = signature_node(node) else {
        return;
    };
    let mut names = Vec::new();
    collect_type_names(signature, source, &mut names);
    for (declaration, declaration_kind) in identities {
        references.extend(names.iter().filter_map(|node| {
            let name = node.utf8_text(source.as_bytes()).ok()?.to_owned();
            if name == declaration {
                return None;
            }
            Some(SignatureReference {
                declaration: declaration.clone(),
                declaration_kind,
                name,
                location: node.start_position().into(),
            })
        }));
    }
}

fn declaration_identities(node: Node<'_>, source: &str) -> Vec<(String, DeclarationKind)> {
    match node.kind() {
        "class_declaration" => optional_identity(
            field_text(node, "name", source).or_else(|| mixin_application_class_name(node, source)),
            DeclarationKind::Class,
        ),
        "mixin_declaration" => {
            optional_identity(field_text(node, "name", source), DeclarationKind::Mixin)
        }
        "extension_declaration" => optional_identity(
            Some(field_text(node, "name", source).unwrap_or_else(|| "<extension>".to_owned())),
            DeclarationKind::Extension,
        ),
        "extension_type_declaration" => optional_identity(
            node.child_by_field_name("name")
                .and_then(|child| first_identifier_text(child, source))
                .or_else(|| field_text(node, "name", source)),
            DeclarationKind::ExtensionType,
        ),
        "enum_declaration" => {
            optional_identity(field_text(node, "name", source), DeclarationKind::Enum)
        }
        "type_alias" => {
            optional_identity(type_alias_name(node, source), DeclarationKind::TypeAlias)
        }
        "top_level_variable_declaration" | "external_variable_declaration" => {
            variable_names(node, source)
                .into_iter()
                .map(|name| (name, DeclarationKind::Variable))
                .collect()
        }
        "function_declaration"
        | "external_function_declaration"
        | "getter_declaration"
        | "external_getter_declaration"
        | "setter_declaration"
        | "external_setter_declaration" => optional_identity(
            node.child_by_field_name("signature")
                .and_then(|signature| field_text(signature, "name", source)),
            DeclarationKind::Function,
        ),
        _ => Vec::new(),
    }
}

fn optional_identity(
    name: Option<String>,
    kind: DeclarationKind,
) -> Vec<(String, DeclarationKind)> {
    name.map_or_else(Vec::new, |name| vec![(name, kind)])
}

fn signature_node(node: Node<'_>) -> Option<Node<'_>> {
    match node.kind() {
        "class_declaration"
        | "mixin_declaration"
        | "enum_declaration"
        | "extension_declaration"
        | "extension_type_declaration"
        | "type_alias" => Some(node),
        "top_level_variable_declaration" | "external_variable_declaration" => {
            node.child_by_field_name("type")
        }
        "function_declaration"
        | "external_function_declaration"
        | "getter_declaration"
        | "external_getter_declaration"
        | "setter_declaration"
        | "external_setter_declaration" => node.child_by_field_name("signature"),
        _ => None,
    }
}

fn collect_type_names<'tree>(node: Node<'tree>, source: &str, names: &mut Vec<Node<'tree>>) {
    if matches!(
        node.kind(),
        "class_body" | "enum_body" | "extension_body" | "extension_type_body" | "mixin_body"
    ) {
        return;
    }
    if node.kind() == "type_identifier"
        && node
            .utf8_text(source.as_bytes())
            .is_ok_and(should_keep_type_name)
    {
        names.push(node);
        return;
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_type_names(child, source, names);
    }
}

fn should_keep_type_name(name: &str) -> bool {
    !matches!(
        name,
        "void"
            | "dynamic"
            | "Never"
            | "Null"
            | "Object"
            | "String"
            | "int"
            | "double"
            | "num"
            | "bool"
            | "List"
            | "Map"
            | "Set"
            | "Future"
            | "Stream"
            | "Iterable"
    )
}

fn type_alias_name(node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| matches!(child.kind(), "identifier" | "type_identifier"))
        .and_then(|child| child.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn variable_names(node: Node<'_>, source: &str) -> Vec<String> {
    let mut names = Vec::new();
    collect_named_fields(
        node,
        source,
        &["static_final_declaration", "initialized_identifier"],
        &mut names,
    );

    if names.is_empty()
        && let Some(identifier_list) = find_first_named_descendant(node, "identifier_list")
    {
        collect_direct_identifier_children(identifier_list, source, &mut names);
    }

    names
}

fn mixin_application_class_name(node: Node<'_>, source: &str) -> Option<String> {
    find_first_named_descendant(node, "mixin_application_class")
        .and_then(|child| first_identifier_text(child, source))
}

fn field_text(node: Node<'_>, field_name: &str, source: &str) -> Option<String> {
    node.child_by_field_name(field_name)
        .and_then(|child| child.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn collect_named_fields(
    node: Node<'_>,
    source: &str,
    owner_kinds: &[&str],
    names: &mut Vec<String>,
) {
    if owner_kinds.contains(&node.kind())
        && let Some(name) = field_text(node, "name", source)
    {
        names.push(name);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_named_fields(child, source, owner_kinds, names);
    }
}

fn collect_direct_identifier_children(node: Node<'_>, source: &str, names: &mut Vec<String>) {
    let mut cursor = node.walk();
    names.extend(
        node.named_children(&mut cursor)
            .filter(|child| child.kind() == "identifier")
            .filter_map(|child| child.utf8_text(source.as_bytes()).ok())
            .map(str::to_owned),
    );
}

fn first_identifier_text(node: Node<'_>, source: &str) -> Option<String> {
    if matches!(node.kind(), "identifier" | "type_identifier") {
        return node.utf8_text(source.as_bytes()).ok().map(str::to_owned);
    }

    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(|child| first_identifier_text(child, source))
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
