use std::collections::BTreeSet;
use std::path::Path;

use tree_sitter::Node;

use super::catalogue::MatcherCatalogue;
use super::{
    DetectedSecurityCandidate, DetectionResult, LibraryImportContext, SecurityBlindSpot,
    SecurityBlindSpotReason, SecurityCategory, SecurityConfidence, SecurityOccurrence,
};
use crate::Location;
use crate::dart_parser::parse_dart_source_lossy;
use crate::generated::{is_flutterfire_options_path, is_generated_dart_path};

mod weak_randomness;

pub(super) fn detect_in_source(
    path: &Path,
    source: &str,
    catalogue: &MatcherCatalogue,
    inherited_imports: Option<&LibraryImportContext>,
) -> DetectionResult {
    let mut result = DetectionResult::default();
    detect_firebase_api_keys(path, source, &mut result.candidates);
    detect_hardcoded_secrets(path, source, &mut result.candidates);
    detect_insecure_transport(path, source, &mut result.candidates);
    detect_tls_bypass(path, source, catalogue, &mut result);
    detect_webview_risk(path, source, catalogue, &mut result.candidates);
    detect_process_execution(path, source, catalogue, inherited_imports, &mut result);
    detect_raw_sql(path, source, catalogue, &mut result);
    detect_plain_secret_storage(path, source, catalogue, &mut result.candidates);
    weak_randomness::detect(path, source, catalogue, &mut result);
    result
}

pub(super) fn is_ignored_path(path: &Path) -> bool {
    if path.components().any(|component| {
        let value = component.as_os_str().to_string_lossy();
        matches!(value.as_ref(), "test" | "integration_test" | "test_driver")
    }) {
        return true;
    }
    is_generated_dart_path(path) && !is_flutterfire_options_path(path)
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
        if oauth_endpoint_metadata_context(source, literal.index, &literal.value) {
            continue;
        }
        let stripe_secret = stripe_secret_prefix(&literal.value)
            || stripe_secret_name(&literal.value)
            || stripe_secret_binding_context(source, literal.index);
        if is_placeholder(&literal.value) && !stripe_secret {
            continue;
        }
        let line = line_at(source, literal.index);
        let binding_name = literal_binding_name(source, literal.index);
        let secret_value = has_secret_shape(&literal.value);
        let firebase_options_literal = firebase_options_context(source, literal.index);
        let secret_name = if firebase_options_literal {
            firebase_secret_name_context(source, literal.index)
        } else {
            binding_name
                .as_deref()
                .is_some_and(secret_binding_identifier)
        };
        let secret_binding_name = secret_name;
        let digest_literal = binding_name
            .as_deref()
            .is_some_and(non_secret_digest_identifier)
            && long_hex(&literal.value);
        let oauth_endpoint_credential = binding_name.as_deref().is_some_and(oauth_endpoint_name)
            && literal_has_embedded_url_credentials(&literal.value);
        if stripe_secret && !is_module_uri_directive_line(line) {
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
                "stripe-secret-signal",
            ));
            continue;
        }
        if is_module_uri_directive_line(line)
            || firebase_api_key_context(source, literal.index)
            || (!secret_value && benign_secret_named_literal(&literal.value, secret_binding_name))
            || (!secret_value && firebase_options_literal && !secret_name)
            || google_app_id_context(source, literal.index)
            || (!secret_value && literal_looks_like_storage_key(&literal.value))
            || (!secret_value && diagnostic_context(source, literal.index))
            || (!secret_value && literal.value.contains('$'))
            || digest_literal
        {
            continue;
        }
        if ((secret_binding_name
            || (secret_name
                && (literal_has_concrete_token_like_segment(&literal.value)
                    || literal_has_embedded_url_credentials(&literal.value))))
            && literal.value.len() >= 12)
            || (literal_looks_like_operational_copy(&literal.value)
                && literal_has_concrete_token_like_segment(&literal.value))
            || oauth_endpoint_credential
            || secret_value
        {
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
    if is_flutterfire_options_path(path) {
        return;
    }
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
        if has_network_context(line)
            || oauth_endpoint_url_context(source, literal.index, &literal.value)
        {
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
    for index in segment_indices(source, "Uri.http") {
        if debug_or_local_only_http_context(source, index) {
            continue;
        }
        let Some(open) = call_open_after(source, index + "Uri.http".len()) else {
            continue;
        };
        let Some(call) = call_inside_after(source, open + 1) else {
            continue;
        };
        let Some(authority) = top_level_call_arguments(&call)
            .first()
            .map(|value| value.trim())
        else {
            continue;
        };
        if fixed_literal_value(authority)
            .is_some_and(|value| value.is_empty() || is_local_http_authority(&value))
        {
            continue;
        }
        candidates.push(detected(
            path,
            source,
            index,
            SecurityCategory::InsecureTransport,
            "cleartext-http",
            SecurityConfidence::High,
            "Uri.http",
        ));
    }
    for index in segment_indices(source, "Uri") {
        if source[index + "Uri".len()..].starts_with('.') {
            continue;
        }
        let Some(open) = call_open_after(source, index + "Uri".len()) else {
            continue;
        };
        let Some(call) = call_inside_after(source, open + 1) else {
            continue;
        };
        if named_call_argument(&call, "scheme")
            .and_then(fixed_literal_value)
            .as_deref()
            != Some("http")
        {
            continue;
        }
        let Some(host) = named_call_argument(&call, "host") else {
            continue;
        };
        if fixed_literal_value(host)
            .is_some_and(|value| value.is_empty() || is_local_http_authority(&value))
        {
            continue;
        }
        candidates.push(detected(
            path,
            source,
            index,
            SecurityCategory::InsecureTransport,
            "cleartext-http",
            SecurityConfidence::High,
            "Uri",
        ));
    }
}

fn detect_tls_bypass(
    path: &Path,
    source: &str,
    catalogue: &MatcherCatalogue,
    result: &mut DetectionResult,
) {
    let Ok(parsed) = parse_dart_source_lossy(path, source) else {
        return;
    };
    let root = parsed.tree().root_node();
    let mut seen = BTreeSet::new();
    for pattern in &catalogue.by_detector("tls-bypass").callees {
        for index in segment_indices(source, pattern) {
            let Some((value, assignment)) = tls_callback_value(root, source, index, pattern) else {
                continue;
            };
            if !seen.insert((pattern.as_str(), assignment)) {
                continue;
            }
            match tls_callback_verdict(root, value, source) {
                TlsCallbackVerdict::Risky => result.candidates.push(detected(
                    path,
                    source,
                    index,
                    SecurityCategory::TlsBypass,
                    pattern,
                    SecurityConfidence::High,
                    pattern,
                )),
                TlsCallbackVerdict::Unknown => result.blind_spots.push(blind_spot(
                    path,
                    source,
                    index,
                    SecurityCategory::TlsBypass,
                    pattern,
                    SecurityBlindSpotReason::AmbiguousTlsCallback,
                )),
                TlsCallbackVerdict::AlwaysFalse => {}
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TlsCallbackVerdict {
    AlwaysFalse,
    Risky,
    Unknown,
}

fn tls_callback_value<'tree>(
    root: Node<'tree>,
    source: &str,
    index: usize,
    pattern: &str,
) -> Option<(Node<'tree>, usize)> {
    let mut node = root.descendant_for_byte_range(index, index.saturating_add(pattern.len()))?;
    loop {
        if node.kind() == "assignment_expression" {
            let left = node.child_by_field_name("left")?;
            let operator = node
                .child_by_field_name("operator")?
                .utf8_text(source.as_bytes())
                .ok()?;
            if operator == "="
                && segment_indices(left.utf8_text(source.as_bytes()).ok()?, pattern)
                    .into_iter()
                    .next()
                    .is_some()
            {
                return node
                    .child_by_field_name("right")
                    .map(|value| (value, node.start_byte()));
            }
        }
        if node.kind() == "named_argument"
            && named_argument_label(node, source).as_deref() == Some(pattern)
        {
            return named_argument_value(node).map(|value| (value, node.start_byte()));
        }
        if node.kind() == "cascade_section" {
            let mut cursor = node.walk();
            return node
                .named_children(&mut cursor)
                .filter(|child| child.kind() != "cascade_selector")
                .last()
                .map(|value| (value, node.start_byte()));
        }
        node = node.parent()?;
    }
}

fn tls_callback_verdict(root: Node<'_>, value: Node<'_>, source: &str) -> TlsCallbackVerdict {
    let text = value
        .utf8_text(source.as_bytes())
        .unwrap_or_default()
        .trim();
    if text == "null" {
        return TlsCallbackVerdict::AlwaysFalse;
    }
    if value.kind() == "function_expression" {
        return callable_boolean_verdict(root, value, source);
    }
    if is_identifier(text) {
        return resolved_callable_boolean_verdict(root, value, text, source);
    }
    let mut callbacks = Vec::new();
    collect_direct_descendants(value, "function_expression", &mut callbacks);
    if !callbacks.is_empty() {
        return combine_tls_verdicts(
            callbacks
                .into_iter()
                .map(|callback| callable_boolean_verdict(root, callback, source)),
        );
    }
    TlsCallbackVerdict::Unknown
}

fn resolved_callable_boolean_verdict(
    root: Node<'_>,
    value: Node<'_>,
    name: &str,
    source: &str,
) -> TlsCallbackVerdict {
    if let Some(callable) = enclosing_node(
        value,
        &[
            "function_declaration",
            "local_function_declaration",
            "method_declaration",
        ],
    ) && let Some(body) = direct_descendant(callable, &["function_body"])
    {
        let mut local = Vec::new();
        collect_named_callables(
            body,
            &["local_function_declaration"],
            name,
            source,
            &mut local,
        );
        if !local.is_empty() {
            return single_callable_boolean_verdict(root, &local, source);
        }
    }
    if let Some(class) = enclosing_node(value, &["class_declaration"]) {
        let mut methods = Vec::new();
        collect_named_callables(class, &["method_declaration"], name, source, &mut methods);
        if !methods.is_empty() {
            return single_callable_boolean_verdict(root, &methods, source);
        }
    }
    let mut top_level = Vec::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() == "function_declaration"
            && member_signature_name(child, source).as_deref() == Some(name)
        {
            top_level.push(child);
        }
    }
    single_callable_boolean_verdict(root, &top_level, source)
}

fn single_callable_boolean_verdict(
    root: Node<'_>,
    declarations: &[Node<'_>],
    source: &str,
) -> TlsCallbackVerdict {
    let [declaration] = declarations else {
        return TlsCallbackVerdict::Unknown;
    };
    callable_boolean_verdict(root, *declaration, source)
}

fn collect_named_callables<'tree>(
    node: Node<'tree>,
    kinds: &[&str],
    name: &str,
    source: &str,
    declarations: &mut Vec<Node<'tree>>,
) {
    if matches!(
        node.kind(),
        "function_declaration" | "local_function_declaration" | "method_declaration"
    ) {
        if kinds.contains(&node.kind())
            && member_signature_name(node, source).as_deref() == Some(name)
        {
            declarations.push(node);
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_named_callables(child, kinds, name, source, declarations);
    }
}

fn enclosing_node<'tree>(mut node: Node<'tree>, kinds: &[&str]) -> Option<Node<'tree>> {
    while let Some(parent) = node.parent() {
        if kinds.contains(&parent.kind()) {
            return Some(parent);
        }
        node = parent;
    }
    None
}

fn callable_boolean_verdict(
    root: Node<'_>,
    callable: Node<'_>,
    source: &str,
) -> TlsCallbackVerdict {
    let Some(body) = direct_descendant(callable, &["function_body", "function_expression_body"])
    else {
        return TlsCallbackVerdict::Unknown;
    };
    let mut returns = Vec::new();
    collect_callable_returns(body, &mut returns);
    if !returns.is_empty() {
        return combine_tls_verdicts(returns.into_iter().map(|statement| {
            let mut cursor = statement.walk();
            let expression = statement.named_children(&mut cursor).next();
            expression.map_or(TlsCallbackVerdict::Unknown, |expression| {
                boolean_expression_verdict(root, expression, source)
            })
        }));
    }
    let mut cursor = body.walk();
    body.named_children(&mut cursor)
        .last()
        .map_or(TlsCallbackVerdict::Unknown, |expression| {
            boolean_expression_verdict(root, expression, source)
        })
}

fn boolean_expression_verdict(
    root: Node<'_>,
    expression: Node<'_>,
    source: &str,
) -> TlsCallbackVerdict {
    let value = expression
        .utf8_text(source.as_bytes())
        .unwrap_or_default()
        .trim();
    match resolved_boolean_value(root, source, expression.start_byte(), value) {
        Some(false) => TlsCallbackVerdict::AlwaysFalse,
        Some(true) | None => TlsCallbackVerdict::Risky,
    }
}

fn collect_callable_returns<'tree>(node: Node<'tree>, returns: &mut Vec<Node<'tree>>) {
    if node.kind() == "return_statement" {
        returns.push(node);
        return;
    }
    if matches!(
        node.kind(),
        "function_declaration"
            | "function_expression"
            | "local_function_declaration"
            | "method_declaration"
    ) {
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_callable_returns(child, returns);
    }
}

fn collect_direct_descendants<'tree>(
    node: Node<'tree>,
    kind: &str,
    descendants: &mut Vec<Node<'tree>>,
) {
    if node.kind() == kind {
        descendants.push(node);
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_direct_descendants(child, kind, descendants);
    }
}

