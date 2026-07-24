use super::{DeclarationKind, IdentifierReference, Location, SignatureReference};

struct PrimaryConstructorHeader {
    declaration: String,
    declaration_kind: DeclarationKind,
    params_start: usize,
    params_end: usize,
}

struct Token {
    text: String,
    start: usize,
    preceded_by_dot: bool,
}

pub(super) fn extract_primary_constructor_identifier_references(
    source: &str,
) -> Vec<IdentifierReference> {
    primary_constructor_type_tokens(source)
        .into_iter()
        .map(|token| IdentifierReference {
            name: token.text,
            qualifier: None,
            location: byte_location(source, token.start),
        })
        .collect()
}

pub(super) fn extract_primary_constructor_signature_references(
    source: &str,
) -> Vec<SignatureReference> {
    let mut references = Vec::new();
    for header in primary_constructor_headers(source) {
        for token in type_tokens_in_params(source, header.params_start, header.params_end) {
            if token.text == header.declaration {
                continue;
            }
            references.push(SignatureReference {
                declaration: header.declaration.clone(),
                declaration_kind: header.declaration_kind,
                name: token.text,
                location: byte_location(source, token.start),
            });
        }
    }
    references
}

fn primary_constructor_type_tokens(source: &str) -> Vec<Token> {
    primary_constructor_headers(source)
        .into_iter()
        .flat_map(|header| type_tokens_in_params(source, header.params_start, header.params_end))
        .collect()
}

fn primary_constructor_headers(source: &str) -> Vec<PrimaryConstructorHeader> {
    let mut headers = Vec::new();
    let mut cursor = 0;
    while cursor < source.len() {
        let Some((keyword_start, keyword, kind)) = find_next_header_keyword(source, cursor) else {
            break;
        };
        cursor = keyword_start + keyword.len();
        let Some(mut name_start) = skip_whitespace(source, cursor) else {
            continue;
        };
        if matches!(keyword, "class" | "enum") && starts_keyword(source, name_start, "const") {
            let const_end = name_start + "const".len();
            let Some(after_const) = skip_whitespace(source, const_end) else {
                continue;
            };
            name_start = after_const;
        }
        let Some(name_end) = identifier_end(source, name_start) else {
            continue;
        };
        let declaration = source[name_start..name_end].to_owned();
        let mut header_cursor = skip_whitespace(source, name_end).unwrap_or(name_end);
        if source.as_bytes().get(header_cursor).copied() == Some(b'<') {
            if let Some(type_params_end) = matching_delimiter(source, header_cursor, b'<', b'>') {
                header_cursor =
                    skip_whitespace(source, type_params_end + 1).unwrap_or(type_params_end + 1);
            }
        }
        if source.as_bytes().get(header_cursor).copied() == Some(b'.') {
            let suffix_start =
                skip_whitespace(source, header_cursor + 1).unwrap_or(header_cursor + 1);
            let Some(suffix_end) = identifier_end(source, suffix_start) else {
                continue;
            };
            header_cursor = skip_whitespace(source, suffix_end).unwrap_or(suffix_end);
        }
        if source.as_bytes().get(header_cursor).copied() != Some(b'(') {
            continue;
        }
        let Some(params_end) = matching_delimiter(source, header_cursor, b'(', b')') else {
            continue;
        };
        headers.push(PrimaryConstructorHeader {
            declaration,
            declaration_kind: kind,
            params_start: header_cursor + 1,
            params_end,
        });
        cursor = params_end + 1;
    }
    headers
}

fn type_tokens_in_params(source: &str, start: usize, end: usize) -> Vec<Token> {
    split_top_level_commas(source, start, end)
        .into_iter()
        .flat_map(|(param_start, param_end)| type_tokens_in_param(source, param_start, param_end))
        .collect()
}

fn type_tokens_in_param(source: &str, start: usize, end: usize) -> Vec<Token> {
    let tokens = identifier_tokens(source, start, end)
        .into_iter()
        .filter(|token| !token.preceded_by_dot)
        .filter(|token| !is_parameter_modifier(&token.text))
        .filter(|token| !is_builtin_type(&token.text))
        .collect::<Vec<_>>();
    if tokens.len() <= 1 {
        return tokens
            .into_iter()
            .filter(|token| is_type_like(&token.text))
            .collect();
    }
    let last_index = tokens.len() - 1;
    tokens
        .into_iter()
        .enumerate()
        .filter_map(|(index, token)| {
            if index == last_index && !is_type_like(&token.text) {
                None
            } else if is_type_like(&token.text) {
                Some(token)
            } else {
                None
            }
        })
        .collect()
}

fn identifier_tokens(source: &str, start: usize, end: usize) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut cursor = start;
    while cursor < end {
        let Some(ch) = source
            .get(cursor..)
            .and_then(|suffix| suffix.chars().next())
        else {
            break;
        };
        if is_identifier_start(ch) {
            let token_start = cursor;
            let Some(token_end) = identifier_end(source, token_start) else {
                break;
            };
            tokens.push(Token {
                text: source[token_start..token_end].to_owned(),
                start: token_start,
                preceded_by_dot: previous_non_whitespace_byte(source.as_bytes(), token_start)
                    == Some(b'.'),
            });
            cursor = token_end;
            continue;
        }
        if matches!(source.as_bytes().get(cursor), Some(b'\'' | b'"')) {
            let Some(quote_end) = skip_quoted(source, cursor) else {
                break;
            };
            cursor = quote_end + 1;
            continue;
        }
        cursor += ch.len_utf8();
    }
    tokens
}

