use std::path::Path;

use super::{DetectedSecurityCandidate, SecurityCategory, SecurityConfidence, SecurityOccurrence};
use crate::Location;
use crate::generated::is_generated_dart_path;

pub(super) fn detect_in_source(path: &Path, source: &str) -> Vec<DetectedSecurityCandidate> {
    let mut candidates = Vec::new();
    detect_firebase_api_keys(path, source, &mut candidates);
    detect_hardcoded_secrets(path, source, &mut candidates);
    detect_insecure_transport(path, source, &mut candidates);
    detect_tls_bypass(path, source, &mut candidates);
    detect_webview_risk(path, source, &mut candidates);
    detect_process_execution(path, source, &mut candidates);
    detect_raw_sql(path, source, &mut candidates);
    detect_plain_secret_storage(path, source, &mut candidates);
    candidates
}

pub(super) fn is_ignored_path(path: &Path) -> bool {
    if path.components().any(|component| {
        let value = component.as_os_str().to_string_lossy();
        matches!(value.as_ref(), "test" | "integration_test" | "test_driver")
    }) {
        return true;
    }
    is_generated_dart_path(path)
}

fn detect_hardcoded_secrets(
    path: &Path,
    source: &str,
    candidates: &mut Vec<DetectedSecurityCandidate>,
) {
    for literal in string_literals(source) {
        if is_comment_match(source, literal.index) {
            continue;
        }
        if let Some(relative_index) = javascript_password_autofill_index(&literal.value) {
            candidates.push(detected(
                path,
                source,
                literal.value_index + relative_index,
                SecurityCategory::HardcodedSecret,
                "javascript-password-autofill",
                SecurityConfidence::High,
                "password-autofill-literal",
            ));
            continue;
        }
        if is_placeholder(&literal.value) {
            continue;
        }
        let line = line_at(source, literal.index);
        let secret_value = has_secret_shape(&literal.value);
        let firebase_options_literal = firebase_options_context(source, literal.index);
        let secret_name = if firebase_options_literal {
            firebase_secret_name_context(source, literal.index)
        } else {
            has_secret_like_name(line)
        };
        if is_module_uri_directive_line(line)
            || firebase_api_key_context(source, literal.index)
            || (!secret_value && benign_secret_named_literal(&literal.value))
            || (!secret_value && firebase_options_literal && !secret_name)
            || google_app_id_context(source, literal.index)
            || (!secret_value && literal_looks_like_storage_key(&literal.value))
            || (!secret_value && diagnostic_context(source, literal.index))
            || (!secret_value && literal.value.contains('$'))
        {
            continue;
        }
        if secret_name && literal.value.len() >= 12 || secret_value {
            candidates.push(detected(
                path,
                source,
                literal.index,
                SecurityCategory::HardcodedSecret,
                "secret-literal",
                if secret_value {
                    SecurityConfidence::High
                } else {
                    SecurityConfidence::Medium
                },
                "string-literal",
            ));
        }
    }
}

fn detect_firebase_api_keys(
    path: &Path,
    source: &str,
    candidates: &mut Vec<DetectedSecurityCandidate>,
) {
    for literal in string_literals(source) {
        if is_comment_match(source, literal.index)
            || is_placeholder(&literal.value)
            || !concrete_firebase_api_key_literal(&literal.value)
            || !firebase_api_key_context(source, literal.index)
        {
            continue;
        }
        candidates.push(detected(
            path,
            source,
            literal.index,
            SecurityCategory::FirebaseApiKey,
            "firebase-api-key",
            SecurityConfidence::Medium,
            "FirebaseOptions.apiKey",
        ));
    }
}

fn detect_insecure_transport(
    path: &Path,
    source: &str,
    candidates: &mut Vec<DetectedSecurityCandidate>,
) {
    for literal in string_literals(source) {
        if is_comment_match(source, literal.index)
            || !literal.value.starts_with("http://")
            || is_local_http_url(&literal.value)
        {
            continue;
        }
        let line = line_at(source, literal.index);
        if has_network_context(line) {
            candidates.push(detected(
                path,
                source,
                literal.index,
                SecurityCategory::InsecureTransport,
                "cleartext-http",
                SecurityConfidence::High,
                "http-url",
            ));
        }
    }
}