fn direct_descendant<'tree>(node: Node<'tree>, kinds: &[&str]) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).find_map(|child| {
        kinds
            .contains(&child.kind())
            .then_some(child)
            .or_else(|| direct_descendant(child, kinds))
    })
}

fn combine_tls_verdicts(
    verdicts: impl IntoIterator<Item = TlsCallbackVerdict>,
) -> TlsCallbackVerdict {
    let mut saw_value = false;
    let mut saw_unknown = false;
    for verdict in verdicts {
        saw_value = true;
        match verdict {
            TlsCallbackVerdict::Risky => return TlsCallbackVerdict::Risky,
            TlsCallbackVerdict::Unknown => saw_unknown = true,
            TlsCallbackVerdict::AlwaysFalse => {}
        }
    }
    if !saw_value || saw_unknown {
        TlsCallbackVerdict::Unknown
    } else {
        TlsCallbackVerdict::AlwaysFalse
    }
}

fn detect_webview_risk(
    path: &Path,
    source: &str,
    catalogue: &MatcherCatalogue,
    candidates: &mut Vec<DetectedSecurityCandidate>,
) {
    for pattern in catalogue
        .by_detector("webview-risk")
        .callees
        .iter()
        .filter(|pattern| !matches!(pattern.as_str(), "loadUrl" | "loadRequest"))
    {
        for index in segment_indices(source, pattern) {
            let line = line_at(source, index);
            if pattern != "JavaScriptMode.unrestricted" && !setting_is_true(line, pattern) {
                continue;
            }
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
    catalogue: &MatcherCatalogue,
    inherited_imports: Option<&LibraryImportContext>,
    result: &mut DetectionResult,
) {
    let matcher = catalogue.by_detector("process-execution");
    let Ok(parsed) = parse_dart_source_lossy(path, source) else {
        return;
    };
    let root = parsed.tree().root_node();
    let parsed_source = parsed.source();
    visit_named(root, &mut |node| {
        detect_process_call(
            path,
            root,
            parsed_source,
            matcher,
            inherited_imports,
            node,
            result,
        );
    });
}

fn detect_process_call(
    path: &Path,
    root: Node<'_>,
    source: &str,
    matcher: &super::catalogue::SecurityMatcher,
    inherited_imports: Option<&LibraryImportContext>,
    node: Node<'_>,
    result: &mut DetectionResult,
) {
    if node.kind() != "call_expression" {
        return;
    }
    let Some(function_node) = node.child_by_field_name("function") else {
        return;
    };
    let Ok(function) = function_node.utf8_text(source.as_bytes()) else {
        return;
    };
    let index = function_node.start_byte();
    let pattern = matcher
        .callees
        .iter()
        .find(|pattern| process_callee_matches(function, pattern))
        .map(String::as_str)
        .or_else(|| {
            resolved_package_process_pattern(root, source, index, function, inherited_imports)
        });
    let Some(pattern) = pattern else {
        if resolved_process_tearoff_pattern(
            root,
            source,
            index,
            function,
            matcher,
            inherited_imports,
        )
        .is_some()
        {
            push_process_blind_spot(
                path,
                source,
                index,
                SecurityBlindSpotReason::UnflattenedCall,
                result,
            );
        }
        return;
    };
    if !process_call_has_provenance(root, source, index, function, pattern, inherited_imports) {
        if is_part_file(root) || pattern.starts_with("processManager.") {
            push_process_blind_spot(
                path,
                source,
                index,
                SecurityBlindSpotReason::AmbiguousCallProvenance,
                result,
            );
        }
        return;
    }
    let Some((call, arguments)) = flattened_process_arguments(node, source) else {
        push_process_blind_spot(
            path,
            source,
            index,
            SecurityBlindSpotReason::UnflattenedCall,
            result,
        );
        return;
    };
    let package_process = pattern.starts_with("processManager.");
    let minimum_arguments = if package_process { 1 } else { 2 };
    if arguments.len() < minimum_arguments {
        push_process_blind_spot(
            path,
            source,
            index,
            SecurityBlindSpotReason::UnflattenedCall,
            result,
        );
        return;
    }
    let risky = if package_process {
        !parsed_run_in_shell_is_disabled(root, source, index)
            || process_manager_invokes_command_shell(root, source, index, arguments[0])
            || !fixed_argument_list(root, source, index, arguments[0])
    } else {
        let fixed_executable =
            resolved_fixed_literal_value(root, source, index, arguments[0]).is_some();
        let fixed_arguments = fixed_argument_list(root, source, index, arguments[1]);
        let fixed_dart_runtime = pattern == "Process.start"
            && resolved_dart_runtime_start(Some((root, source)), index, call, inherited_imports);
        !parsed_run_in_shell_is_disabled(root, source, index)
            || call_invokes_command_shell(root, source, index, call)
            || (!fixed_executable && !fixed_dart_runtime)
            || !fixed_arguments
    };
    if risky {
        result.candidates.push(detected(
            path,
            source,
            index,
            SecurityCategory::ProcessExecution,
            "process-exec",
            SecurityConfidence::High,
            pattern,
        ));
    }
}

fn flattened_process_arguments<'source>(
    node: Node<'_>,
    source: &'source str,
) -> Option<(&'source str, Vec<&'source str>)> {
    let arguments = node
        .child_by_field_name("arguments")?
        .utf8_text(source.as_bytes())
        .ok()?
        .strip_prefix('(')?
        .strip_suffix(')')?;
    Some((arguments, top_level_call_arguments(arguments)))
}

fn push_process_blind_spot(
    path: &Path,
    source: &str,
    index: usize,
    reason: SecurityBlindSpotReason,
    result: &mut DetectionResult,
) {
    result.blind_spots.push(blind_spot(
        path,
        source,
        index,
        SecurityCategory::ProcessExecution,
        "process-exec",
        reason,
    ));
}

fn resolved_process_tearoff_pattern(
    root: Node<'_>,
    source: &str,
    index: usize,
    function: &str,
    matcher: &super::catalogue::SecurityMatcher,
    inherited_imports: Option<&LibraryImportContext>,
) -> Option<String> {
    if !is_identifier(function) {
        return None;
    }
    let value = lexical_binding_value(root, source, index, function)?.trim();
    let pattern = matcher
        .callees
        .iter()
        .find(|pattern| process_callee_matches(value, pattern))
        .map(String::as_str)
        .or_else(|| {
            resolved_package_process_pattern(root, source, index, value, inherited_imports)
        })?;
    process_call_has_provenance(root, source, index, value, pattern, inherited_imports)
        .then(|| pattern.to_owned())
}

fn process_callee_matches(function: &str, pattern: &str) -> bool {
    function == pattern
        || pattern.starts_with("Process.")
            && function
                .strip_suffix(pattern)
                .is_some_and(|prefix| prefix.ends_with('.'))
}

fn resolved_package_process_pattern(
    root: Node<'_>,
    source: &str,
    index: usize,
    function: &str,
    inherited_imports: Option<&LibraryImportContext>,
) -> Option<&'static str> {
    let (receiver, method) = function.rsplit_once('.')?;
    if !is_identifier(receiver) || !matches!(method, "run" | "runSync" | "start") {
        return None;
    }
    let declared_type = lexical_binding_declared_type(root, source, index, receiver);
    let initializer_type =
        lexical_binding_value(root, source, index, receiver).and_then(constructor_reference);
    let process_manager = declared_type
        .as_deref()
        .or(initializer_type)
        .is_some_and(|type_name| {
            is_package_process_type_reference(root, source, index, type_name, inherited_imports)
        });
    if !process_manager {
        return None;
    }
    match method {
        "run" => Some("processManager.run"),
        "runSync" => Some("processManager.runSync"),
        "start" => Some("processManager.start"),
        _ => None,
    }
}

fn constructor_reference(value: &str) -> Option<&str> {
    let value = value.trim();
    let value = value
        .strip_prefix("const ")
        .or_else(|| value.strip_prefix("new "))
        .unwrap_or(value)
        .trim_start();
    let constructor = value.split_once('(')?.0.trim();
    (!constructor.is_empty()).then_some(constructor)
}

