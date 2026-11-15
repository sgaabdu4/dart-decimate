use tree_sitter::Node;

use super::{
    ClassSupertypes, MemberDeclaration, MemberKind, collect_direct_identifier_children,
    collect_named_fields, declared_return_type, field_text, find_first_named_descendant,
    find_first_named_descendant_in, first_identifier_text,
};

pub(super) fn push_class_like_members(
    members: &mut Vec<MemberDeclaration>,
    class_supertypes: &mut Vec<ClassSupertypes>,
    node: Node<'_>,
    source: &str,
) {
    let Some(owner) = declaration_owner_name(node, source) else {
        return;
    };
    let Some(body) = node.child_by_field_name("body") else {
        return;
    };
    let start = members.len();
    collect_class_like_members(members, &owner, body, source);
    if header_has_contract(node, body, source) {
        for member in &mut members[start..] {
            member.contract = true;
        }
        let header = source
            .get(node.start_byte()..body.start_byte())
            .unwrap_or_default();
        class_supertypes.push(header_supertypes(owner, header));
    }
}

fn header_supertypes(owner: String, header: &str) -> ClassSupertypes {
    let mut flat = String::new();
    let mut depth = 0_usize;
    for character in header.chars() {
        match character {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => flat.push(character),
            _ => {}
        }
    }
    let mut names = Vec::new();
    let mut implements = false;
    let mut mixes_in = false;
    let mut listing = false;
    for word in flat.split(|character: char| character.is_whitespace() || character == ',') {
        match word {
            "extends" | "on" => listing = true,
            "with" => {
                listing = true;
                mixes_in = true;
            }
            "implements" => {
                listing = true;
                implements = true;
            }
            "" => {}
            _ if listing => {
                let name = word
                    .rsplit('.')
                    .next()
                    .unwrap_or(word)
                    .trim_end_matches('?');
                if !name.is_empty() {
                    names.push(name.to_owned());
                }
            }
            _ => {}
        }
    }
    ClassSupertypes {
        owner,
        names,
        implements,
        mixes_in,
    }
}

fn header_has_contract(node: Node<'_>, body: Node<'_>, source: &str) -> bool {
    let header = source
        .get(node.start_byte()..body.start_byte())
        .unwrap_or_default();
    header.trim_start().starts_with('@')
        || header.split_whitespace().any(|word| {
            matches!(word, "extends" | "implements" | "with" | "on") || word.starts_with('@')
        })
}

fn member_is_annotated(node: Node<'_>, source: &str) -> bool {
    source
        .get(node.start_byte()..)
        .is_some_and(|text| text.trim_start().starts_with('@'))
        || node
            .prev_sibling()
            .is_some_and(|sibling| sibling.kind() == "annotation")
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
    let start = members.len();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "declaration" => push_bodyless_member(members, owner, child, source),
            "method_declaration" => push_method_member(members, owner, child, source),
            _ => {}
        }
    }
    if member_is_annotated(node, source) {
        for member in &mut members[start..] {
            member.contract = true;
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
        contract: false,
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
            contract: false,
            location: node.start_position().into(),
        });
    }
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
            contract: false,
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
    if names.is_empty()
        && let Some(identifier_list) = find_first_named_descendant(node, "identifier_list")
    {
        collect_direct_identifier_children(identifier_list, source, names);
    }
}

fn member_name_without_owner(name: &str, owner: &str) -> String {
    name.strip_prefix(owner)
        .and_then(|rest| rest.strip_prefix('.'))
        .unwrap_or(name)
        .to_owned()
}