fn detect_tls_bypass(path: &Path, source: &str, candidates: &mut Vec<DetectedSecurityCandidate>) {
    for pattern in [
        "badCertificateCallback",
        "HttpOverrides.global",
        "SecurityContext(withTrustedRoots: false",
        "validateCertificate",
    ] {
        for index in match_indices(source, pattern) {
            let window = following_window(source, index, 4);
            let risky = match pattern {
                "badCertificateCallback" | "validateCertificate" => {
                    returns_true(window) && !returns_false(window)
                }
                _ => true,
            };
            if risky {
                candidates.push(detected(
                    path,
                    source,
                    index,
                    SecurityCategory::TlsBypass,
                    pattern,
                    SecurityConfidence::High,
                    pattern,
                ));
            }
        }
    }
}

fn detect_webview_risk(path: &Path, source: &str, candidates: &mut Vec<DetectedSecurityCandidate>) {
    for pattern in [
        "JavaScriptMode.unrestricted",
        "javaScriptEnabled: true",
        "allowFileAccess: true",
        "allowFileAccessFromFileURLs: true",
        "allowUniversalAccessFromFileURLs: true",
    ] {
        for index in match_indices(source, pattern) {
            candidates.push(detected(
                path,
                source,
                index,
                SecurityCategory::WebViewRisk,
                "webview",
                SecurityConfidence::High,
                pattern,
            ));
        }
    }
    for literal in string_literals(source) {
        if literal.value.starts_with("file://") {
            let line = line_at(source, literal.index);
            if line.contains("loadUrl") || line.contains("loadRequest") {
                candidates.push(detected(
                    path,
                    source,
                    literal.index,
                    SecurityCategory::WebViewRisk,
                    "webview-file-url",
                    SecurityConfidence::Medium,
                    "file-url",
                ));
            }
        }
    }
}

fn detect_process_execution(
    path: &Path,
    source: &str,
    candidates: &mut Vec<DetectedSecurityCandidate>,
) {
    for pattern in [
        "Process.run(",
        "Process.start(",
        "processManager.run(",
        "processManager.start(",
    ] {
        for index in match_indices(source, pattern) {
            let window = following_window(source, index, 3);
            let args = call_inside_after(source, index + pattern.len());
            let risky = window.contains("runInShell: true")
                || args.as_deref().is_some_and(|call| {
                    !first_call_arg_is_fixed_literal(call) || has_dynamic_text(call)
                });
            if risky {
                candidates.push(detected(
                    path,
                    source,
                    index,
                    SecurityCategory::ProcessExecution,
                    "process-exec",
                    SecurityConfidence::High,
                    pattern.trim_end_matches('('),
                ));
            }
        }
    }
}

fn detect_raw_sql(path: &Path, source: &str, candidates: &mut Vec<DetectedSecurityCandidate>) {
    for pattern in [
        ".rawQuery(",
        ".rawInsert(",
        ".rawUpdate(",
        ".rawDelete(",
        ".execute(",
        ".customSelect(",
        ".customStatement(",
    ] {
        for index in match_indices(source, pattern) {
            let Some(call) = call_inside_after(source, index + pattern.len()) else {
                continue;
            };
            if sql_call_is_parameterized(&call) {
                continue;
            }
            let risky = has_dynamic_text(&call) || !first_call_arg_is_fixed_literal(&call);
            let broad_execute = pattern == ".execute(";
            if risky && (!broad_execute || sql_like_text(&call)) {
                candidates.push(detected(
                    path,
                    source,
                    index,
                    SecurityCategory::RawSql,
                    "raw-sql",
                    SecurityConfidence::High,
                    pattern.trim_start_matches('.').trim_end_matches('('),
                ));
            }
        }
    }
    for index in match_indices(source, "where:") {
        let line = line_at(source, index);
        if (line.contains('$') || line.contains('+')) && !line.contains("whereArgs") {
            candidates.push(detected(
                path,
                source,
                index,
                SecurityCategory::RawSql,
                "dynamic-where",
                SecurityConfidence::Medium,
                "where",
            ));
        }
    }
}

fn detect_plain_secret_storage(
    path: &Path,
    source: &str,
    candidates: &mut Vec<DetectedSecurityCandidate>,
) {
    for pattern in [
        ".setString(",
        ".setStringList(",
        ".put(",
        ".writeAsString(",
        ".writeAsBytes(",
    ] {
        for index in match_indices(source, pattern) {
            let line = line_at(source, index);
            if line.contains("FlutterSecureStorage") || line.contains(".write(") {
                continue;
            }
            let storage_context = match pattern {
                ".setString(" | ".setStringList(" => true,
                ".put(" => line.contains("Hive") || line.contains("box."),
                _ => line.contains("File(") || line.contains(".writeAs"),
            };
            if storage_context && has_secret_like_name(line) && !line.contains("HiveAesCipher") {
                candidates.push(detected(
                    path,
                    source,
                    index,
                    SecurityCategory::PlainSecretStorage,
                    "plain-local-storage",
                    SecurityConfidence::Medium,
                    pattern.trim_start_matches('.').trim_end_matches('('),
                ));
            }
        }
    }
}