fn process_call_has_provenance(
    root: Node<'_>,
    source: &str,
    index: usize,
    function: &str,
    pattern: &str,
    inherited_imports: Option<&LibraryImportContext>,
) -> bool {
    if pattern.starts_with("Process.") {
        let Some(method) = pattern.strip_prefix("Process") else {
            return false;
        };
        let Some(process) = function.strip_suffix(method) else {
            return false;
        };
        return is_dart_io_type_reference(
            root,
            source,
            index,
            process,
            "Process",
            inherited_imports,
        );
    }
    pattern.starts_with("processManager.")
        && resolved_package_process_pattern(root, source, index, function, inherited_imports)
            == Some(pattern)
}

fn detect_raw_sql(
    path: &Path,
    source: &str,
    catalogue: &MatcherCatalogue,
    result: &mut DetectionResult,
) {
    let matcher = catalogue.by_detector("raw-sql");
    let parsed = parse_dart_source_lossy(path, source).ok();
    let root = parsed.as_ref().map(|parsed| parsed.tree().root_node());
    for pattern in matcher
        .callees
        .iter()
        .filter(|pattern| pattern.as_str() != "where:")
    {
        for index in segment_indices(source, pattern) {
            let Some(open) = call_open_after(source, index + pattern.len()) else {
                continue;
            };
            let Some(call) = call_inside_after(source, open + 1) else {
                result.blind_spots.push(blind_spot(
                    path,
                    source,
                    index,
                    SecurityCategory::RawSql,
                    "raw-sql",
                    SecurityBlindSpotReason::UnflattenedCall,
                ));
                continue;
            };
            let dynamic_text = root.map_or_else(
                || has_dynamic_text(&call),
                |root| sql_text_has_dynamic_input(root, source, index, &call),
            );
            if sql_call_is_parameterized(&call) && !dynamic_text {
                continue;
            }
            let risky = dynamic_text || !first_call_arg_is_fixed_literal(&call);
            let broad_execute = pattern == ".execute";
            if risky && (!broad_execute || sql_like_text(&call)) {
                result.candidates.push(detected(
                    path,
                    source,
                    index,
                    SecurityCategory::RawSql,
                    "raw-sql",
                    SecurityConfidence::High,
                    pattern.trim_start_matches('.'),
                ));
            }
        }
    }
    for index in segment_indices(source, "where:") {
        let line = line_at(source, index);
        let has_where_args =
            root.is_some_and(|root| call_has_named_argument(root, source, index, "whereArgs"));
        if has_dynamic_text(line) && !line.contains("whereArgs") && !has_where_args {
            result.candidates.push(detected(
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
    catalogue: &MatcherCatalogue,
    candidates: &mut Vec<DetectedSecurityCandidate>,
) {
    for pattern in &catalogue.by_detector("plain-secret-storage").callees {
        for index in segment_indices(source, pattern) {
            let line = line_at(source, index);
            if line.contains("FlutterSecureStorage") || line.contains(".write(") {
                continue;
            }
            let storage_context = match pattern.as_str() {
                ".setString" | ".setStringList" => plain_string_storage_receiver(source, index),
                ".put" => line.contains("Hive") || line.contains("box."),
                _ => line.contains("File(") || line.contains(".writeAs"),
            };
            let Some(open) = call_open_after(source, index + pattern.len()) else {
                continue;
            };
            let Some(call) = call_inside_after(source, open + 1) else {
                continue;
            };
            let arguments = top_level_call_arguments(&call);
            let (key, value) = match pattern.as_str() {
                ".writeAsString" | ".writeAsBytes" => {
                    ("", arguments.first().copied().unwrap_or(""))
                }
                _ => (
                    arguments.first().copied().unwrap_or(""),
                    arguments.get(1).copied().unwrap_or(""),
                ),
            };
            let secret_context = has_secret_like_name(key) || has_secret_like_name(value);
            if storage_context
                && secret_context
                && !benign_secret_storage_metadata(key, value)
                && !line.contains("HiveAesCipher")
            {
                candidates.push(detected(
                    path,
                    source,
                    index,
                    SecurityCategory::PlainSecretStorage,
                    "plain-local-storage",
                    SecurityConfidence::Medium,
                    pattern.trim_start_matches('.'),
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

fn blind_spot(
    path: &Path,
    source: &str,
    index: usize,
    category: SecurityCategory,
    sink: &str,
    reason: SecurityBlindSpotReason,
) -> SecurityBlindSpot {
    SecurityBlindSpot {
        category,
        path: path.to_path_buf(),
        location: location_for_index(source, index),
        sink: sink.to_owned(),
        reason,
        evidence: redact_line(line_at(source, index)),
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

fn segment_indices(source: &str, pattern: &str) -> Vec<usize> {
    match_indices(source, pattern)
        .into_iter()
        .filter(|index| {
            let before = index
                .checked_sub(1)
                .and_then(|position| source.as_bytes().get(position))
                .copied();
            let after = source.as_bytes().get(index + pattern.len()).copied();
            let starts_with_member = pattern.starts_with('.');
            let begins_with_identifier = pattern
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric);
            let ends_with_identifier = pattern
                .as_bytes()
                .last()
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_');
            (starts_with_member
                || !begins_with_identifier
                || before.is_none_or(|byte| !is_identifier_byte(byte)))
                && (!ends_with_identifier || after.is_none_or(|byte| !is_identifier_byte(byte)))
        })
        .collect()
}

fn call_open_after(source: &str, start: usize) -> Option<usize> {
    let relative = source
        .get(start..)?
        .find(|character: char| !character.is_whitespace())?;
    let index = start + relative;
    (source.as_bytes().get(index) == Some(&b'(')).then_some(index)
}

fn setting_is_true(line: &str, pattern: &str) -> bool {
    line.find(pattern).is_some_and(|index| {
        line[index + pattern.len()..]
            .trim_start()
            .starts_with(": true")
    })
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
    let mut rest = call.trim_start();
    let mut saw_literal = false;
    loop {
        let raw = rest
            .as_bytes()
            .get(0..2)
            .is_some_and(|prefix| matches!(prefix, [b'r' | b'R', b'\'' | b'"']));
        let quote_index = usize::from(raw);
        if !matches!(rest.as_bytes().get(quote_index), Some(b'\'' | b'"')) {
            return saw_literal
                && (rest.is_empty() || rest.starts_with(',') || rest.starts_with(')'));
        }
        let Some((_, _, end)) = read_string(rest, quote_index) else {
            return false;
        };
        saw_literal = true;
        rest = rest[end..].trim_start();
        if rest.is_empty() || rest.starts_with(',') || rest.starts_with(')') {
            return true;
        }
    }
}

fn call_invokes_command_shell(root: Node<'_>, source: &str, index: usize, call: &str) -> bool {
    let arguments = top_level_call_arguments(call);
    let Some(executable) = arguments
        .first()
        .and_then(|value| resolved_fixed_literal_value(root, source, index, value))
    else {
        return false;
    };
    let Some(shell_arguments) = arguments
        .get(1)
        .and_then(|value| resolved_list_arguments(root, source, index, value))
    else {
        return false;
    };
    fixed_command_invokes_shell(root, source, index, &executable, &shell_arguments)
}

fn process_manager_invokes_command_shell(
    root: Node<'_>,
    source: &str,
    index: usize,
    command: &str,
) -> bool {
    let Some(command) = resolved_list_arguments(root, source, index, command) else {
        return false;
    };
    let Some((executable, arguments)) = command.split_first() else {
        return false;
    };
    let Some(executable) = resolved_fixed_literal_value(root, source, index, executable) else {
        return false;
    };
    fixed_command_invokes_shell(root, source, index, &executable, arguments)
}

fn fixed_command_invokes_shell(
    root: Node<'_>,
    source: &str,
    index: usize,
    executable: &str,
    shell_arguments: &[String],
) -> bool {
    let executable = executable
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(executable)
        .to_ascii_lowercase();
    let posix_executable = executable.strip_suffix(".exe").unwrap_or(&executable);
    if !matches!(
        executable.as_str(),
        "cmd" | "cmd.exe" | "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe"
    ) && !command_interpreter_uses_c_flag(posix_executable)
    {
        return false;
    }
    for argument in shell_arguments {
        let Some(flag) = resolved_fixed_literal_value(root, source, index, argument)
            .map(|value| value.to_ascii_lowercase())
        else {
            return true;
        };
        let (invokes_command, is_option) = match executable.as_str() {
            "cmd" | "cmd.exe" => (
                matches!(flag.as_str(), "/c" | "/k"),
                flag.starts_with('/') || flag.starts_with('-'),
            ),
            "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe" => (
                matches!(flag.as_str(), "-c" | "-command" | "-encodedcommand"),
                flag.starts_with('-') || flag.starts_with('/'),
            ),
            _ if command_interpreter_uses_c_flag(posix_executable) => (
                posix_shell_flag_invokes_command(&flag),
                flag.starts_with('-'),
            ),
            _ => return false,
        };
        if invokes_command {
            return true;
        }
        if !is_option {
            return false;
        }
    }
    false
}

fn posix_shell_flag_invokes_command(flag: &str) -> bool {
    flag.strip_prefix('-')
        .is_some_and(|options| !options.starts_with('-') && options.contains('c'))
}

fn command_interpreter_uses_c_flag(executable: &str) -> bool {
    matches!(
        executable,
        "sh" | "ash"
            | "bash"
            | "csh"
            | "dash"
            | "elvish"
            | "fish"
            | "ksh"
            | "mksh"
            | "nu"
            | "osh"
            | "pdksh"
            | "rc"
            | "tcsh"
            | "xonsh"
            | "yash"
            | "ysh"
            | "zsh"
    )
}

fn fixed_literal_value(value: &str) -> Option<String> {
    let value = value.trim();
    let raw = value
        .as_bytes()
        .get(0..2)
        .is_some_and(|prefix| matches!(prefix, [b'r' | b'R', b'\'' | b'"']));
    let quote_index = usize::from(raw);
    read_string(value, quote_index)
        .filter(|(literal, _, end)| *end == value.len() && (raw || !literal.contains('$')))
        .map(|(literal, _, _)| literal)
}

fn resolved_dart_runtime_start(
    syntax: Option<(Node<'_>, &str)>,
    index: usize,
    call: &str,
    inherited_imports: Option<&LibraryImportContext>,
) -> bool {
    let Some((root, source)) = syntax else {
        return false;
    };
    let args = top_level_call_arguments(call);
    let Some(executable) = args.first().map(|arg| arg.trim()) else {
        return false;
    };
    let Some(arguments) = args.get(1).map(|arg| arg.trim()) else {
        return false;
    };
    (is_dart_io_platform_reference(root, source, index, executable, inherited_imports)
        || is_identifier(executable)
            && lexical_binding_value(root, source, index, executable).is_some_and(|value| {
                value == "Platform.resolvedExecutable"
                    && is_dart_io_platform_reference(root, source, index, value, inherited_imports)
            }))
        && fixed_argument_list(root, source, index, arguments)
}

fn is_dart_io_platform_reference(
    root: Node<'_>,
    source: &str,
    index: usize,
    value: &str,
    inherited_imports: Option<&LibraryImportContext>,
) -> bool {
    if library_declares_parts(root) && inherited_imports.is_none() {
        return false;
    }
    let Some(platform) = value.strip_suffix(".resolvedExecutable") else {
        return false;
    };
    is_dart_io_type_reference(root, source, index, platform, "Platform", inherited_imports)
}

fn is_dart_io_type_reference(
    root: Node<'_>,
    source: &str,
    index: usize,
    value: &str,
    type_name: &str,
    inherited_imports: Option<&LibraryImportContext>,
) -> bool {
    is_imported_type_reference(
        root,
        source,
        index,
        value,
        type_name,
        "dart:io",
        inherited_imports,
    )
}

fn is_package_process_type_reference(
    root: Node<'_>,
    source: &str,
    index: usize,
    value: &str,
    inherited_imports: Option<&LibraryImportContext>,
) -> bool {
    ["ProcessManager", "LocalProcessManager"]
        .into_iter()
        .any(|type_name| {
            is_imported_type_reference(
                root,
                source,
                index,
                value.trim_end_matches('?'),
                type_name,
                "package:process/process.dart",
                inherited_imports,
            )
        })
}

fn is_imported_type_reference(
    root: Node<'_>,
    source: &str,
    index: usize,
    value: &str,
    type_name: &str,
    import_uri: &str,
    inherited_imports: Option<&LibraryImportContext>,
) -> bool {
    let qualifier = if value == type_name {
        ""
    } else if let Some(qualifier) = value.strip_suffix(&format!(".{type_name}")) {
        qualifier
    } else {
        return false;
    };
    let prefix = (!qualifier.is_empty()).then_some(qualifier);
    if prefix.is_some_and(|prefix| !is_identifier(prefix)) {
        return false;
    }
    if prefix.is_none()
        && (top_level_declares_name(root, type_name, source)
            || lexical_name_shadowed(root, source, index, type_name)
            || import_prefix_declares_name(root, source, type_name))
    {
        return false;
    }
    if prefix.is_some_and(|prefix| {
        top_level_declares_name(root, prefix, source)
            || lexical_name_shadowed(root, source, index, prefix)
    }) {
        return false;
    }
    if let Some(inherited) = inherited_imports {
        if prefix.is_none()
            && (inherited.declared_names.contains(type_name)
                || inherited.import_prefixes.contains(type_name))
        {
            return false;
        }
        if prefix.is_some_and(|prefix| inherited.declared_names.contains(prefix)) {
            return false;
        }
    }
    let mut found = false;
    visit_named(root, &mut |node| {
        if found || node.kind() != "library_import" {
            return;
        }
        let Some(specification) = first_named_descendant(node, "import_specification") else {
            return;
        };
        let Some(uri) = specification
            .child_by_field_name("uri")
            .and_then(|uri| uri.utf8_text(source.as_bytes()).ok())
            .and_then(fixed_literal_value)
        else {
            return;
        };
        if uri != import_uri {
            return;
        }
        let alias = specification
            .child_by_field_name("alias")
            .and_then(|alias| alias.utf8_text(source.as_bytes()).ok());
        if alias != prefix {
            return;
        }
        if import_exposes_name(node, type_name, source) {
            found = true;
        }
    });
    if found {
        return true;
    }
    let Some(inherited) = inherited_imports else {
        return false;
    };
    inherited.exposes(import_uri, type_name, prefix)
}

fn import_exposes_name(node: Node<'_>, name: &str, source: &str) -> bool {
    let mut visible = true;
    let mut combinators = Vec::new();
    collect_named_descendants(node, "combinator", &mut combinators);
    for combinator in combinators {
        let Ok(text) = combinator.utf8_text(source.as_bytes()) else {
            return false;
        };
        let text = text.trim_start();
        let mut identifiers = Vec::new();
        collect_named_descendants(combinator, "identifier", &mut identifiers);
        let contains_name = identifiers
            .into_iter()
            .any(|identifier| identifier.utf8_text(source.as_bytes()).ok() == Some(name));
        if text.starts_with("show") {
            visible = contains_name;
        } else if text.starts_with("hide") && contains_name {
            visible = false;
        }
    }
    visible
}

fn lexical_name_shadowed(root: Node<'_>, source: &str, index: usize, name: &str) -> bool {
    let Some(mut child) = root.descendant_for_byte_range(index, index.saturating_add(1)) else {
        return true;
    };
    while let Some(parent) = child.parent() {
        if process_scope_parameters_bind_name(parent, name, source)
            || process_scope_header_binds_name(parent, child, name, source)
        {
            return true;
        }
        let mut cursor = parent.walk();
        if parent
            .named_children(&mut cursor)
            .take_while(|sibling| sibling.start_byte() < child.start_byte())
            .any(|sibling| direct_binding(sibling, name, source, false).is_some())
        {
            return true;
        }
        child = parent;
    }
    false
}

fn top_level_declares_name(root: Node<'_>, name: &str, source: &str) -> bool {
    let mut cursor = root.walk();
    root.named_children(&mut cursor).any(|child| {
        if direct_binding(child, name, source, false).is_some() {
            return true;
        }
        top_level_declaration_name(child, source).as_deref() == Some(name)
    })
}

fn top_level_declaration_name(node: Node<'_>, source: &str) -> Option<String> {
    if !matches!(
        node.kind(),
        "class_declaration"
            | "enum_declaration"
            | "extension_declaration"
            | "extension_type_declaration"
            | "external_function_declaration"
            | "external_getter_declaration"
            | "external_setter_declaration"
            | "function_declaration"
            | "getter_declaration"
            | "mixin_declaration"
            | "setter_declaration"
            | "type_alias"
    ) {
        return None;
    }
    if matches!(
        node.kind(),
        "external_function_declaration"
            | "external_getter_declaration"
            | "external_setter_declaration"
            | "function_declaration"
            | "getter_declaration"
            | "setter_declaration"
    ) {
        return member_signature_name(node, source);
    }
    node.child_by_field_name("name")
        .or_else(|| {
            if node.kind() == "type_alias" {
                first_named_descendant(node, "identifier")
                    .or_else(|| first_named_descendant(node, "type_identifier"))
            } else {
                None
            }
        })
        .and_then(|name| name.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn is_part_file(root: Node<'_>) -> bool {
    let mut is_part = false;
    visit_named(root, &mut |node| {
        is_part |= node.kind() == "part_of_directive";
    });
    is_part
}

fn library_declares_parts(root: Node<'_>) -> bool {
    let mut declares_parts = false;
    visit_named(root, &mut |node| {
        declares_parts |= node.kind() == "part_directive";
    });
    declares_parts
}

fn import_prefix_declares_name(root: Node<'_>, source: &str, name: &str) -> bool {
    let mut found = false;
    visit_named(root, &mut |node| {
        if found || node.kind() != "library_import" {
            return;
        }
        let Some(specification) = first_named_descendant(node, "import_specification") else {
            return;
        };
        if specification
            .child_by_field_name("alias")
            .and_then(|alias| alias.utf8_text(source.as_bytes()).ok())
            == Some(name)
        {
            found = true;
        }
    });
    found
}

fn visit_named(node: Node<'_>, visit: &mut impl FnMut(Node<'_>)) {
    visit(node);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visit_named(child, visit);
    }
}

fn collect_named_descendants<'tree>(node: Node<'tree>, kind: &str, nodes: &mut Vec<Node<'tree>>) {
    if node.kind() == kind {
        nodes.push(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_named_descendants(child, kind, nodes);
    }
}

fn first_named_descendant<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    if node.kind() == kind {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(|child| first_named_descendant(child, kind))
}

fn top_level_call_arguments(call: &str) -> Vec<&str> {
    let mut arguments = Vec::new();
    let mut start = 0usize;
    let mut index = 0usize;
    let mut depth = 0usize;
    let bytes = call.as_bytes();
    while index < bytes.len() {
        match bytes[index] {
            b'\'' | b'"' => {
                let Some((_, _, end)) = read_string(call, index) else {
                    return Vec::new();
                };
                index = end;
                continue;
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                arguments.push(&call[start..index]);
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    arguments.push(&call[start..]);
    arguments
}

fn named_call_argument<'call>(call: &'call str, name: &str) -> Option<&'call str> {
    top_level_call_arguments(call)
        .into_iter()
        .find_map(|argument| {
            let (label, value) = argument.split_once(':')?;
            (label.trim() == name).then_some(value.trim())
        })
}

fn call_has_named_argument(root: Node<'_>, source: &str, index: usize, name: &str) -> bool {
    let Some(call) = call_expression_at(root, index) else {
        return false;
    };
    let Some(arguments) = call.child_by_field_name("arguments") else {
        return false;
    };
    let mut cursor = arguments.walk();
    arguments.named_children(&mut cursor).any(|argument| {
        argument.kind() == "named_argument"
            && named_argument_label(argument, source).as_deref() == Some(name)
    })
}

fn parsed_run_in_shell_is_disabled(root: Node<'_>, source: &str, index: usize) -> bool {
    let Some(call) = call_expression_at(root, index) else {
        return false;
    };
    let Some(arguments) = call.child_by_field_name("arguments") else {
        return false;
    };
    let mut cursor = arguments.walk();
    for argument in arguments.named_children(&mut cursor) {
        if argument.kind() != "named_argument"
            || named_argument_label(argument, source).as_deref() != Some("runInShell")
        {
            continue;
        }
        return named_argument_value(argument)
            .and_then(|value| value.utf8_text(source.as_bytes()).ok())
            .is_some_and(|value| {
                resolved_boolean_value(root, source, index, value) == Some(false)
            });
    }
    true
}

fn resolved_boolean_value(root: Node<'_>, source: &str, index: usize, value: &str) -> Option<bool> {
    let mut value = value.trim();
    let mut visited = BTreeSet::new();
    loop {
        match value {
            "false" => return Some(false),
            "true" => return Some(true),
            _ if is_identifier(value) && visited.insert(value.to_owned()) => {
                value = lexical_binding_value(root, source, index, value)?.trim();
            }
            _ => return None,
        }
    }
}

fn call_expression_at(root: Node<'_>, index: usize) -> Option<Node<'_>> {
    let mut node = root.descendant_for_byte_range(index, index.saturating_add(1))?;
    loop {
        if node.kind() == "call_expression" && node.start_byte() <= index && node.end_byte() > index
        {
            return Some(node);
        }
        node = node.parent()?;
    }
}

fn named_argument_label(node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == "label")
        .and_then(|label| label.utf8_text(source.as_bytes()).ok())
        .map(str::trim)
        .and_then(|label| label.strip_suffix(':'))
        .map(str::to_owned)
}

fn named_argument_value(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() != "label")
}

fn fixed_argument_list(root: Node<'_>, source: &str, index: usize, arguments: &str) -> bool {
    let Some(elements) = resolved_list_arguments(root, source, index, arguments) else {
        return false;
    };
    elements.into_iter().all(|argument| {
        let argument = argument.trim();
        argument.is_empty()
            || fixed_string_literal(argument)
            || is_identifier(argument)
                && lexical_binding_value(root, source, index, argument)
                    .is_some_and(fixed_string_literal)
    })
}

fn list_literal_contents(value: &str) -> Option<&str> {
    let value = value.trim();
    let value = value.strip_prefix("const").map_or(value, str::trim_start);
    let start = value.find('[')?;
    if !value[..start].trim().is_empty() && !value[..start].trim().ends_with('>') {
        return None;
    }
    value[start + 1..].strip_suffix(']')
}

fn resolved_fixed_literal_value(
    root: Node<'_>,
    source: &str,
    index: usize,
    value: &str,
) -> Option<String> {
    let mut visited = BTreeSet::new();
    resolved_fixed_literal_value_inner(root, source, index, value, &mut visited)
}

fn resolved_fixed_literal_value_inner(
    root: Node<'_>,
    source: &str,
    index: usize,
    value: &str,
    visited: &mut BTreeSet<String>,
) -> Option<String> {
    if let Some(literal) = fixed_literal_value(value) {
        return Some(literal);
    }
    let name = value.trim();
    if !is_identifier(name) || !visited.insert(name.to_owned()) {
        return None;
    }
    let binding = lexical_binding_value(root, source, index, name)?;
    resolved_fixed_literal_value_inner(root, source, index, binding, visited)
}

fn resolved_list_arguments(
    root: Node<'_>,
    source: &str,
    index: usize,
    value: &str,
) -> Option<Vec<String>> {
    let mut visited = BTreeSet::new();
    resolved_list_arguments_inner(root, source, index, value, &mut visited)
}

fn resolved_list_arguments_inner(
    root: Node<'_>,
    source: &str,
    index: usize,
    value: &str,
    visited: &mut BTreeSet<String>,
) -> Option<Vec<String>> {
    let value = value.trim();
    if let Some(inner) = list_literal_contents(value) {
        return Some(
            top_level_call_arguments(inner)
                .into_iter()
                .map(str::trim)
                .map(str::to_owned)
                .collect(),
        );
    }
    if !is_identifier(value) || !visited.insert(value.to_owned()) {
        return None;
    }
    let binding = lexical_binding_value(root, source, index, value)?;
    resolved_list_arguments_inner(root, source, index, binding, visited)
}

fn fixed_string_literal(value: &str) -> bool {
    fixed_literal_value(value).is_some()
}

fn lexical_binding_value<'source>(
    root: Node<'_>,
    source: &'source str,
    index: usize,
    name: &str,
) -> Option<&'source str> {
    let mut child = root.descendant_for_byte_range(index, index.saturating_add(1))?;
    while let Some(parent) = child.parent() {
        if parent.kind() == "class_body" {
            if let Some(binding) = class_binding(parent, child, name, source) {
                return match binding {
                    LexicalBinding::Value(value) => Some(value),
                    LexicalBinding::Unknown => None,
                };
            }
        }
        if process_scope_parameters_bind_name(parent, name, source)
            || process_scope_header_binds_name(parent, child, name, source)
        {
            return None;
        }
        if parent.parent().is_none() {
            return top_level_binding(parent, name, source);
        }
        let mut cursor = parent.walk();
        let mut siblings = parent
            .named_children(&mut cursor)
            .take_while(|sibling| sibling.start_byte() < child.start_byte())
            .collect::<Vec<_>>();
        while let Some(sibling) = siblings.pop() {
            if let Some(binding) = direct_binding(sibling, name, source, false) {
                return match binding {
                    LexicalBinding::Value(value) => Some(value),
                    LexicalBinding::Unknown => None,
                };
            }
        }
        child = parent;
    }
    None
}

fn lexical_binding_declared_type(
    root: Node<'_>,
    source: &str,
    index: usize,
    name: &str,
) -> Option<String> {
    let mut child = root.descendant_for_byte_range(index, index.saturating_add(1))?;
    while let Some(parent) = child.parent() {
        if let Some(type_name) = parameter_binding_type(parent, name, source) {
            return Some(type_name);
        }
        if parent.kind() == "class_body" {
            let mut cursor = parent.walk();
            return parent
                .named_children(&mut cursor)
                .filter(|member| member.start_byte() != child.start_byte())
                .find_map(|member| direct_declared_binding_type(member, name, source));
        }
        if parent.parent().is_none() {
            let mut cursor = parent.walk();
            return parent
                .named_children(&mut cursor)
                .find_map(|node| direct_declared_binding_type(node, name, source));
        }
        let mut cursor = parent.walk();
        let mut siblings = parent
            .named_children(&mut cursor)
            .take_while(|sibling| sibling.start_byte() < child.start_byte())
            .collect::<Vec<_>>();
        while let Some(sibling) = siblings.pop() {
            if let Some(type_name) = direct_declared_binding_type(sibling, name, source) {
                return Some(type_name);
            }
            if direct_binding(sibling, name, source, false).is_some() {
                return None;
            }
        }
        child = parent;
    }
    None
}

fn parameter_binding_type(node: Node<'_>, name: &str, source: &str) -> Option<String> {
    let parameter_lists = match node.kind() {
        "function_expression" => parameter_field_lists(node),
        "method_declaration" => node
            .child_by_field_name("signature")
            .map_or_else(Vec::new, collect_parameter_lists),
        "declaration" | "function_declaration" | "local_function_declaration" => {
            direct_parameterized_signature(node).map_or_else(Vec::new, collect_parameter_lists)
        }
        _ => Vec::new(),
    };
    for parameters in parameter_lists {
        let mut candidates = Vec::new();
        collect_named_descendants(parameters, "formal_parameter", &mut candidates);
        for parameter in candidates {
            if formal_parameter_name(parameter, source).as_deref() == Some(name) {
                return declared_type_before_binding(
                    parameter.utf8_text(source.as_bytes()).ok()?,
                    name,
                );
            }
        }
    }
    None
}

fn direct_declared_binding_type(node: Node<'_>, name: &str, source: &str) -> Option<String> {
    direct_binding(node, name, source, false)?;
    declared_type_before_binding(node.utf8_text(source.as_bytes()).ok()?, name)
}

fn declared_type_before_binding(text: &str, name: &str) -> Option<String> {
    let name_index = segment_indices(text, name).into_iter().next()?;
    let before_name = text[..name_index].trim();
    before_name
        .split_whitespace()
        .map(|token| {
            token.trim_matches(|character: char| {
                !character.is_ascii_alphanumeric()
                    && !matches!(character, '_' | '$' | '.' | '<' | '>' | '?' | ',')
            })
        })
        .rfind(|token| {
            !token.is_empty()
                && !matches!(
                    *token,
                    "const"
                        | "covariant"
                        | "external"
                        | "final"
                        | "late"
                        | "required"
                        | "static"
                        | "super"
                        | "this"
                        | "var"
                )
        })
        .map(str::to_owned)
}

fn top_level_binding<'source>(
    root: Node<'_>,
    name: &str,
    source: &'source str,
) -> Option<&'source str> {
    let mut cursor = root.walk();
    let mut binding = None;
    for child in root.named_children(&mut cursor) {
        let Some(candidate) = direct_binding(child, name, source, false) else {
            continue;
        };
        if binding.is_some() {
            return None;
        }
        binding = Some(candidate);
    }
    match binding {
        Some(LexicalBinding::Value(value)) => Some(value),
        Some(LexicalBinding::Unknown) | None => None,
    }
}

fn class_binding<'source>(
    class_body: Node<'_>,
    current_member: Node<'_>,
    name: &str,
    source: &'source str,
) -> Option<LexicalBinding<'source>> {
    let mut cursor = class_body.walk();
    let direct = class_body
        .named_children(&mut cursor)
        .filter(|member| member.start_byte() != current_member.start_byte())
        .find_map(|member| class_member_binding(member, name, source));
    if direct.is_some() {
        return direct;
    }
    let class = class_body.parent()?;
    let mut visited = BTreeSet::new();
    let root = root_node(class);
    inherited_class_binding(root, class, name, source, &mut visited).or_else(|| {
        if name.starts_with('_') && !is_part_file(root) && !library_declares_parts(root) {
            return None;
        }
        let mut visited = BTreeSet::new();
        class_hierarchy_is_unresolved(root, class, source, &mut visited)
            .then_some(LexicalBinding::Unknown)
    })
}

fn class_member_binding<'source>(
    member: Node<'_>,
    name: &str,
    source: &'source str,
) -> Option<LexicalBinding<'source>> {
    direct_binding(member, name, source, false).or_else(|| {
        class_member_declares_name(member, name, source).then_some(LexicalBinding::Unknown)
    })
}

fn class_member_declares_name(member: Node<'_>, name: &str, source: &str) -> bool {
    let mut cursor = member.walk();
    member
        .named_children(&mut cursor)
        .filter(|child| matches!(child.kind(), "declaration" | "method_declaration"))
        .any(|child| member_signature_name(child, source).as_deref() == Some(name))
}

fn member_signature_name(node: Node<'_>, source: &str) -> Option<String> {
    if matches!(
        node.kind(),
        "function_signature" | "getter_signature" | "setter_signature"
    ) {
        return node
            .child_by_field_name("name")?
            .utf8_text(source.as_bytes())
            .ok()
            .map(str::to_owned);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(|child| member_signature_name(child, source))
}

fn inherited_class_binding<'source>(
    root: Node<'_>,
    class: Node<'_>,
    name: &str,
    source: &'source str,
    visited: &mut BTreeSet<String>,
) -> Option<LexicalBinding<'source>> {
    for mixin_name in class_mixin_names(class, source).into_iter().rev() {
        if !visited.insert(mixin_name.clone()) {
            continue;
        }
        let mixin = find_class_like_declaration(root, &mixin_name, source)?;
        let body = mixin.child_by_field_name("body")?;
        let mut cursor = body.walk();
        if let Some(binding) = body
            .named_children(&mut cursor)
            .find_map(|member| class_member_binding(member, name, source))
        {
            return Some(binding);
        }
        if let Some(binding) = inherited_class_binding(root, mixin, name, source, visited) {
            return Some(binding);
        }
    }
    let parent_name = class_superclass_name(class, source)?;
    if !visited.insert(parent_name.clone()) {
        return None;
    }
    let parent = find_class_like_declaration(root, &parent_name, source)?;
    let body = parent.child_by_field_name("body")?;
    let mut cursor = body.walk();
    if let Some(binding) = body
        .named_children(&mut cursor)
        .find_map(|member| class_member_binding(member, name, source))
    {
        return Some(binding);
    }
    inherited_class_binding(root, parent, name, source, visited)
}

fn class_hierarchy_is_unresolved(
    root: Node<'_>,
    class: Node<'_>,
    source: &str,
    visited: &mut BTreeSet<String>,
) -> bool {
    for mixin_name in class_mixin_names(class, source) {
        if !visited.insert(mixin_name.clone()) {
            return true;
        }
        let Some(mixin) = find_class_like_declaration(root, &mixin_name, source) else {
            return true;
        };
        if class_hierarchy_is_unresolved(root, mixin, source, visited) {
            return true;
        }
    }
    let Some(parent_name) = class_superclass_name(class, source) else {
        return false;
    };
    if !visited.insert(parent_name.clone()) {
        return true;
    }
    if parent_name == "Object" {
        return false;
    }
    let Some(parent) = find_class_like_declaration(root, &parent_name, source) else {
        return true;
    };
    class_hierarchy_is_unresolved(root, parent, source, visited)
}

fn class_mixin_names(class: Node<'_>, source: &str) -> Vec<String> {
    let Some(superclass) = class.child_by_field_name("superclass") else {
        return Vec::new();
    };
    let mixins = if superclass.kind() == "mixins" {
        Some(superclass)
    } else {
        let mut cursor = superclass.walk();
        superclass
            .named_children(&mut cursor)
            .find(|child| child.kind() == "mixins")
    };
    let Some(mixins) = mixins else {
        return Vec::new();
    };
    let mut cursor = mixins.walk();
    mixins
        .named_children(&mut cursor)
        .filter(|child| child.kind() == "type")
        .filter_map(|child| child.utf8_text(source.as_bytes()).ok())
        .map(|name| name.split('<').next().unwrap_or(name).trim().to_owned())
        .collect()
}

fn class_superclass_name(class: Node<'_>, source: &str) -> Option<String> {
    let superclass = class.child_by_field_name("superclass")?;
    let name = superclass
        .child_by_field_name("type")?
        .utf8_text(source.as_bytes())
        .ok()?;
    let name = name.split('<').next()?.trim();
    Some(name.to_owned())
}

fn find_class_like_declaration<'tree>(
    node: Node<'tree>,
    name: &str,
    source: &str,
) -> Option<Node<'tree>> {
    if matches!(node.kind(), "class_declaration" | "mixin_declaration")
        && node
            .child_by_field_name("name")
            .and_then(|name| name.utf8_text(source.as_bytes()).ok())
            == Some(name)
    {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(|child| find_class_like_declaration(child, name, source))
}

fn root_node(mut node: Node<'_>) -> Node<'_> {
    while let Some(parent) = node.parent() {
        node = parent;
    }
    node
}

enum LexicalBinding<'source> {
    Value(&'source str),
    Unknown,
}

fn direct_binding<'source>(
    node: Node<'_>,
    name: &str,
    source: &'source str,
    immutable: bool,
) -> Option<LexicalBinding<'source>> {
    let immutable = immutable || declaration_is_immutable(node);
    if node.kind() == "pattern_variable_declaration" {
        let mut cursor = node.walk();
        let pattern = node.named_children(&mut cursor).next()?;
        return pattern_identifier_binds_name(pattern, name, source)
            .then_some(LexicalBinding::Unknown);
    }
    if matches!(
        node.kind(),
        "formal_parameter" | "typed_identifier" | "variable_pattern"
    ) {
        let binding_name = node
            .child_by_field_name("name")?
            .utf8_text(source.as_bytes())
            .ok()?;
        return (binding_name == name).then_some(LexicalBinding::Unknown);
    }
    if matches!(
        node.kind(),
        "initialized_identifier" | "initialized_variable_definition" | "static_final_declaration"
    ) {
        let binding_name = node
            .child_by_field_name("name")?
            .utf8_text(source.as_bytes())
            .ok()?;
        if binding_name != name {
            return None;
        }
        if !immutable {
            return Some(LexicalBinding::Unknown);
        }
        return Some(
            node.child_by_field_name("value")
                .and_then(|value| value.utf8_text(source.as_bytes()).ok())
                .map(str::trim)
                .map_or(LexicalBinding::Unknown, LexicalBinding::Value),
        );
    }
    if node.kind() == "assignment_expression" {
        let left = node
            .child_by_field_name("left")?
            .utf8_text(source.as_bytes())
            .ok()?;
        let operator = node
            .child_by_field_name("operator")?
            .utf8_text(source.as_bytes())
            .ok()?;
        if left.trim() == name && operator == "=" {
            return Some(LexicalBinding::Unknown);
        }
        return None;
    }
    if !matches!(
        node.kind(),
        "class_member"
            | "declaration"
            | "expression_statement"
            | "initialized_identifier_list"
            | "local_variable_declaration"
            | "static_final_declaration_list"
            | "top_level_variable_declaration"
    ) {
        return None;
    }
    let mut cursor = node.walk();
    let mut children = node.named_children(&mut cursor).collect::<Vec<_>>();
    while let Some(child) = children.pop() {
        if let Some(binding) = direct_binding(child, name, source, immutable) {
            return Some(binding);
        }
    }
    None
}

fn declaration_is_immutable(node: Node<'_>) -> bool {
    if node.kind() == "static_final_declaration_list" {
        return true;
    }
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|child| matches!(child.kind(), "const" | "final"))
}

