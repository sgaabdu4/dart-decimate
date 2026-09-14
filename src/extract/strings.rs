pub(super) fn unquote_dart_string(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let (quoted, raw_literal) = quoted_dart_string_body(trimmed)?;

    if raw_literal {
        Some(quoted.to_owned())
    } else {
        decode_dart_string_body(quoted)
    }
}

fn quoted_dart_string_body(trimmed: &str) -> Option<(&str, bool)> {
    if let Some(raw) = trimmed
        .strip_prefix('r')
        .or_else(|| trimmed.strip_prefix('R'))
        && let Some(quoted) = quoted_dart_string_body_without_prefix(raw)
    {
        return Some((quoted, true));
    }

    quoted_dart_string_body_without_prefix(trimmed).map(|quoted| (quoted, false))
}

fn quoted_dart_string_body_without_prefix(value: &str) -> Option<&str> {
    value
        .strip_prefix("'''")
        .and_then(|inner| inner.strip_suffix("'''"))
        .or_else(|| {
            value
                .strip_prefix("\"\"\"")
                .and_then(|inner| inner.strip_suffix("\"\"\""))
        })
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|inner| inner.strip_suffix('\''))
        })
        .or_else(|| {
            value
                .strip_prefix('"')
                .and_then(|inner| inner.strip_suffix('"'))
        })
}

fn decode_dart_string_body(value: &str) -> Option<String> {
    let mut decoded = String::with_capacity(value.len());
    let mut chars = value.chars();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            decoded.push(ch);
            continue;
        }

        let escaped = chars.next()?;
        match escaped {
            'n' => decoded.push('\n'),
            'r' => decoded.push('\r'),
            'f' => decoded.push('\u{000C}'),
            'b' => decoded.push('\u{0008}'),
            't' => decoded.push('\t'),
            'v' => decoded.push('\u{000B}'),
            'x' => {
                let byte = u8::try_from(hex_escape_value(&mut chars, 2)?).ok()?;
                decoded.push(char::from(byte));
            }
            'u' => decoded.push(unicode_escape_value(&mut chars)?),
            other => {
                decoded.push('\\');
                decoded.push(other);
            }
        }
    }

    Some(decoded)
}

fn unicode_escape_value(chars: &mut std::str::Chars<'_>) -> Option<char> {
    if chars.clone().next() != Some('{') {
        return char::from_u32(hex_escape_value(chars, 4)?);
    }

    chars.next();
    let mut value = String::new();
    for ch in chars.by_ref() {
        if ch == '}' {
            return char::from_u32(u32::from_str_radix(&value, 16).ok()?);
        }
        if !ch.is_ascii_hexdigit() {
            return None;
        }
        value.push(ch);
    }
    None
}

fn hex_escape_value(chars: &mut std::str::Chars<'_>, digits: usize) -> Option<u32> {
    let mut value = String::with_capacity(digits);
    for _ in 0..digits {
        let ch = chars.next()?;
        if !ch.is_ascii_hexdigit() {
            return None;
        }
        value.push(ch);
    }
    u32::from_str_radix(&value, 16).ok()
}