fn detected(
    path: &Path,
    source: &str,
    index: usize,
    category: SecurityCategory,
    sink: &str,
    confidence: SecurityConfidence,
    expression: &str,
) -> DetectedSecurityCandidate {
    DetectedSecurityCandidate {
        category,
        sink: sink.to_owned(),
        confidence,
        occurrence: SecurityOccurrence {
            path: path.to_path_buf(),
            location: location_for_index(source, index),
            expression: expression.to_owned(),
            evidence: redact_line(line_at(source, index)),
            reachability: None,
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StringLiteral {
    index: usize,
    value_index: usize,
    value: String,
}

fn string_literals(source: &str) -> Vec<StringLiteral> {
    let mut literals = Vec::new();
    let mut index = 0;
    let bytes = source.as_bytes();
    while index < bytes.len() {
        if matches!(bytes[index], b'\'' | b'"') && !is_comment_match(source, index) {
            if let Some((value, value_index, end)) = read_string(source, index) {
                literals.push(StringLiteral {
                    index,
                    value_index,
                    value,
                });
                index = end;
                continue;
            }
        }
        index += 1;
    }
    literals
}

fn read_string(source: &str, start: usize) -> Option<(String, usize, usize)> {
    let bytes = source.as_bytes();
    let quote = *bytes.get(start)?;
    if bytes.get(start..start + 3) == Some([quote, quote, quote].as_slice()) {
        let mut index = start + 3;
        let value_start = index;
        while index + 2 < bytes.len() {
            if bytes.get(index..index + 3) == Some([quote, quote, quote].as_slice()) {
                return Some((
                    source[value_start..index].to_owned(),
                    value_start,
                    index + 3,
                ));
            }
            index += 1;
        }
        return None;
    }
    let mut index = start + 1;
    let value_start = index;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index = (index + 2).min(bytes.len());
            continue;
        }
        if bytes[index] == quote {
            return Some((
                source[value_start..index].to_owned(),
                value_start,
                index + 1,
            ));
        }
        index += 1;
    }
    None
}

fn match_indices(source: &str, pattern: &str) -> Vec<usize> {
    let mut matches = Vec::new();
    let mut offset = 0;
    while let Some(relative) = source[offset..].find(pattern) {
        let index = offset + relative;
        offset = index + pattern.len();
        if !is_comment_match(source, index) {
            matches.push(index);
        }
    }
    matches
}

fn call_inside_after(source: &str, start: usize) -> Option<String> {
    let bytes = source.as_bytes();
    let mut depth = 1usize;
    let mut index = start;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' | b'"' => {
                index = read_string(source, index)?.2;
                continue;
            }
            b'(' => depth += 1,
            b')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(source[start..index].to_owned());
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn first_call_arg_is_fixed_literal(call: &str) -> bool {
    let trimmed = call.trim_start();
    if !matches!(trimmed.as_bytes().first(), Some(b'\'' | b'"')) {
        return false;
    }
    let Some((_, _, end)) = read_string(trimmed, 0) else {
        return false;
    };
    let rest = trimmed[end..].trim_start();
    matches!(rest.as_bytes().first(), Some(b',' | b')'))
}

fn sql_call_is_parameterized(call: &str) -> bool {
    call.contains('?') && (call.contains('[') || call.contains("whereArgs"))
}

fn has_dynamic_text(text: &str) -> bool {
    text.contains('$') || text.contains(" + ") || text.contains("+ ") || text.contains(" +")
}

fn sql_like_text(text: &str) -> bool {
    let upper = text.to_ascii_uppercase();
    [
        "SELECT ", "INSERT ", "UPDATE ", "DELETE ", "CREATE ", "DROP ", "ALTER ", " WHERE ",
        " FROM ", " JOIN ",
    ]
    .iter()
    .any(|keyword| upper.contains(keyword))
}

fn has_network_context(line: &str) -> bool {
    [
        "Uri.parse",
        "http.",
        "Dio(",
        "BaseOptions",
        "Request(",
        "getUrl",
        "openUrl",
        "loadRequest",
    ]
    .iter()
    .any(|pattern| line.contains(pattern))
}

fn has_secret_like_name(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "secret",
        "token",
        "jwt",
        "privatekey",
        "private_key",
        "clientsecret",
        "client_secret",
        "refreshtoken",
        "refresh_token",
        "accesstoken",
        "access_token",
        "password",
        "passwd",
        "bearer",
        "authorization",
        "apikey",
        "api_key",
        "signingkey",
        "signing_key",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn firebase_api_key_context(source: &str, index: usize) -> bool {
    firebase_options_context(source, index) && named_argument_context(source, index, "apiKey")
}

fn firebase_options_context(source: &str, index: usize) -> bool {
    enclosing_call_name(source, index).is_some_and(|name| name == "FirebaseOptions")
}

fn concrete_firebase_api_key_literal(value: &str) -> bool {
    !value.trim().is_empty() && !value.contains('$')
}

fn firebase_secret_name_context(source: &str, index: usize) -> bool {
    literal_argument_context(source, index).is_some_and(has_secret_like_name)
}

fn named_argument_context(source: &str, index: usize, name: &str) -> bool {
    let Some(context) = literal_argument_context(source, index) else {
        return false;
    };
    let Some((candidate, rest)) = context.rsplit_once(':') else {
        return false;
    };
    rest.trim().is_empty()
        && trailing_identifier(candidate).is_some_and(|identifier| identifier == name)
}

fn google_app_id_context(source: &str, index: usize) -> bool {
    named_argument_context(source, index, "googleAppId")
        || assignment_context(source, index, "googleAppId")
}

fn assignment_context(source: &str, index: usize, name: &str) -> bool {
    let Some(context) = literal_context(source, index) else {
        return false;
    };
    let Some((candidate, rest)) = context.rsplit_once('=') else {
        return false;
    };
    rest.trim().is_empty()
        && trailing_identifier(candidate).is_some_and(|identifier| identifier == name)
}

fn literal_context(source: &str, index: usize) -> Option<&str> {
    literal_context_until_separator(source, index, true)
}

fn literal_argument_context(source: &str, index: usize) -> Option<&str> {
    literal_context_until_separator(source, index, false)
}

fn literal_context_until_separator(
    source: &str,
    index: usize,
    stop_at_newline: bool,
) -> Option<&str> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut cursor = index.min(bytes.len());
    while cursor > 0 {
        cursor -= 1;
        match bytes[cursor] {
            b')' | b']' | b'}' => depth += 1,
            b'(' | b'[' | b'{' => {
                if depth == 0 {
                    return trimmed_context(source, cursor + 1, index);
                }
                depth -= 1;
            }
            b',' | b';' if depth == 0 => {
                return trimmed_context(source, cursor + 1, index);
            }
            b'\n' if depth == 0 && stop_at_newline => {
                return trimmed_context(source, cursor + 1, index);
            }
            _ => {}
        }
    }
    trimmed_context(source, 0, index)
}

fn trimmed_context(source: &str, start: usize, end: usize) -> Option<&str> {
    source
        .get(start..end)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn enclosing_call_name(source: &str, index: usize) -> Option<&str> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut cursor = index.min(bytes.len());
    while cursor > 0 {
        cursor -= 1;
        match bytes[cursor] {
            b')' => depth += 1,
            b'(' => {
                if depth == 0 {
                    return preceding_identifier(source, cursor);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

fn preceding_identifier(source: &str, index: usize) -> Option<&str> {
    let bytes = source.as_bytes();
    let mut end = index.min(bytes.len());
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    let mut start = end;
    while start > 0
        && (bytes[start - 1].is_ascii_alphanumeric()
            || bytes[start - 1] == b'_'
            || bytes[start - 1] == b'.')
    {
        start -= 1;
    }
    source
        .get(start..end)
        .and_then(|name| name.rsplit('.').next())
        .filter(|name| !name.is_empty())
}

fn trailing_identifier(text: &str) -> Option<&str> {
    let trimmed = text.trim_end();
    let bytes = trimmed.as_bytes();
    let mut start = trimmed.len();
    while start > 0 && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_') {
        start -= 1;
    }
    trimmed.get(start..).filter(|name| !name.is_empty())
}

fn javascript_password_autofill_index(value: &str) -> Option<usize> {
    let lower = value.to_ascii_lowercase();
    if !lower.contains(".value") {
        return None;
    }
    let mut offset = 0;
    let mut previous_non_empty = None;
    for raw_line in value.split_inclusive('\n') {
        let line = raw_line.strip_suffix('\n').unwrap_or(raw_line);
        let line_lower = line.to_ascii_lowercase();
        if !line_lower.contains(".value") {
            if !line.trim().is_empty() {
                previous_non_empty = Some(line);
            }
            offset += raw_line.len();
            continue;
        }
        let Some(quote_index) =
            value_assignment_literal_index(line, &line_lower, previous_non_empty)
        else {
            if !line.trim().is_empty() {
                previous_non_empty = Some(line);
            }
            offset += raw_line.len();
            continue;
        };
        let Some((assigned, _, _)) = read_string(line, quote_index) else {
            if !line.trim().is_empty() {
                previous_non_empty = Some(line);
            }
            offset += raw_line.len();
            continue;
        };
        if assigned.len() >= 8 && !is_placeholder(&assigned) {
            return Some(offset + quote_index);
        }
        if !line.trim().is_empty() {
            previous_non_empty = Some(line);
        }
        offset += raw_line.len();
    }
    None
}

struct ValueAssignmentTarget<'a> {
    start: usize,
    text: &'a str,
}

fn value_assignment_literal_index(
    line: &str,
    line_lower: &str,
    previous_non_empty: Option<&str>,
) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut search_start = 0;
    while let Some(relative_index) = line_lower[search_start..].find(".value") {
        let value_index = search_start + relative_index;
        let Some(target) = value_assignment_target(line, value_index) else {
            search_start = value_index + ".value".len();
            continue;
        };
        let mut cursor = value_index + ".value".len();
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'=') {
            search_start = cursor.min(bytes.len());
            continue;
        }
        cursor += 1;
        if matches!(bytes.get(cursor), Some(b'=' | b'>')) {
            search_start = cursor;
            continue;
        }
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if matches!(bytes.get(cursor), Some(b'\'' | b'"'))
            && password_autofill_assignment_context(line_lower, previous_non_empty, &target)
        {
            return Some(cursor);
        }
        search_start = cursor;
    }
    None
}

fn value_assignment_target(line: &str, value_index: usize) -> Option<ValueAssignmentTarget<'_>> {
    let bytes = line.as_bytes();
    let mut cursor = value_index;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    while cursor > 0 {
        let byte = bytes[cursor - 1];
        if paren_depth > 0 {
            match byte {
                b')' => paren_depth += 1,
                b'(' => paren_depth -= 1,
                _ => {}
            }
            cursor -= 1;
            continue;
        }
        if bracket_depth > 0 {
            match byte {
                b']' => bracket_depth += 1,
                b'[' => bracket_depth -= 1,
                _ => {}
            }
            cursor -= 1;
            continue;
        }
        match byte {
            b')' => {
                paren_depth = 1;
                cursor -= 1;
            }
            b']' => {
                bracket_depth = 1;
                cursor -= 1;
            }
            byte if is_value_target_byte(byte) => cursor -= 1,
            _ => break,
        }
    }
    line.get(cursor..value_index)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(|text| ValueAssignmentTarget {
            start: value_index - text.len(),
            text,
        })
}

fn is_value_target_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$' | b'.' | b'?')
}