fn process_scope_header_binds_name(
    node: Node<'_>,
    path_child: Node<'_>,
    name: &str,
    source: &str,
) -> bool {
    if node.kind() == "catch_clause" {
        return catch_clause_binds_name(node, name, source);
    }
    if node.kind() == "try_statement" {
        let mut cursor = node.walk();
        let preceding = node
            .named_children(&mut cursor)
            .take_while(|child| child.start_byte() < path_child.start_byte())
            .collect::<Vec<_>>();
        return preceding
            .last()
            .filter(|child| child.kind() == "catch_clause")
            .is_some_and(|clause| catch_clause_binds_name(*clause, name, source));
    }
    match node.kind() {
        "for_element" | "for_statement" => {
            let Some(body) = node.child_by_field_name("body") else {
                return false;
            };
            if !node_contains(body, path_child) {
                return false;
            }
            if node
                .child_by_field_name("name")
                .is_some_and(|binding| binding.utf8_text(source.as_bytes()).ok() == Some(name))
            {
                return true;
            }
            let mut cursor = node.walk();
            if node
                .children_by_field_name("init", &mut cursor)
                .any(|binding| direct_binding(binding, name, source, false).is_some())
            {
                return true;
            }
            let Some(value) = node.child_by_field_name("value") else {
                return false;
            };
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .take_while(|child| child.end_byte() <= value.start_byte())
                .any(|pattern| pattern_identifier_binds_name(pattern, name, source))
        }
        "if_element" | "if_statement" => node
            .child_by_field_name("consequence")
            .filter(|consequence| node_contains(*consequence, path_child))
            .is_some_and(|consequence| {
                patterns_before_bind_name(node, consequence.start_byte(), name, source)
            }),
        "switch_expression_case" | "switch_statement_case" => {
            patterns_before_bind_name(node, path_child.start_byte(), name, source)
        }
        _ => false,
    }
}

