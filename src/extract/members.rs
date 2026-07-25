use tree_sitter::Node;

use super::{
    MemberDeclaration, MemberKind, collect_direct_identifier_children, collect_named_fields,
    field_text, find_first_named_descendant, find_first_named_descendant_in, first_identifier_text,
};

pub(super) fn push_class_like_members(
    members: &mut Vec<MemberDeclaration>,
    node: Node<'_>,
    source: &str,
) {
    let Some(owner) = declaration_owner_name(node, source) else {
        return;
    };
    let Some(body) = node.child_by_field_name("body") else {
        return;
    };
    collect_class_like_members(members, &owner, body, source);
}

fn declaration_owner_name(node: Node<'_>, source: &str) -> Option<String> {
    if node.kind() == "class_declaration" {
        return field_text(node, "name", source).or_else(|| {
            find_first_named_descendant(node, "mixin_application_class")
                .and_then(|child| first_identifier_text(child, source))
        });
    }
    node.child_by_field_name("name")
        .and_then(|child| first_identifier_text(child, source))
        .or_else(|| field_text(node, "name", source))
}

fn collect_class_like_members(
    members: &mut Vec<MemberDeclaration>,
    owner: &str,
    body: Node<'_>,
    source: &str,
) {
    let mut cursor = body.walk();
    for child in body.named_children(&mut cursor) {
        match child.kind() {
            "enum_constant" => push_member(members, owner, MemberKind::EnumConstant, child, source),
            "class_member" => push_class_member(members, owner, child, source),
            _ => {}
        }
    }
}

fn push_class_member(
    members: &mut Vec<MemberDeclaration>,
    owner: &str,
    node: Node<'_>,
    source: &str,
) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "declaration" => push_bodyless_member(members, owner, child, source),
            "method_declaration" => push_method_member(members, owner, child, source),
            _ => {}
        }
    }
}

fn push_bodyless_member(
    members: &mut Vec<MemberDeclaration>,
    owner: &str,
    node: Node<'_>,
    source: &str,
) {
    if let Some(kind) = member_signature_kind(node) {
        push_signature_member(members, owner, kind, node, source);
        return;
    }

    let mut names = Vec::new();
    collect_member_field_names(node, source, &mut names);
    members.extend(names.into_iter().map(|name| MemberDeclaration {
        owner: owner.to_owned(),
        kind: MemberKind::Field,
        name,
        return_type: None,
        location: node.start_position().into(),
    }));
}

fn push_method_member(
    members: &mut Vec<MemberDeclaration>,
    owner: &str,
    node: Node<'_>,
    source: &str,
) {
    let Some(signature) = node.child_by_field_name("signature") else {
        return;
    };
    let Some(kind) = member_signature_kind(signature) else {
        return;
    };
    push_signature_member(members, owner, kind, signature, source);
}

fn member_signature_kind(node: Node<'_>) -> Option<MemberKind> {
    find_first_named_descendant_in(node, SIGNATURE_KINDS).and_then(|signature| {
        match signature.kind() {
            "constant_constructor_signature"
            | "constructor_signature"
            | "factory_constructor_signature"
            | "redirecting_factory_constructor_signature" => Some(MemberKind::Constructor),
            "operator_signature" => Some(MemberKind::Operator),
            "getter_signature" => Some(MemberKind::Getter),
            "setter_signature" => Some(MemberKind::Setter),
            "function_signature" => Some(MemberKind::Method),
            _ => None,
        }
    })
}

fn push_signature_member(
    members: &mut Vec<MemberDeclaration>,
    owner: &str,
    kind: MemberKind,
    node: Node<'_>,
    source: &str,
) {
    let Some(signature) = find_member_signature_node(node) else {
        return;
    };
    let name = match kind {
        MemberKind::Constructor => constructor_signature_name(signature, owner, source),
        MemberKind::Operator => field_text(signature, "operator", source),
        _ => field_text(signature, "name", source)
            .map(|name| member_name_without_owner(&name, owner)),
    };
    if let Some(name) = name {
        let return_type = (kind == MemberKind::Method)
            .then(|| declared_return_type(signature, source))
            .flatten();
        members.push(MemberDeclaration {
            owner: owner.to_owned(),
            kind,
            name,
            return_type,
            location: node.start_position().into(),
        });
    }
}

fn declared_return_type(signature: Node<'_>, source: &str) -> Option<String> {
    let name = signature.child_by_field_name("name")?;
    let before_name = source
        .get(signature.start_byte()..name.start_byte())?
        .trim();
    (!before_name.is_empty()).then(|| before_name.to_owned())
}

fn find_member_signature_node(node: Node<'_>) -> Option<Node<'_>> {
    if SIGNATURE_KINDS.contains(&node.kind()) {
        return Some(node);
    }
    find_first_named_descendant_in(node, SIGNATURE_KINDS)
}

fn constructor_signature_name(signature: Node<'_>, owner: &str, source: &str) -> Option<String> {
    let mut cursor = signature.walk();
    let mut parts = signature
        .children_by_field_name("name", &mut cursor)
        .filter(|child| matches!(child.kind(), "identifier" | "new"))
        .filter_map(|child| child.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned);
    let first = parts.next()?;
    let name = parts.last().unwrap_or(first);
    Some(member_name_without_owner(&name, owner))
}

const SIGNATURE_KINDS: &[&str] = &[
    "constant_constructor_signature",
    "constructor_signature",
    "factory_constructor_signature",
    "redirecting_factory_constructor_signature",
    "operator_signature",
    "getter_signature",
    "setter_signature",
    "function_signature",
];

fn push_member(
    members: &mut Vec<MemberDeclaration>,
    owner: &str,
    kind: MemberKind,
    node: Node<'_>,
    source: &str,
) {
    if let Some(name) = field_text(node, "name", source) {
        members.push(MemberDeclaration {
            owner: owner.to_owned(),
            kind,
            name,
            return_type: None,
            location: node.start_position().into(),
        });
    }
}

fn collect_member_field_names(node: Node<'_>, source: &str, names: &mut Vec<String>) {
    collect_named_fields(
        node,
        source,
        &["static_final_declaration", "initialized_identifier"],
        names,
    );
    if names.is_empty() {
        if let Some(identifier_list) = find_first_named_descendant(node, "identifier_list") {
            collect_direct_identifier_children(identifier_list, source, names);
        }
    }
}

fn member_name_without_owner(name: &str, owner: &str) -> String {
    name.strip_prefix(owner)
        .and_then(|rest| rest.strip_prefix('.'))
        .unwrap_or(name)
        .to_owned()
}