fn password_autofill_assignment_context(
    line_lower: &str,
    previous_non_empty: Option<&str>,
    target: &ValueAssignmentTarget<'_>,
) -> bool {
    let target_lower = target.text.to_ascii_lowercase();
    if value_target_has_password_context(&target_lower) {
        return true;
    }
    if line_lower
        .get(..target.start)
        .is_some_and(|context| context_ties_password_to_target(context, &target_lower))
    {
        return true;
    }
    previous_non_empty.is_some_and(|line| {
        let context = line.to_ascii_lowercase();
        context_ties_password_to_target(&context, &target_lower)
    })
}

fn value_target_has_password_context(target_lower: &str) -> bool {
    let terminal = terminal_value_target_segment(target_lower);
    let compact = compact_without_ascii_whitespace(terminal);
    !negative_password_input_hint(&compact) && terminal.contains("password")
}

fn terminal_value_target_segment(target_lower: &str) -> &str {
    let target = target_lower.trim();
    if let Some(segment) = target
        .ends_with(']')
        .then(|| matching_open_bracket(target).and_then(|start| target.get(start..)))
        .flatten()
    {
        return segment;
    }
    if let Some(index) = last_top_level_dot(target) {
        target.get(index + 1..).unwrap_or(target)
    } else {
        target
    }
}