fn catch_clause_binds_name(node: Node<'_>, name: &str, source: &str) -> bool {
    ["exception", "stack_trace"].into_iter().any(|field| {
        node.child_by_field_name(field)
            .and_then(|binding| binding.utf8_text(source.as_bytes()).ok())
            == Some(name)
    })
}

fn node_contains(parent: Node<'_>, child: Node<'_>) -> bool {
    parent.start_byte() <= child.start_byte() && child.end_byte() <= parent.end_byte()
}

fn patterns_before_bind_name(node: Node<'_>, end: usize, name: &str, source: &str) -> bool {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .take_while(|child| child.end_byte() <= end)
        .any(|child| variable_pattern_binds_name(child, name, source))
}

fn variable_pattern_binds_name(node: Node<'_>, name: &str, source: &str) -> bool {
    if node.kind() == "variable_pattern"
        && node
            .child_by_field_name("name")
            .and_then(|binding| binding.utf8_text(source.as_bytes()).ok())
            == Some(name)
    {
        return true;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| variable_pattern_binds_name(child, name, source))
}

fn pattern_identifier_binds_name(node: Node<'_>, name: &str, source: &str) -> bool {
    if matches!(node.kind(), "identifier" | "identifier_dollar_escaped")
        && node.utf8_text(source.as_bytes()).ok() == Some(name)
    {
        return true;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| pattern_identifier_binds_name(child, name, source))
}