fn split_top_level_commas(source: &str, start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut segment_start = start;
    let mut cursor = start;
    let mut depth = 0usize;
    while cursor < end {
        match source.as_bytes()[cursor] {
            b'(' | b'[' | b'{' | b'<' => depth += 1,
            b')' | b']' | b'}' | b'>' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                spans.push((segment_start, cursor));
                segment_start = cursor + 1;
            }
            b'\'' | b'"' => {
                if let Some(quote_end) = skip_quoted(source, cursor) {
                    cursor = quote_end;
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    if segment_start < end {
        spans.push((segment_start, end));
    }
    spans
}

fn find_next_header_keyword(
    source: &str,
    start: usize,
) -> Option<(usize, &'static str, DeclarationKind)> {
    let mut cursor = start;
    while cursor < source.len() {
        if starts_keyword(source, cursor, "class") {
            return Some((cursor, "class", DeclarationKind::Class));
        }
        if starts_keyword(source, cursor, "enum") {
            return Some((cursor, "enum", DeclarationKind::Enum));
        }
        cursor = next_char_boundary(source, cursor)?;
    }
    None
}

fn is_parameter_modifier(name: &str) -> bool {
    matches!(
        name,
        "covariant" | "final" | "required" | "super" | "this" | "var"
    )
}

fn is_type_like(name: &str) -> bool {
    name.starts_with('_') || name.chars().next().is_some_and(char::is_uppercase)
}

fn is_builtin_type(name: &str) -> bool {
    matches!(
        name,
        "bool"
            | "double"
            | "dynamic"
            | "Future"
            | "int"
            | "Iterable"
            | "List"
            | "Map"
            | "Never"
            | "Null"
            | "num"
            | "Object"
            | "Set"
            | "Stream"
            | "String"
            | "void"
    )
}

fn byte_location(source: &str, byte: usize) -> Location {
    let mut line = 1;
    let mut column = 0;
    for ch in source[..byte].chars() {
        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += ch.len_utf8();
        }
    }
    Location { line, column }
}

fn starts_keyword(source: &str, start: usize, keyword: &str) -> bool {
    source
        .get(start..)
        .is_some_and(|suffix| suffix.starts_with(keyword))
        && !source
            .get(..start)
            .and_then(|prefix| prefix.chars().next_back())
            .is_some_and(is_identifier_char)
        && !source
            .get(start + keyword.len()..)
            .and_then(|suffix| suffix.chars().next())
            .is_some_and(is_identifier_char)
}

fn skip_whitespace(source: &str, start: usize) -> Option<usize> {
    let mut cursor = start;
    while cursor < source.len() {
        let ch = source.get(cursor..)?.chars().next()?;
        if !ch.is_whitespace() {
            return Some(cursor);
        }
        cursor += ch.len_utf8();
    }
    Some(cursor)
}

fn identifier_end(source: &str, start: usize) -> Option<usize> {
    let mut chars = source.get(start..)?.char_indices();
    let (_, first) = chars.next()?;
    if !is_identifier_start(first) {
        return None;
    }
    for (offset, ch) in chars {
        if !is_identifier_char(ch) {
            return Some(start + offset);
        }
    }
    Some(source.len())
}

fn matching_delimiter(source: &str, start: usize, open: u8, close: u8) -> Option<usize> {
    let bytes = source.as_bytes();
    if bytes.get(start).copied() != Some(open) {
        return None;
    }
    let mut depth = 0usize;
    let mut cursor = start;
    while cursor < bytes.len() {
        match bytes[cursor] {
            byte if byte == open => depth += 1,
            byte if byte == close => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(cursor);
                }
            }
            b'\'' | b'"' => cursor = skip_quoted(source, cursor)?,
            _ => {}
        }
        cursor += 1;
    }
    None
}

fn skip_quoted(source: &str, start: usize) -> Option<usize> {
    let quote = source.as_bytes().get(start).copied()?;
    let mut cursor = start + 1;
    while cursor < source.len() {
        match source.as_bytes()[cursor] {
            b'\\' => cursor += 2,
            byte if byte == quote => return Some(cursor),
            _ => cursor += 1,
        }
    }
    None
}

fn previous_non_whitespace_byte(bytes: &[u8], cursor: usize) -> Option<u8> {
    bytes
        .get(..cursor)?
        .iter()
        .rev()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
}

fn next_char_boundary(source: &str, start: usize) -> Option<usize> {
    source
        .get(start..)?
        .chars()
        .next()
        .map(|ch| start + ch.len_utf8())
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_identifier_char(ch: char) -> bool {
    is_identifier_start(ch) || ch.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::super::extract_dart_source;

    #[test]
    fn primary_constructor_references_survive_compatibility_parse()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = "\
class Screen(
  _Controller controller,
  {required _Service service, this.child, super.key}
) extends Widget {}

enum Tone(_Palette palette) {
  dark(_Palette());
}
";
        let extracted = extract_dart_source("lib/screen.dart", source)?;
        let references = extracted
            .references
            .iter()
            .map(|reference| reference.name.as_str())
            .collect::<Vec<_>>();
        let signatures = extracted
            .signature_references
            .iter()
            .map(|reference| {
                (
                    reference.declaration.as_str(),
                    reference.name.as_str(),
                    reference.location.line,
                )
            })
            .collect::<Vec<_>>();

        assert!(references.contains(&"_Controller"));
        assert!(references.contains(&"_Service"));
        assert!(references.contains(&"_Palette"));
        assert!(!references.contains(&"controller"));
        assert!(!references.contains(&"child"));
        assert!(signatures.contains(&("Screen", "_Controller", 2)));
        assert!(signatures.contains(&("Screen", "_Service", 3)));
        assert!(signatures.contains(&("Tone", "_Palette", 6)));

        Ok(())
    }
}