fn matching_open_bracket(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    for index in (0..bytes.len()).rev() {
        match bytes[index] {
            b']' => depth += 1,
            b'[' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn last_top_level_dot(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    for index in (0..bytes.len()).rev() {
        match bytes[index] {
            b')' => paren_depth += 1,
            b'(' => paren_depth = paren_depth.saturating_sub(1),
            b']' => bracket_depth += 1,
            b'[' => bracket_depth = bracket_depth.saturating_sub(1),
            b'.' if paren_depth == 0 && bracket_depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

fn context_ties_password_to_target(context_lower: &str, target_lower: &str) -> bool {
    let mut search_start = 0;
    while let Some((start, end)) = target_reference_range(context_lower, target_lower, search_start)
    {
        if password_input_hint(target_reference_context(context_lower, start, end)) {
            return true;
        }
        search_start = end;
    }
    false
}

fn password_input_hint(context_lower: &str) -> bool {
    let compact = compact_without_ascii_whitespace(context_lower);
    if negative_password_input_hint(&compact) {
        return false;
    }
    positive_password_input_hint(&compact)
}

fn negative_password_input_hint(compact_context: &str) -> bool {
    [
        "type!=='password'",
        "type!==\"password\"",
        "type!==password",
        "type!='password'",
        "type!=\"password\"",
        "type!=password",
        ":not([type=password",
        ":not([type='password'",
        ":not([type=\"password\"",
        "not([type=password",
        "not([type='password'",
        "not([type=\"password\"",
    ]
    .iter()
    .any(|needle| compact_context.contains(needle))
}

fn compact_without_ascii_whitespace(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_ascii_whitespace())
        .collect()
}

fn positive_password_input_hint(compact_context: &str) -> bool {
    [
        "type==='password'",
        "type===\"password\"",
        "type=='password'",
        "type==\"password\"",
        "type=password",
        "type='password'",
        "type=\"password\"",
        "[type=password",
        "[type='password'",
        "[type=\"password\"",
    ]
    .iter()
    .any(|needle| compact_context.contains(needle))
}

fn target_reference_range(
    context_lower: &str,
    target_lower: &str,
    search_start: usize,
) -> Option<(usize, usize)> {
    let bounded_identifier = target_lower.bytes().all(is_identifier_byte);
    let mut cursor = search_start;
    while let Some(relative_index) = context_lower[cursor..].find(target_lower) {
        let start = cursor + relative_index;
        let end = start + target_lower.len();
        if !bounded_identifier || identifier_boundaries(context_lower, start, end) {
            return Some((start, end));
        }
        cursor = end;
    }
    None
}

fn identifier_boundaries(context_lower: &str, start: usize, end: usize) -> bool {
    let before_boundary = start == 0 || !is_identifier_byte(context_lower.as_bytes()[start - 1]);
    let after_boundary =
        end == context_lower.len() || !is_identifier_byte(context_lower.as_bytes()[end]);
    before_boundary && after_boundary
}

fn target_reference_context(context_lower: &str, start: usize, end: usize) -> &str {
    let context_start = logical_expression_start(context_lower, start);
    let context_end = logical_expression_end(context_lower, end);
    &context_lower[context_start..context_end]
}

fn logical_expression_start(context_lower: &str, start: usize) -> usize {
    let bytes = context_lower.as_bytes();
    let mut cursor = start;
    while cursor > 0 {
        if cursor >= 2 && bytes.get(cursor - 2..cursor) == Some(b"&&") {
            return cursor;
        }
        if cursor >= 2 && bytes.get(cursor - 2..cursor) == Some(b"||") {
            return cursor;
        }
        if matches!(bytes[cursor - 1], b';' | b'{' | b'}' | b'\n' | b',') {
            return cursor;
        }
        cursor -= 1;
    }
    0
}

fn logical_expression_end(context_lower: &str, end: usize) -> usize {
    let bytes = context_lower.as_bytes();
    let mut cursor = end;
    while cursor < bytes.len() {
        if bytes.get(cursor..cursor + 2) == Some(b"&&")
            || bytes.get(cursor..cursor + 2) == Some(b"||")
        {
            return cursor;
        }
        if matches!(bytes[cursor], b';' | b'{' | b'}' | b'\n' | b',') {
            return cursor;
        }
        cursor += 1;
    }
    bytes.len()
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')
}

fn is_module_uri_directive_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    ["import ", "export ", "part ", "part of "]
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
}

const RESET_OR_RECOVERY_PATH_MARKERS: &[&str] = &[
    "forgot-password",
    "reset-password",
    "passwordreset",
    "password-reset",
    "password-recovery",
    "recover-password",
];

fn benign_secret_named_literal(value: &str) -> bool {
    let route_or_reset_url =
        literal_looks_like_route_path(value) || literal_looks_like_reset_or_recovery_url(value);
    (route_or_reset_url
        && !literal_has_secret_like_url_parameter(value)
        && !literal_has_secret_like_reset_path_segment(value))
        || literal_looks_like_user_facing_copy(value)
}

fn literal_looks_like_route_path(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with('/')
        && trimmed.len() <= 128
        && !trimmed.contains(char::is_whitespace)
        && trimmed.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'/' | b'-' | b'_' | b'.' | b':' | b'?' | b'&' | b'=' | b'#'
                )
        })
}

fn literal_looks_like_reset_or_recovery_url(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://"))
        && RESET_OR_RECOVERY_PATH_MARKERS
            .iter()
            .any(|needle| lower.contains(needle))
}

fn literal_has_secret_like_url_parameter(value: &str) -> bool {
    let trimmed = value.trim();
    if let Some(query_index) = trimmed.find('?') {
        let query_start = query_index + 1;
        let query_end = trimmed[query_start..]
            .find('#')
            .map_or(trimmed.len(), |fragment_index| query_start + fragment_index);
        if segment_has_secret_like_url_parameter(&trimmed[query_start..query_end]) {
            return true;
        }
    }
    trimmed.find('#').is_some_and(|fragment_index| {
        segment_has_secret_like_url_parameter(&trimmed[fragment_index + 1..])
    })
}

fn segment_has_secret_like_url_parameter(segment: &str) -> bool {
    segment.split('&').any(|parameter| {
        let Some((name, value)) = parameter.split_once('=') else {
            return false;
        };
        has_secret_like_name(name) && concrete_url_parameter_value(value)
    })
}

fn concrete_url_parameter_value(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.len() < 8
        || trimmed.contains('$')
        || trimmed.starts_with(':')
        || (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('<') && trimmed.ends_with('>'))
    {
        return false;
    }
    !is_placeholder(trimmed)
}

fn literal_has_secret_like_reset_path_segment(value: &str) -> bool {
    let trimmed = value.trim();
    let path_end = trimmed.find(['?', '#']).unwrap_or(trimmed.len());
    let path = &trimmed[..path_end];
    let mut reset_or_recovery_path_seen = false;
    for segment in path.split('/') {
        let lower = segment.to_ascii_lowercase();
        if reset_or_recovery_path_seen && concrete_secret_like_path_segment(segment) {
            return true;
        }
        if RESET_OR_RECOVERY_PATH_MARKERS.contains(&lower.as_str()) {
            reset_or_recovery_path_seen = true;
        }
    }
    false
}

fn concrete_secret_like_path_segment(segment: &str) -> bool {
    let trimmed = segment.trim();
    if !concrete_url_parameter_value(trimmed) || trimmed.len() < 12 {
        return false;
    }
    let mut has_alpha = false;
    let mut has_digit = false;
    for byte in trimmed.bytes() {
        if byte.is_ascii_alphabetic() {
            has_alpha = true;
        } else if byte.is_ascii_digit() {
            has_digit = true;
        } else if !matches!(byte, b'-' | b'_' | b'.' | b'~') {
            return false;
        }
    }
    has_alpha && has_digit
}

fn literal_looks_like_user_facing_copy(value: &str) -> bool {
    let trimmed = value.trim();
    let lower = trimmed.to_ascii_lowercase();
    trimmed.len() <= 120
        && trimmed.contains(char::is_whitespace)
        && ["password", "secret", "token", "credential"]
            .iter()
            .any(|word| lower.contains(word))
        && [
            "change password",
            "invalid email or password",
            "forgot password",
            "reset password",
            "enter password",
            "confirm password",
            "password must",
            "password is",
            "token expired",
            "invalid token",
        ]
        .iter()
        .any(|phrase| lower.contains(phrase))
        && trimmed.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || character.is_ascii_whitespace()
                || matches!(
                    character,
                    '.' | ',' | '!' | '?' | ':' | ';' | '\'' | '"' | '-' | '/' | '(' | ')'
                )
        })
}