fn process_scope_parameters_bind_name(node: Node<'_>, name: &str, source: &str) -> bool {
    let parameter_lists = match node.kind() {
        "function_expression" => parameter_field_lists(node),
        "method_declaration" => node
            .child_by_field_name("signature")
            .map_or_else(Vec::new, collect_parameter_lists),
        "declaration" | "function_declaration" | "local_function_declaration" => {
            direct_parameterized_signature(node).map_or_else(Vec::new, collect_parameter_lists)
        }
        _ => Vec::new(),
    };
    parameter_lists
        .into_iter()
        .any(|parameters| parameter_list_binds_name(parameters, name, source))
}

fn parameter_field_lists(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.children_by_field_name("parameters", &mut cursor)
        .filter_map(formal_parameter_list)
        .collect()
}

fn collect_parameter_lists(node: Node<'_>) -> Vec<Node<'_>> {
    own_parameter_list(node).into_iter().collect()
}

fn own_parameter_list(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "formal_parameter_list" {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(own_parameter_list)
}

fn formal_parameter_list(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "formal_parameter_list" {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == "formal_parameter_list")
}

fn parameter_list_binds_name(parameters: Node<'_>, name: &str, source: &str) -> bool {
    let mut cursor = parameters.walk();
    parameters.named_children(&mut cursor).any(|candidate| {
        if candidate.kind() == "formal_parameter" {
            return formal_parameter_name(candidate, source).as_deref() == Some(name);
        }
        candidate.kind() == "optional_formal_parameters"
            && parameter_list_binds_name(candidate, name, source)
    })
}

fn formal_parameter_name(param: Node<'_>, source: &str) -> Option<String> {
    param
        .child_by_field_name("name")
        .or_else(|| last_identifier_child(param, source))
        .and_then(|name| name.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn last_identifier_child<'tree>(node: Node<'tree>, source: &str) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| {
            matches!(
                child.kind(),
                "identifier" | "identifier_dollar_escaped" | "type_identifier"
            ) && child.utf8_text(source.as_bytes()).ok() != Some("key")
        })
        .last()
}

fn direct_parameterized_signature(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).find(|child| {
        matches!(
            child.kind(),
            "constant_constructor_signature"
                | "constructor_signature"
                | "factory_constructor_signature"
                | "function_signature"
                | "operator_signature"
                | "redirecting_factory_constructor_signature"
                | "setter_signature"
        )
    })
}

