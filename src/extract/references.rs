use tree_sitter::Node;

use super::{IdentifierReference, Location};

pub(super) fn extract_identifier_references(
    root: Node<'_>,
    source: &str,
) -> Vec<IdentifierReference> {
    let mut references = Vec::new();
    collect_identifier_references(root, source, &mut references);
    references
}

fn collect_identifier_references(
    node: Node<'_>,
    source: &str,
    references: &mut Vec<IdentifierReference>,
) {
    if node.kind() == "string_literal" {
        collect_string_interpolation_references(node, source, references);
    }

    if is_reference_identifier(node, source) {
        if let Ok(name) = node.utf8_text(source.as_bytes()) {
            references.push(IdentifierReference {
                name: name.to_owned(),
                qualifier: simple_member_qualifier(node, source),
                location: node.start_position().into(),
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_identifier_references(child, source, references);
    }
}

fn collect_string_interpolation_references(
    node: Node<'_>,
    source: &str,
    references: &mut Vec<IdentifierReference>,
) {
    let mut cursor = node.walk();
    let segments = node
        .children(&mut cursor)
        .filter(|child| is_string_literal_segment(child.kind()))
        .collect::<Vec<_>>();
    if !segments.is_empty() {
        for segment in segments {
            if is_raw_string_literal_segment(segment.kind()) {
                continue;
            }
            let Ok(text) = segment.utf8_text(source.as_bytes()) else {
                continue;
            };
            collect_string_segment_interpolation_references(
                text,
                segment.start_byte(),
                source,
                references,
            );
        }
        return;
    }

    let Ok(text) = node.utf8_text(source.as_bytes()) else {
        return;
    };
    if is_raw_string_literal(text) {
        return;
    }
    collect_string_segment_interpolation_references(text, node.start_byte(), source, references);
}

fn collect_string_segment_interpolation_references(
    text: &str,
    base_byte: usize,
    source: &str,
    references: &mut Vec<IdentifierReference>,
) {
    let bytes = text.as_bytes();
    let mut index = 0;
    while let Some(relative) = bytes[index..].iter().position(|byte| *byte == b'$') {
        let dollar = index + relative;
        index = dollar + 1;
        if is_escaped_dollar(bytes, dollar) {
            continue;
        }
        let interpolation_start = dollar + 1;
        if bytes.get(interpolation_start) == Some(&b'{') {
            let Some(end) = interpolation_body_end(text, interpolation_start + 1) else {
                continue;
            };
            index = end + 1;
            continue;
        }
        if let Some((name, start)) = interpolation_identifier(text, interpolation_start) {
            references.push(IdentifierReference {
                name: name.to_owned(),
                qualifier: None,
                location: location_at(source, base_byte + start),
            });
            index = start + name.len();
        }
    }
}

fn simple_member_qualifier(node: Node<'_>, source: &str) -> Option<String> {
    let parent = node.parent()?;
    if parent.kind() == "constant_pattern" {
        let text = parent.utf8_text(source.as_bytes()).ok()?;
        let (qualifier, property) = text.rsplit_once('.')?;
        let qualifier = qualifier.trim();
        if property.trim() == node.utf8_text(source.as_bytes()).ok()?
            && qualifier.bytes().all(is_identifier_byte)
        {
            return Some(qualifier.to_owned());
        }
        return None;
    }
    if parent.kind() == "qualified" {
        let qualifier = parent.named_child(0)?;
        let property_index = u32::try_from(parent.named_child_count().checked_sub(1)?).ok()?;
        let property = parent.named_child(property_index)?;
        if property == node && matches!(qualifier.kind(), "identifier" | "type_identifier") {
            return qualifier
                .utf8_text(source.as_bytes())
                .ok()
                .map(str::to_owned);
        }
        return None;
    }
    if !matches!(
        parent.kind(),
        "member_expression" | "null_aware_member_expression" | "assignable_expression"
    ) || parent.child_by_field_name("property")? != node
    {
        return None;
    }
    let object = parent.child_by_field_name("object")?;
    if !matches!(object.kind(), "identifier" | "type_identifier") {
        return None;
    }
    object.utf8_text(source.as_bytes()).ok().map(str::to_owned)
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')
}

fn is_string_literal_segment(kind: &str) -> bool {
    kind.starts_with("raw_string_literal") || kind.starts_with("string_literal_")
}

fn is_raw_string_literal_segment(kind: &str) -> bool {
    kind.starts_with("raw_string_literal")
}

fn is_raw_string_literal(text: &str) -> bool {
    text.trim_start().starts_with("r'")
        || text.trim_start().starts_with("r\"")
        || text.trim_start().starts_with("R'")
        || text.trim_start().starts_with("R\"")
}

fn is_escaped_dollar(bytes: &[u8], dollar: usize) -> bool {
    let mut slash_count = 0;
    let mut index = dollar;
    while index > 0 && bytes[index - 1] == b'\\' {
        slash_count += 1;
        index -= 1;
    }
    slash_count % 2 == 1
}

fn interpolation_identifier(text: &str, start: usize) -> Option<(&str, usize)> {
    let bytes = text.as_bytes();
    let end = identifier_end(bytes, start)?;
    Some((&text[start..end], start))
}

fn interpolation_body_end(text: &str, start: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = start;
    let mut brace_depth = 0;
    while index < bytes.len() {
        if line_comment_start(bytes, index) {
            index = line_comment_end(bytes, index);
            continue;
        }
        if block_comment_start(bytes, index) {
            index = block_comment_end(bytes, index);
            continue;
        }
        match bytes[index] {
            b'}' if brace_depth == 0 => return Some(index),
            b'}' => brace_depth -= 1,
            b'{' => brace_depth += 1,
            b'r' | b'R' if raw_quoted_segment_start(bytes, index).is_some() => {
                index = raw_quoted_segment_end(bytes, index);
                continue;
            }
            b'\'' | b'"' => {
                index = quoted_segment_end(bytes, index);
                continue;
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn line_comment_start(bytes: &[u8], start: usize) -> bool {
    bytes.get(start) == Some(&b'/') && bytes.get(start + 1) == Some(&b'/')
}

fn line_comment_end(bytes: &[u8], start: usize) -> usize {
    bytes[start + 2..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(bytes.len(), |relative| start + 2 + relative + 1)
}

fn block_comment_start(bytes: &[u8], start: usize) -> bool {
    bytes.get(start) == Some(&b'/') && bytes.get(start + 1) == Some(&b'*')
}

fn block_comment_end(bytes: &[u8], start: usize) -> usize {
    let mut index = start + 2;
    let mut depth = 1;
    while index + 1 < bytes.len() {
        if block_comment_start(bytes, index) {
            depth += 1;
            index += 2;
            continue;
        }
        if bytes[index] == b'*' && bytes[index + 1] == b'/' {
            depth -= 1;
            index += 2;
            if depth == 0 {
                return index;
            }
            continue;
        }
        index += 1;
    }
    bytes.len()
}

fn raw_quoted_segment_start(bytes: &[u8], start: usize) -> Option<usize> {
    let quote_start = start + 1;
    matches!(bytes.get(quote_start), Some(b'\'' | b'"')).then_some(quote_start)
}

fn raw_quoted_segment_end(bytes: &[u8], start: usize) -> usize {
    let Some(quote_start) = raw_quoted_segment_start(bytes, start) else {
        return start + 1;
    };
    let quote = bytes[quote_start];
    let is_triple =
        bytes.get(quote_start + 1) == Some(&quote) && bytes.get(quote_start + 2) == Some(&quote);
    let mut index = quote_start + if is_triple { 3 } else { 1 };
    while index < bytes.len() {
        if is_triple {
            if bytes[index] == quote
                && bytes.get(index + 1) == Some(&quote)
                && bytes.get(index + 2) == Some(&quote)
            {
                return index + 3;
            }
        } else if bytes[index] == quote {
            return index + 1;
        }
        index += 1;
    }
    bytes.len()
}

fn quoted_segment_end(bytes: &[u8], start: usize) -> usize {
    let quote = bytes[start];
    let is_triple = bytes.get(start + 1) == Some(&quote) && bytes.get(start + 2) == Some(&quote);
    let mut index = start + if is_triple { 3 } else { 1 };
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index += 2;
            continue;
        }
        if is_triple {
            if bytes[index] == quote
                && bytes.get(index + 1) == Some(&quote)
                && bytes.get(index + 2) == Some(&quote)
            {
                return index + 3;
            }
        } else if bytes[index] == quote {
            return index + 1;
        }
        index += 1;
    }
    bytes.len()
}

fn identifier_end(bytes: &[u8], start: usize) -> Option<usize> {
    if !bytes
        .get(start)
        .is_some_and(|byte| is_identifier_start(*byte))
    {
        return None;
    }
    let mut end = start + 1;
    while bytes.get(end).is_some_and(|byte| is_identifier_part(*byte)) {
        end += 1;
    }
    Some(end)
}

const fn is_identifier_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

const fn is_identifier_part(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit()
}

fn location_at(source: &str, byte_offset: usize) -> Location {
    let mut line = 1;
    let mut column = 0;
    for byte in source[..byte_offset].bytes() {
        if byte == b'\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    Location { line, column }
}

fn is_reference_identifier(node: Node<'_>, source: &str) -> bool {
    if !matches!(node.kind(), "identifier" | "type_identifier") {
        return false;
    }
    if node.utf8_text(source.as_bytes()).ok() == Some("_") {
        return false;
    }
    if has_ancestor_kind(node, &["import_or_export", "part_directive"]) {
        return false;
    }
    let Some(parent) = node.parent() else {
        return true;
    };
    if parent.kind() == "variable_pattern" || is_pattern_binding(node, source) {
        return false;
    }
    if is_pattern_field_label(node, source) {
        return false;
    }
    if parent.kind() == "type_alias" && is_type_alias_name(parent, node) {
        return false;
    }
    if DECLARATION_NAME_OWNER_KINDS.contains(&parent.kind()) && is_child_field(parent, node, "name")
    {
        return false;
    }
    true
}

const DECLARATION_NAME_OWNER_KINDS: &[&str] = &[
    "class_declaration",
    "constant_constructor_signature",
    "constructor_signature",
    "default_formal_parameter",
    "enum_declaration",
    "enum_constant",
    "extension_declaration",
    "extension_type_declaration",
    "factory_constructor_signature",
    "field_formal_parameter",
    "formal_parameter",
    "function_signature",
    "getter_signature",
    "initialized_identifier",
    "mixin_declaration",
    "normal_formal_parameter",
    "operator_signature",
    "redirecting_factory_constructor_signature",
    "setter_signature",
    "static_final_declaration",
    "type_alias",
];

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

fn is_child_field(parent: Node<'_>, child: Node<'_>, field_name: &str) -> bool {
    let mut cursor = parent.walk();
    parent
        .children_by_field_name(field_name, &mut cursor)
        .any(|field| same_node(field, child))
}

fn is_type_alias_name(parent: Node<'_>, child: Node<'_>) -> bool {
    let mut cursor = parent.walk();
    parent
        .named_children(&mut cursor)
        .find(|node| matches!(node.kind(), "identifier" | "type_identifier"))
        .is_some_and(|name| same_node(name, child))
}

fn is_pattern_field_label(node: Node<'_>, source: &str) -> bool {
    if !has_ancestor_kind(node, &["object_pattern", "record_pattern"]) {
        return false;
    }
    source
        .get(node.end_byte()..)
        .is_some_and(|suffix| suffix.trim_start().starts_with(':'))
}

fn is_pattern_binding(node: Node<'_>, source: &str) -> bool {
    if !has_ancestor_kind(
        node,
        &[
            "constant_pattern",
            "list_pattern",
            "map_pattern",
            "object_pattern",
            "record_pattern",
        ],
    ) {
        return false;
    }
    let Some(prefix) = source.get(..node.start_byte()) else {
        return false;
    };
    let token_prefix = prefix
        .rsplit(|character: char| {
            character.is_whitespace() || matches!(character, '(' | '[' | '{' | ':' | ',')
        })
        .find(|segment| !segment.is_empty())
        .unwrap_or_default();
    matches!(token_prefix, "final" | "var")
}

fn same_node(left: Node<'_>, right: Node<'_>) -> bool {
    left.kind() == right.kind()
        && left.start_byte() == right.start_byte()
        && left.end_byte() == right.end_byte()
}