fn has_secret_shape(value: &str) -> bool {
    if value.contains('$') {
        return false;
    }
    value.contains("-----BEGIN ") && value.contains("PRIVATE KEY-----")
        || value.starts_with("Bearer ")
        || value.starts_with("sk_")
        || value.starts_with("ghp_")
        || jwt_like(value)
        || long_hex(value)
}

fn jwt_like(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| part.len() >= 10 && is_base64ish(part))
}

fn long_hex(value: &str) -> bool {
    value.len() >= 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_base64ish(value: &str) -> bool {
    value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'+' | b'/' | b'=')
    })
}

fn literal_looks_like_storage_key(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 64
        && trimmed
            .bytes()
            .all(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'-' | b'.'))
}

fn diagnostic_context(source: &str, index: usize) -> bool {
    let mut start = source[..index]
        .rfind('\n')
        .map_or(0, |position| position + 1);
    for _ in 0..2 {
        if start == 0 {
            break;
        }
        start = source[..start - 1]
            .rfind('\n')
            .map_or(0, |position| position + 1);
    }
    let end = source[index..]
        .find('\n')
        .map_or(source.len(), |position| index + position);
    line_is_diagnostic_text(&source[start..end])
}

fn line_is_diagnostic_text(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    [
        "_log.",
        "logger.",
        "log.",
        "print(",
        "debugprint(",
        "throw ",
        "exception(",
        "error(",
    ]
    .iter()
    .any(|pattern| lower.contains(pattern))
}