fn is_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && characters.all(|character| {
            character == '_' || character == '$' || character.is_ascii_alphanumeric()
        })
}

fn sql_call_is_parameterized(call: &str) -> bool {
    let arguments = top_level_call_arguments(call);
    arguments.get(1).is_some_and(|argument| {
        let value = argument.trim();
        !value.is_empty() && value != "null"
    }) || arguments.iter().any(|argument| {
        ["whereArgs", "variables", "parameters", "args"]
            .iter()
            .any(|name| {
                argument
                    .split_once(':')
                    .is_some_and(|(label, value)| label.trim() == *name && !value.trim().is_empty())
            })
    })
}

fn sql_text_has_dynamic_input(root: Node<'_>, source: &str, index: usize, call: &str) -> bool {
    let Some(query) = top_level_call_arguments(call)
        .first()
        .map(|query| query.trim())
    else {
        return true;
    };
    if query.contains(" + ") || query.contains("+ ") || query.contains(" +") {
        return true;
    }
    let Some(expressions) = dart_string_interpolations(query) else {
        return true;
    };
    for expression in expressions {
        if !sql_interpolation_is_fixed(root, source, index, expression) {
            return true;
        }
    }
    false
}

fn sql_interpolation_is_fixed(
    root: Node<'_>,
    source: &str,
    index: usize,
    expression: &str,
) -> bool {
    let expression = expression.trim();
    if expression.is_empty() || !is_identifier(expression) {
        return false;
    }
    if resolved_fixed_literal_value(root, source, index, expression).is_some() {
        return true;
    }
    let Some(value) = lexical_binding_value(root, source, index, expression) else {
        return false;
    };
    lexical_binding_is_const(source, value)
        || fixed_string_choice(value)
        || fixed_sql_placeholder_builder(value)
}

fn lexical_binding_is_const(source: &str, value: &str) -> bool {
    let source_start = source.as_ptr() as usize;
    let value_start = value.as_ptr() as usize;
    let Some(offset) = value_start.checked_sub(source_start) else {
        return false;
    };
    let Some(prefix) = source.get(..offset) else {
        return false;
    };
    let statement_start = prefix
        .rfind([';', '{', '}'])
        .map_or(0, |delimiter| delimiter + 1);
    prefix[statement_start..]
        .split(|character: char| {
            character != '_' && character != '$' && !character.is_ascii_alphanumeric()
        })
        .any(|token| token == "const")
}

fn fixed_string_choice(value: &str) -> bool {
    let Some(question) = top_level_delimiter(value, b'?', 0) else {
        return false;
    };
    let Some(colon) = top_level_delimiter(value, b':', question + 1) else {
        return false;
    };
    fixed_literal_value(&value[question + 1..colon]).is_some()
        && fixed_literal_value(&value[colon + 1..]).is_some()
}

fn top_level_delimiter(value: &str, delimiter: u8, start: usize) -> Option<usize> {
    let bytes = value.as_bytes();
    let mut cursor = start;
    let mut depth = 0usize;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\'' | b'"' => {
                cursor = read_string(value, cursor)?.2;
                continue;
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            byte if byte == delimiter && depth == 0 => return Some(cursor),
            _ => {}
        }
        cursor += 1;
    }
    None
}

fn fixed_sql_placeholder_builder(value: &str) -> bool {
    let compact = value.split_whitespace().collect::<String>();
    (compact.contains("List.filled(") || compact.contains("List.generate("))
        && (compact.contains(",'?'") || compact.contains("=>'?'"))
        && compact.contains(".join(")
}

fn has_dynamic_text(text: &str) -> bool {
    text.contains(" + ")
        || text.contains("+ ")
        || text.contains(" +")
        || dart_string_interpolations(text).is_none_or(|expressions| !expressions.is_empty())
}

fn dart_string_interpolations(text: &str) -> Option<Vec<&str>> {
    let bytes = text.as_bytes();
    let mut expressions = Vec::new();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        let (quote_index, raw) = if matches!(bytes[cursor], b'\'' | b'"') {
            (cursor, false)
        } else if matches!(bytes[cursor], b'r' | b'R')
            && matches!(bytes.get(cursor + 1), Some(b'\'' | b'"'))
            && cursor
                .checked_sub(1)
                .and_then(|index| bytes.get(index))
                .is_none_or(|byte| !is_identifier_byte(*byte))
        {
            (cursor + 1, true)
        } else {
            cursor += 1;
            continue;
        };
        let quote = bytes[quote_index];
        let triple =
            bytes.get(quote_index..quote_index + 3) == Some([quote, quote, quote].as_slice());
        cursor = quote_index + if triple { 3 } else { 1 };
        loop {
            if cursor >= bytes.len() {
                return None;
            }
            if triple {
                if bytes.get(cursor..cursor + 3) == Some([quote, quote, quote].as_slice()) {
                    cursor += 3;
                    break;
                }
            } else if bytes[cursor] == quote {
                cursor += 1;
                break;
            }
            if !raw && bytes[cursor] == b'\\' {
                cursor = (cursor + 2).min(bytes.len());
                continue;
            }
            if !raw && bytes[cursor] == b'$' {
                cursor += 1;
                let expression = if bytes.get(cursor) == Some(&b'{') {
                    let start = cursor + 1;
                    let relative_end = text.get(start..)?.find('}')?;
                    cursor = start + relative_end + 1;
                    text.get(start..start + relative_end)?.trim()
                } else {
                    let start = cursor;
                    while bytes
                        .get(cursor)
                        .is_some_and(|byte| is_identifier_byte(*byte))
                    {
                        cursor += 1;
                    }
                    text.get(start..cursor)?
                };
                expressions.push(expression);
                continue;
            }
            cursor += 1;
        }
    }
    Some(expressions)
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

fn plain_string_storage_receiver(source: &str, index: usize) -> bool {
    let receiver = source[..index]
        .rsplit(|character: char| {
            !character.is_ascii_alphanumeric() && !matches!(character, '_' | '$')
        })
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    [
        "prefs",
        "preference",
        "storage",
        "cache",
        "settings",
        "store",
    ]
    .iter()
    .any(|marker| receiver.contains(marker))
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

fn literal_binding_name(source: &str, index: usize) -> Option<String> {
    for context in [
        literal_context(source, index),
        literal_argument_context(source, index),
    ]
    .into_iter()
    .flatten()
    {
        for separator in ['=', ':'] {
            let Some((candidate, rest)) = context.rsplit_once(separator) else {
                continue;
            };
            if rest.trim().is_empty() {
                return trailing_identifier(candidate)
                    .map(str::to_owned)
                    .or_else(|| trailing_string_literal(candidate));
            }
        }
    }
    None
}

fn trailing_string_literal(text: &str) -> Option<String> {
    let trimmed = text.trim_end();
    trimmed
        .char_indices()
        .filter(|(_, character)| matches!(character, '\'' | '"'))
        .filter_map(|(index, _)| read_string(trimmed, index))
        .find(|(_, _, end)| *end == trimmed.len())
        .map(|(value, _, _)| value)
}

fn secret_binding_identifier(identifier: &str) -> bool {
    let lower = identifier.to_ascii_lowercase();
    let suffixes = [
        "token",
        "secret",
        "secretkey",
        "secret_key",
        "jwt",
        "privatekey",
        "private_key",
        "clientsecret",
        "client_secret",
        "refreshtoken",
        "refresh_token",
        "accesstoken",
        "access_token",
        "bearer",
        "authorization",
        "apikey",
        "api_key",
        "signingkey",
        "signing_key",
    ];
    suffixes
        .iter()
        .any(|suffix| lower == *suffix || lower.ends_with(suffix))
        || lower
            .char_indices()
            .next_back()
            .filter(|(_, suffix)| suffix.is_ascii_alphanumeric())
            .map(|(index, _)| &lower[..index])
            .is_some_and(|partitioned| {
                suffixes
                    .iter()
                    .any(|suffix| partitioned == *suffix || partitioned.ends_with(suffix))
            })
}

fn non_secret_digest_identifier(identifier: &str) -> bool {
    let normalized = identifier
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase();
    normalized.contains("sha256")
        || normalized.ends_with("checksum")
        || normalized.ends_with("digest")
        || normalized.ends_with("fingerprint")
}

fn benign_secret_storage_metadata(key: &str, value: &str) -> bool {
    let normalized_key = key
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase();
    let normalized_value = value.trim().to_ascii_lowercase();
    (normalized_value.strip_suffix(".name").is_some()
        && ["type", "kind", "name", "label"]
            .iter()
            .any(|marker| normalized_key.contains(marker)))
        || (normalized_key.contains("showtoken") && !has_secret_like_name(value))
}

fn stripe_secret_binding_context(source: &str, index: usize) -> bool {
    literal_binding_name(source, index).is_some_and(|identifier| {
        identifier
            .chars()
            .filter(|character| *character != '_')
            .collect::<String>()
            .eq_ignore_ascii_case("stripeSecretKey")
    })
}

fn stripe_secret_prefix(value: &str) -> bool {
    value.starts_with("sk_test_") || value.starts_with("sk_live_")
}

fn stripe_secret_name(value: &str) -> bool {
    value
        .chars()
        .filter(|character| !matches!(character, '_' | '-'))
        .collect::<String>()
        .eq_ignore_ascii_case("stripeSecretKey")
}

fn oauth_endpoint_metadata_context(source: &str, index: usize, value: &str) -> bool {
    if oauth_endpoint_name(value) && literal_is_map_key(source, index) {
        return true;
    }
    let Some(identifier) = literal_binding_name(source, index) else {
        return false;
    };
    oauth_endpoint_name(&identifier)
        && matches_url_scheme(value)
        && !literal_has_embedded_url_credentials(value)
}

fn oauth_endpoint_name(value: &str) -> bool {
    let normalized = value
        .chars()
        .filter(|character| !matches!(character, '_' | '-'))
        .collect::<String>();
    matches!(
        normalized.to_ascii_lowercase().as_str(),
        "authorizationendpoint"
            | "tokenendpoint"
            | "oauthendpoint"
            | "oauthtoken"
            | "oauthtokenurl"
            | "oauthtokenendpoint"
    )
}

fn literal_is_map_key(source: &str, index: usize) -> bool {
    read_string(source, index).is_some_and(|(_, _, end)| {
        source
            .get(end..)
            .is_some_and(|rest| rest.trim_start().starts_with(':'))
    })
}

fn matches_url_scheme(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

fn oauth_endpoint_url_context(source: &str, index: usize, value: &str) -> bool {
    matches_url_scheme(value)
        && literal_binding_name(source, index).is_some_and(|name| oauth_endpoint_name(&name))
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

fn benign_secret_named_literal(value: &str, secret_binding_name: bool) -> bool {
    let route_or_reset_url =
        literal_looks_like_route_path(value) || literal_looks_like_reset_or_recovery_url(value);
    let benign_copy = literal_looks_like_user_facing_copy(value)
        || literal_looks_like_validation_copy(value)
        || literal_looks_like_operational_copy(value);
    (route_or_reset_url
        && !literal_has_embedded_url_credentials(value)
        && !literal_has_secret_like_reset_path_segment(value))
        || (benign_copy && !secret_binding_name && !literal_has_concrete_token_like_segment(value))
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

fn literal_has_embedded_url_credentials(value: &str) -> bool {
    literal_has_url_userinfo(value) || literal_has_secret_like_url_parameter(value)
}

fn literal_has_url_userinfo(value: &str) -> bool {
    let trimmed = value.trim();
    let Some(authority_start) = trimmed.find("://").map(|index| index + 3) else {
        return false;
    };
    let authority_end = trimmed[authority_start..]
        .find(['/', '?', '#'])
        .map_or(trimmed.len(), |index| authority_start + index);
    trimmed[authority_start..authority_end]
        .split_once('@')
        .is_some_and(|(userinfo, host)| !userinfo.is_empty() && !host.is_empty())
}

fn segment_has_secret_like_url_parameter(segment: &str) -> bool {
    segment.split('&').any(|parameter| {
        let Some((name, value)) = parameter.split_once('=') else {
            return false;
        };
        url_parameter_name_is_credential(name) && concrete_url_parameter_value(value)
    })
}

fn url_parameter_name_is_credential(name: &str) -> bool {
    let normalized = name
        .chars()
        .filter(|character| !matches!(character, '_' | '-'))
        .collect::<String>()
        .to_ascii_lowercase();
    has_secret_like_name(name) || matches!(normalized.as_str(), "code" | "codeverifier")
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
            "current password",
            "new password",
            "invalid email or password",
            "forgot password",
            "forgot your password",
            "reset password",
            "password reset",
            "enter password",
            "confirm password",
            "confirm your password",
            "password must",
            "password is",
            "password should",
            "password cannot",
            "passwords do not match",
            "password do not match",
            "password field",
            "password requirements",
            "token expired",
            "invalid token",
            "cloud function error",
        ]
        .iter()
        .any(|phrase| lower.contains(phrase))
        && trimmed.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || character.is_ascii_whitespace()
                || matches!(
                    character,
                    '.' | ','
                        | '!'
                        | '?'
                        | ':'
                        | ';'
                        | '\''
                        | '"'
                        | '-'
                        | '_'
                        | '/'
                        | '|'
                        | '('
                        | ')'
                )
        })
}

fn literal_looks_like_validation_copy(value: &str) -> bool {
    let trimmed = value.trim();
    let lower = trimmed.to_ascii_lowercase();
    trimmed.len() <= 120
        && trimmed.contains(char::is_whitespace)
        && ((lower.contains("use at least") && lower.contains("characters"))
            || (lower.contains("reset link") && lower.contains("invalid or expired")))
        && trimmed.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || character.is_ascii_whitespace()
                || matches!(
                    character,
                    '.' | ',' | '!' | '?' | ':' | ';' | '\'' | '"' | '-' | '_' | '/' | '|'
                )
        })
}

fn literal_looks_like_operational_copy(value: &str) -> bool {
    let trimmed = value.trim();
    let lower = trimmed.to_ascii_lowercase();
    trimmed.len() <= 120
        && trimmed.contains(char::is_whitespace)
        && [
            "token issued",
            "pre-fetch token",
            "token notifications active",
            "updating password",
        ]
        .iter()
        .any(|phrase| lower.contains(phrase))
        && trimmed.chars().all(|character| !character.is_control())
}

fn literal_has_concrete_token_like_segment(value: &str) -> bool {
    value
        .split(|character: char| {
            character.is_ascii_whitespace()
                || matches!(
                    character,
                    ':' | ';'
                        | ','
                        | '='
                        | '&'
                        | '?'
                        | '#'
                        | '\''
                        | '"'
                        | '('
                        | ')'
                        | '['
                        | ']'
                        | '{'
                        | '}'
                        | '<'
                        | '>'
                )
        })
        .any(concrete_secret_like_path_segment)
}

fn has_secret_shape(value: &str) -> bool {
    if value.contains('$') {
        return false;
    }
    value.contains("-----BEGIN ") && value.contains("PRIVATE KEY-----")
        || value
            .strip_prefix("Bearer ")
            .is_some_and(|token| token.trim().len() >= 8)
        || value.starts_with("ghp_")
        || jwt_like(value)
        || long_hex(value)
}

fn jwt_like(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts[0].starts_with("eyJ")
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

fn is_local_http_authority(value: &str) -> bool {
    let lower = value
        .rsplit_once('@')
        .map_or(value, |(_, authority)| authority)
        .to_ascii_lowercase();
    lower == "localhost"
        || lower.starts_with("localhost:")
        || lower == "127.0.0.1"
        || lower.starts_with("127.0.0.1:")
        || lower == "[::1]"
        || lower.starts_with("[::1]:")
        || lower == "::1"
        || lower.starts_with("::1:")
        || lower == "10.0.2.2"
        || lower.starts_with("10.0.2.2:")
        || lower == "0.0.0.0"
        || lower.starts_with("0.0.0.0:")
}

fn debug_or_local_only_http_context(source: &str, index: usize) -> bool {
    let start = source[..index]
        .rfind('\n')
        .map_or(0, |line| {
            let mut start = line;
            for _ in 0..5 {
                start = source[..start].rfind('\n').unwrap_or(0);
            }
            start
        })
        .max(index.saturating_sub(768));
    let context = &source[start..index];
    if context.contains("kDebugMode") {
        return true;
    }
    context.contains('?')
        && [
            "startsWith('localhost')",
            "startsWith(\"localhost\")",
            "startsWith('127.')",
            "startsWith(\"127.\")",
            "startsWith('10.')",
            "startsWith(\"10.\")",
            "startsWith('192.168.')",
            "startsWith(\"192.168.\")",
            "startsWith('[::1]')",
            "startsWith(\"[::1]\")",
        ]
        .iter()
        .any(|guard| context.contains(guard))
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn fixed_runtime_lookup_crosses_a_class_without_a_matching_member() {
        let source = r"import 'dart:io';
final dartExe = Platform.resolvedExecutable;
class Runner {
  Future<Process> start() => Process.start(dartExe, const ['/path/to/snapshot']);
}
";
        let parsed = parse_dart_source_lossy(Path::new("lib/main.dart"), source)
            .unwrap_or_else(|error| panic!("parse fixture: {error}"));
        let index = source
            .find("Process.start")
            .unwrap_or_else(|| panic!("process call"));
        let call = call_inside_after(source, index + "Process.start(".len())
            .unwrap_or_else(|| panic!("process arguments"));
        assert_eq!(
            lexical_binding_value(parsed.tree().root_node(), parsed.source(), index, "dartExe"),
            Some("Platform.resolvedExecutable")
        );
        assert!(resolved_dart_runtime_start(
            Some((parsed.tree().root_node(), parsed.source())),
            index,
            &call,
            None,
        ));
    }

    #[test]
    fn fixed_sql_lookup_crosses_nested_transaction_callbacks() {
        let source = r"const _stylesTable = 'tb_styles';
class TagDao extends BaseDao {
  Future<int> insertTag(String name) async {
    final db = await database;
    return db.transaction((txn) async {
      final existing = await txn.rawQuery(
        '''
        SELECT id FROM $_stylesTable
        WHERE LOWER(font_family) = LOWER(?)
        ''',
        [name],
      );
      return existing.length;
    });
  }
}
";
        let parsed = parse_dart_source_lossy(Path::new("lib/tag.dart"), source)
            .unwrap_or_else(|error| panic!("parse fixture: {error}"));
        let index = source
            .find(".rawQuery")
            .unwrap_or_else(|| panic!("raw query call"));
        let open = call_open_after(source, index + ".rawQuery".len())
            .unwrap_or_else(|| panic!("call open"));
        let call = call_inside_after(source, open + 1).unwrap_or_else(|| panic!("query arguments"));
        assert_eq!(
            lexical_binding_value(
                parsed.tree().root_node(),
                parsed.source(),
                index,
                "_stylesTable",
            ),
            Some("'tb_styles'")
        );
        assert!(!sql_text_has_dynamic_input(
            parsed.tree().root_node(),
            parsed.source(),
            index,
            &call,
        ));
    }

    #[test]
    fn unresolved_public_inherited_sql_names_remain_dynamic() {
        let source = r"const table = 'users';
class Store extends UnknownStore {
  void query(dynamic db) {
    db.rawQuery('SELECT * FROM $table');
  }
}
";
        let parsed = parse_dart_source_lossy(Path::new("lib/store.dart"), source)
            .unwrap_or_else(|error| panic!("parse fixture: {error}"));
        let index = source
            .find(".rawQuery")
            .unwrap_or_else(|| panic!("raw query call"));
        let open = call_open_after(source, index + ".rawQuery".len())
            .unwrap_or_else(|| panic!("call open"));
        let call = call_inside_after(source, open + 1).unwrap_or_else(|| panic!("query arguments"));
        assert!(sql_text_has_dynamic_input(
            parsed.tree().root_node(),
            parsed.source(),
            index,
            &call,
        ));
    }

    #[test]
    fn oauth_metadata_exclusion_preserves_embedded_credentials() {
        let source = r"const tokenEndpoint =
    'https://client:concrete-secret@login.acme.com/oauth2/token';
const metadata = OAuthMetadata(
  authorizationEndpoint:
      'https://login.acme.com/oauth2/authorize#code_verifier=concrete-code-verifier',
);";
        let literals = string_literals(source);
        assert_eq!(literals.len(), 2);
        assert!(literal_has_url_userinfo(&literals[0].value));
        assert!(literal_has_secret_like_url_parameter(&literals[1].value));
        assert!(!oauth_endpoint_metadata_context(
            source,
            literals[0].index,
            &literals[0].value,
        ));
        assert!(!oauth_endpoint_metadata_context(
            source,
            literals[1].index,
            &literals[1].value,
        ));
    }
}