fn is_placeholder(value: &str) -> bool {
    let normalized = value
        .trim()
        .trim_matches(|character| matches!(character, '<' | '>' | '{' | '}'))
        .to_ascii_lowercase()
        .replace(['-', '_', ' '], "");
    normalized.is_empty()
        || normalized.chars().all(|character| character == 'x')
        || [
            "todo",
            "example",
            "dummy",
            "test",
            "changeme",
            "redacted",
            "yourapikey",
            "yourkeyhere",
            "yourtoken",
            "placeholder",
        ]
        .iter()
        .any(|placeholder| normalized.contains(placeholder))
}

fn is_local_http_url(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "http://localhost",
        "http://127.0.0.1",
        "http://[::1]",
        "http://::1",
        "http://10.0.2.2",
        "http://0.0.0.0",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

fn returns_true(text: &str) -> bool {
    text.contains("=> true") || text.contains("return true") || text.contains(": true")
}

fn returns_false(text: &str) -> bool {
    text.contains("=> false") || text.contains("return false") || text.contains(": false")
}

fn following_window(source: &str, index: usize, lines: usize) -> &str {
    let mut end = index;
    for _ in 0..lines {
        end = source[end..]
            .find('\n')
            .map_or(source.len(), |relative| end + relative + 1);
        if end == source.len() {
            break;
        }
    }
    &source[index..end]
}

fn line_at(source: &str, index: usize) -> &str {
    let start = source[..index]
        .rfind('\n')
        .map_or(0, |position| position + 1);
    let end = source[index..]
        .find('\n')
        .map_or(source.len(), |position| index + position);
    &source[start..end]
}

fn redact_line(line: &str) -> String {
    let mut redacted = String::new();
    let mut index = 0;
    let bytes = line.as_bytes();
    while index < bytes.len() {
        if matches!(bytes[index], b'\'' | b'"') {
            let quote = bytes[index] as char;
            redacted.push(quote);
            redacted.push_str("<redacted>");
            redacted.push(quote);
            if let Some((_, _, end)) = read_string(line, index) {
                index = end;
                continue;
            }
        }
        redacted.push(bytes[index] as char);
        index += 1;
    }
    redacted.trim().to_owned()
}

fn is_comment_match(source: &str, index: usize) -> bool {
    let line_start = source[..index]
        .rfind('\n')
        .map_or(0, |position| position + 1);
    if source[line_start..index].contains("//") {
        return true;
    }
    let last_block_open = source[..index].rfind("/*");
    let last_block_close = source[..index].rfind("*/");
    last_block_open.is_some_and(|open| last_block_close.is_none_or(|close| open > close))
}

fn location_for_index(source: &str, index: usize) -> Location {
    let line = source[..index]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1;
    let column = source[..index]
        .rfind('\n')
        .map_or(index, |position| index - position - 1);
    Location { line, column }
}
