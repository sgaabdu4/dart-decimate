use std::collections::BTreeSet;
use std::path::Path;

use tree_sitter::Node;

use super::{blind_spot, detected, is_identifier_byte, line_at, match_indices, segment_indices};
use crate::dart_parser::parse_dart_source_lossy;
use crate::security::catalogue::MatcherCatalogue;
use crate::security::{
    DetectionResult, SecurityBlindSpotReason, SecurityCategory, SecurityConfidence,
};

const SECURITY_SEGMENTS: [&str; 12] = [
    "token",
    "secret",
    "nonce",
    "salt",
    "otp",
    "session",
    "key",
    "password",
    "passcode",
    "auth",
    "csrf",
    "credential",
];
const SECURITY_CODE_PREFIXES: [&str; 17] = [
    "access",
    "authentication",
    "authorization",
    "backup",
    "challenge",
    "confirmation",
    "invite",
    "login",
    "mfa",
    "pairing",
    "pin",
    "recovery",
    "reset",
    "security",
    "totp",
    "verification",
    "verify",
];

pub(super) fn detect(
    path: &Path,
    source: &str,
    catalogue: &MatcherCatalogue,
    result: &mut DetectionResult,
) {
    let matcher = catalogue.by_detector("weak-randomness");
    if matcher.import_provenance != "dart:math"
        || !matcher.callees.iter().any(|callee| callee == "Random")
    {
        return;
    }
    let Ok(parsed) = parse_dart_source_lossy(path, source) else {
        return;
    };
    let root = parsed.tree().root_node();
    let imports = dart_math_imports(source);
    let random_aliases = confirmed_random_type_aliases(source, &imports);
    let random_factories = confirmed_random_factories(source, root, &imports);
    let mut known_factory_names = random_factories.clone();
    known_factory_names.extend(confirmed_secure_random_factories(source, root, &imports));
    let random_bindings =
        confirmed_random_bindings(source, root, &imports, &random_aliases, &random_factories);
    let mut direct_lines = BTreeSet::new();

    for index in random_indices(source) {
        if !identifier_node_at(root, index) || !random_constructor_at(source, index) {
            continue;
        }
        if flutter_key_context(source, index)
            || !security_shaped_flow_context(root, source, index)
            || secure_constructor_at(source, index)
        {
            continue;
        }
        if random_has_provenance(source, index, &imports) {
            direct_lines.insert(line_start(source, index));
            result.candidates.push(detected(
                path,
                source,
                index,
                SecurityCategory::WeakRandomness,
                "weak-randomness",
                SecurityConfidence::Medium,
                "Random",
            ));
        } else {
            result.blind_spots.push(blind_spot(
                path,
                source,
                index,
                SecurityCategory::WeakRandomness,
                "weak-randomness",
                SecurityBlindSpotReason::AmbiguousRandomProvenance,
            ));
        }
    }

    for alias in &random_aliases {
        for index in segment_indices(source, alias) {
            if !identifier_node_named_at(root, index, alias)
                || !source[index + alias.len()..].trim_start().starts_with('(')
                || flutter_key_context(source, index)
            {
                continue;
            }
            if !security_shaped_flow_context(root, source, index) {
                continue;
            }
            direct_lines.insert(line_start(source, index));
            result.candidates.push(detected(
                path,
                source,
                index,
                SecurityCategory::WeakRandomness,
                "weak-randomness",
                SecurityConfidence::Medium,
                "Random",
            ));
        }
    }

    let flows = RandomFlowFacts {
        bindings: &random_bindings,
        factories: &random_factories,
        known_factory_names: &known_factory_names,
        direct_lines: &direct_lines,
    };
    report_indirect_random_flows(path, source, root, &flows, result);
}

struct RandomFlowFacts<'facts> {
    bindings: &'facts BTreeSet<String>,
    factories: &'facts BTreeSet<String>,
    known_factory_names: &'facts BTreeSet<String>,
    direct_lines: &'facts BTreeSet<usize>,
}

fn report_indirect_random_flows(
    path: &Path,
    source: &str,
    root: Node<'_>,
    flows: &RandomFlowFacts<'_>,
    result: &mut DetectionResult,
) {
    for (offset, line) in source_lines(source) {
        if flows.direct_lines.contains(&offset) {
            continue;
        }
        let binding_use = flows
            .bindings
            .iter()
            .find_map(|binding| member_access_index(line, binding))
            .map(|index| offset + index);
        let factory_use = flows
            .factories
            .iter()
            .find_map(|factory| constructor_call_index(line, factory))
            .map(|index| offset + index);
        let security_use = binding_use
            .or(factory_use)
            .is_some_and(|index| security_shaped_flow_context(root, source, index));
        if security_use
            && !non_security_enclosing_scope(root, source, offset + first_non_whitespace(line))
        {
            result.blind_spots.push(blind_spot(
                path,
                source,
                offset + first_non_whitespace(line),
                SecurityCategory::WeakRandomness,
                "weak-randomness",
                SecurityBlindSpotReason::UnflattenedRandomFlow,
            ));
        } else if security_shaped_context(line)
            && ambiguous_random_constructor(line, flows.known_factory_names)
        {
            result.blind_spots.push(blind_spot(
                path,
                source,
                offset + first_non_whitespace(line),
                SecurityCategory::WeakRandomness,
                "weak-randomness",
                SecurityBlindSpotReason::AmbiguousRandomProvenance,
            ));
        }
    }
}

#[derive(Debug, Default)]
struct DartMathImports {
    unprefixed: bool,
    prefixes: BTreeSet<String>,
}

fn dart_math_imports(source: &str) -> DartMathImports {
    let mut imports = DartMathImports::default();
    for (_, line) in source_lines(source) {
        let trimmed = line.trim();
        if !trimmed.starts_with("import ")
            || !(trimmed.contains("'dart:math'") || trimmed.contains("\"dart:math\""))
            || trimmed.contains("hide Random")
            || (trimmed.contains(" show ") && !shown_name(trimmed, "Random"))
        {
            continue;
        }
        if let Some(prefix) = trimmed
            .split_once(" as ")
            .map(|(_, tail)| identifier_prefix(tail))
            .filter(|prefix| !prefix.is_empty())
        {
            imports.prefixes.insert(prefix.to_owned());
        } else {
            imports.unprefixed = true;
        }
    }
    imports
}

fn confirmed_random_bindings(
    source: &str,
    root: Node<'_>,
    imports: &DartMathImports,
    aliases: &BTreeSet<String>,
    factories: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut bindings = BTreeSet::new();
    for index in random_indices(source) {
        if !identifier_node_at(root, index)
            || !random_constructor_at(source, index)
            || secure_constructor_at(source, index)
            || !random_has_provenance(source, index, imports)
        {
            continue;
        }
        if let Some(binding) = initialized_binding_name(root, source, index)
            && !security_shaped_identifier(&binding)
            && !flutter_key_context(source, index)
        {
            bindings.insert(binding);
        }
    }
    for constructor in aliases.iter().chain(factories) {
        for index in segment_indices(source, constructor) {
            if !identifier_node_named_at(root, index, constructor)
                || !source[index + constructor.len()..]
                    .trim_start()
                    .starts_with('(')
            {
                continue;
            }
            if let Some(binding) = initialized_binding_name(root, source, index)
                && !security_shaped_identifier(&binding)
                && !flutter_key_context(source, index)
            {
                bindings.insert(binding);
            }
        }
    }
    bindings
}

fn confirmed_random_type_aliases(source: &str, imports: &DartMathImports) -> BTreeSet<String> {
    let mut aliases = BTreeSet::new();
    for (offset, line) in source_lines(source) {
        let trimmed = line.trim();
        let Some((declaration, target)) = trimmed
            .strip_prefix("typedef ")
            .and_then(|typedef| typedef.split_once('='))
        else {
            continue;
        };
        let alias = identifier_prefix(declaration);
        let target = target.trim().trim_end_matches(';').trim();
        let target_is_random =
            (target == "Random" && imports.unprefixed && !local_random_declaration(source))
                || target.split_once('.').is_some_and(|(prefix, name)| {
                    name == "Random"
                        && imports.prefixes.contains(prefix)
                        && !prefix_shadowed(source, prefix, offset)
                });
        if !alias.is_empty() && target_is_random {
            aliases.insert(alias.to_owned());
        }
    }
    aliases
}

fn confirmed_random_factories(
    source: &str,
    root: Node<'_>,
    imports: &DartMathImports,
) -> BTreeSet<String> {
    let mut factories = BTreeSet::new();
    for index in random_indices(source) {
        if !identifier_node_at(root, index) || !random_has_provenance(source, index, imports) {
            continue;
        }
        let tearoff = source[index + "Random".len()..]
            .trim_start()
            .starts_with(".new");
        if tearoff && let Some(binding) = initialized_binding_name(root, source, index) {
            factories.insert(binding);
        }
        if !secure_constructor_at(source, index)
            && let Some(factory) = returned_random_factory_name(root, source, index)
        {
            factories.insert(factory);
        }
    }
    factories
}

fn confirmed_secure_random_factories(
    source: &str,
    root: Node<'_>,
    imports: &DartMathImports,
) -> BTreeSet<String> {
    let mut factories = BTreeSet::new();
    for index in random_indices(source) {
        if !identifier_node_at(root, index)
            || !secure_constructor_at(source, index)
            || !random_has_provenance(source, index, imports)
        {
            continue;
        }
        if let Some(binding) = initialized_binding_name(root, source, index) {
            factories.insert(binding);
        }
        if let Some(factory) = returned_random_factory_name(root, source, index) {
            factories.insert(factory);
        }
    }
    factories
}

fn returned_random_factory_name(root: Node<'_>, source: &str, index: usize) -> Option<String> {
    if !random_constructor_is_return_value(root, index) {
        return None;
    }
    let mut node = root.descendant_for_byte_range(index, index.saturating_add(1))?;
    let mut returned = false;
    while let Some(parent) = node.parent() {
        returned |= parent.kind() == "return_statement";
        if matches!(
            parent.kind(),
            "function_declaration"
                | "local_function_declaration"
                | "method_declaration"
                | "function_expression"
        ) {
            let text = parent.utf8_text(source.as_bytes()).ok()?;
            let relative_index = index.checked_sub(parent.start_byte())?;
            let expression_body = text[..relative_index.min(text.len())].contains("=>");
            if !returned && !expression_body {
                return None;
            }
            if parent.kind() == "function_expression" {
                return initialized_binding_name(root, source, index);
            }
            let header_end = text.find('(')?;
            return Some(identifier_suffix(&text[..header_end]).to_owned());
        }
        node = parent;
    }
    None
}

fn random_constructor_is_return_value(root: Node<'_>, index: usize) -> bool {
    let mut node = root.descendant_for_byte_range(index, index.saturating_add(1));
    if node.is_some_and(|identifier| {
        identifier
            .parent()
            .is_some_and(|parent| parent.kind() == "function_expression_body")
    }) {
        return true;
    }
    while let Some(current) = node {
        if current.kind() == "call_expression" {
            node = Some(current);
            break;
        }
        node = current.parent();
    }
    let Some(mut value) = node else {
        return false;
    };
    while value.parent().is_some_and(|parent| {
        matches!(
            parent.kind(),
            "parenthesized_expression" | "null_assertion_expression"
        )
    }) {
        value = value.parent().unwrap_or(value);
    }
    value.parent().is_some_and(|parent| {
        matches!(
            parent.kind(),
            "return_statement" | "function_body" | "function_expression_body"
        )
    })
}

fn initialized_binding_name(root: Node<'_>, source: &str, index: usize) -> Option<String> {
    let mut node = root.descendant_for_byte_range(index, index.saturating_add(1))?;
    loop {
        if matches!(
            node.kind(),
            "initialized_identifier"
                | "initialized_variable_definition"
                | "static_final_declaration"
        ) && node
            .child_by_field_name("value")
            .is_some_and(|value| value.start_byte() <= index && value.end_byte() >= index)
        {
            if let Some(binding) = node
                .child_by_field_name("name")
                .and_then(|name| name.utf8_text(source.as_bytes()).ok())
                .map(str::to_owned)
            {
                return Some(binding);
            }
        }
        let Some(parent) = node.parent() else {
            break;
        };
        node = parent;
    }
    statement_binding_name(source, index)
}

fn statement_binding_name(source: &str, index: usize) -> Option<String> {
    let start = source[..index]
        .rfind([';', '{', '}'])
        .map_or(0, |position| position + 1);
    let declaration = source[start..index].split_once('=')?.0;
    declaration
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .rfind(|segment| !segment.is_empty())
        .map(str::to_owned)
}

fn random_has_provenance(source: &str, index: usize, imports: &DartMathImports) -> bool {
    let prefix = source[..index]
        .trim_end()
        .strip_suffix('.')
        .map(identifier_suffix)
        .unwrap_or_default();
    if prefix.is_empty() {
        imports.unprefixed && !local_random_declaration(source)
    } else {
        imports.prefixes.contains(prefix) && !prefix_shadowed(source, prefix, index)
    }
}

fn random_indices(source: &str) -> Vec<usize> {
    match_indices(source, "Random")
        .into_iter()
        .filter(|index| {
            index
                .checked_sub(1)
                .and_then(|position| source.as_bytes().get(position))
                .is_none_or(|byte| !is_identifier_byte(*byte))
        })
        .collect()
}

fn shown_name(import_line: &str, name: &str) -> bool {
    import_line
        .split_once(" show ")
        .map(|(_, shown)| shown)
        .unwrap_or_default()
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .any(|shown| shown == name)
}

fn local_random_declaration(source: &str) -> bool {
    source_lines(source).any(|(_, line)| {
        let trimmed = line.trim_start();
        [
            "class Random",
            "mixin Random",
            "enum Random",
            "typedef Random",
        ]
        .iter()
        .any(|declaration| trimmed.starts_with(declaration))
    })
}

fn prefix_shadowed(source: &str, prefix: &str, use_index: usize) -> bool {
    source_lines(&source[..use_index]).any(|(_, line)| {
        ["final", "var", "const"].iter().any(|keyword| {
            line.split_whitespace()
                .zip(line.split_whitespace().skip(1))
                .any(|(left, right)| {
                    left == *keyword
                        && right.trim_matches(|character: char| {
                            !character.is_ascii_alphanumeric() && character != '_'
                        }) == prefix
                })
        })
    })
}

fn random_constructor_at(source: &str, index: usize) -> bool {
    let after = source[index + "Random".len()..].trim_start();
    after.starts_with('(') || after.starts_with(".secure")
}

fn secure_constructor_at(source: &str, index: usize) -> bool {
    source[index + "Random".len()..]
        .trim_start()
        .starts_with(".secure")
}

fn identifier_node_at(root: Node<'_>, index: usize) -> bool {
    identifier_node_named_at(root, index, "Random")
}

fn identifier_node_named_at(root: Node<'_>, index: usize, name: &str) -> bool {
    root.descendant_for_byte_range(index, index + name.len())
        .is_some_and(|node| {
            matches!(
                node.kind(),
                "identifier" | "identifier_dollar_escaped" | "type_identifier"
            ) && node.start_byte() == index
                && node.end_byte() == index + name.len()
        })
}

fn security_shaped_statement_context(source: &str, index: usize) -> bool {
    let natural_start = source[..index]
        .rfind([';', '{', '}'])
        .map_or(0, |position| position + 1);
    let start = natural_start.max(index.saturating_sub(512));
    let natural_end = source[index..]
        .find([';', '{', '}'])
        .map_or(source.len(), |relative| index + relative);
    let end = natural_end.min(index.saturating_add(512));
    source.get(start..end).is_some_and(security_shaped_context)
}

fn security_shaped_context(line: &str) -> bool {
    line.split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .any(security_shaped_identifier)
}

fn security_shaped_flow_context(root: Node<'_>, source: &str, index: usize) -> bool {
    if security_shaped_context(line_at(source, index))
        || security_shaped_statement_context(source, index)
    {
        return true;
    }
    let Some(mut node) = root.descendant_for_byte_range(index, index.saturating_add(1)) else {
        return false;
    };
    while let Some(parent) = node.parent() {
        if parent
            .child_by_field_name("name")
            .and_then(|name| name.utf8_text(source.as_bytes()).ok())
            .is_some_and(security_shaped_identifier)
        {
            return true;
        }
        if parent.kind().contains("function") || parent.kind().contains("method") {
            let header = parent
                .utf8_text(source.as_bytes())
                .ok()
                .map(|text| text.find(['{', '=']).map_or(text, |end| &text[..end]));
            if header.is_some_and(security_shaped_context) {
                return true;
            }
        }
        if parent.kind() == "class_declaration" {
            return false;
        }
        node = parent;
    }
    false
}

fn security_shaped_identifier(identifier: &str) -> bool {
    let segments = identifier_segments(identifier);
    if strong_security_segments(&segments) {
        return true;
    }
    let ambiguous_security_segment = segments
        .iter()
        .any(|segment| matches!(segment.as_str(), "key" | "session"));
    let ui_or_simulation_name = segments.first().is_some_and(|segment| {
        matches!(
            segment.as_str(),
            "animation" | "game" | "keyboard" | "map" | "simulation" | "ui" | "widget"
        )
    }) || segments.as_slice() == ["key", "code"];
    ambiguous_security_segment && !ui_or_simulation_name
}

fn strong_security_segments(segments: &[String]) -> bool {
    let strong_security_segment = segments.iter().any(|segment| {
        SECURITY_SEGMENTS.contains(&segment.as_str())
            && !matches!(segment.as_str(), "key" | "session")
    });
    let security_code = segments
        .windows(2)
        .any(|pair| pair[1] == "code" && SECURITY_CODE_PREFIXES.contains(&pair[0].as_str()));
    strong_security_segment || security_code
}

fn non_security_enclosing_scope(root: Node<'_>, source: &str, index: usize) -> bool {
    let line_has_strong_security = line_at(source, index)
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .map(identifier_segments)
        .any(|segments| strong_security_segments(&segments));
    if line_has_strong_security {
        return false;
    }
    let Some(mut node) = root.descendant_for_byte_range(index, index.saturating_add(1)) else {
        return false;
    };
    while let Some(parent) = node.parent() {
        let scope_name = parent
            .child_by_field_name("name")
            .and_then(|name| name.utf8_text(source.as_bytes()).ok());
        if scope_name.is_some_and(non_security_scope_name) {
            return true;
        }
        if parent.kind() == "class_declaration" {
            let header = parent
                .utf8_text(source.as_bytes())
                .ok()
                .map(|text| text.find('{').map_or(text, |end| &text[..end]));
            return header.is_some_and(non_security_scope_name);
        }
        node = parent;
    }
    false
}

fn non_security_scope_name(name: &str) -> bool {
    identifier_segments(name).iter().any(|segment| {
        matches!(
            segment.as_str(),
            "animation" | "game" | "keyboard" | "map" | "simulation" | "ui" | "widget"
        )
    })
}

fn identifier_segments(identifier: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let characters = identifier.chars().collect::<Vec<_>>();
    for (index, character) in characters.iter().copied().enumerate() {
        if character == '_' || character == '-' {
            if !current.is_empty() {
                segments.push(std::mem::take(&mut current));
            }
        } else if character.is_ascii_uppercase()
            && !current.is_empty()
            && (characters[index - 1].is_ascii_lowercase()
                || characters
                    .get(index + 1)
                    .is_some_and(char::is_ascii_lowercase))
        {
            segments.push(std::mem::take(&mut current));
            current.push(character.to_ascii_lowercase());
        } else {
            current.push(character.to_ascii_lowercase());
        }
    }
    if !current.is_empty() {
        segments.push(current);
    }
    segments
}

fn ambiguous_random_constructor(line: &str, known_factories: &BTreeSet<String>) -> bool {
    match_indices(line, "Random").into_iter().any(|index| {
        let before = index
            .checked_sub(1)
            .and_then(|position| line.as_bytes().get(position))
            .copied();
        if !before.is_some_and(is_identifier_byte)
            || !line[index + "Random".len()..].trim_start().starts_with('(')
        {
            return false;
        }
        let identifier_start = line[..index]
            .rfind(|character: char| !character.is_ascii_alphanumeric() && character != '_')
            .map_or(0, |position| position + 1);
        let identifier = &line[identifier_start..index + "Random".len()];
        identifier
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_uppercase())
            && !known_factories.contains(identifier)
    })
}

fn flutter_key_context(source: &str, index: usize) -> bool {
    let start = source[..index]
        .rfind([';', '{', '}', '\n'])
        .map_or(0, |position| position + 1);
    let context = &source[start..index];
    ["ValueKey(", "PageStorageKey(", "ObjectKey("]
        .iter()
        .any(|constructor| context.contains(constructor))
}

fn member_access_index(line: &str, binding: &str) -> Option<usize> {
    segment_indices(line, binding)
        .into_iter()
        .find(|index| line[index + binding.len()..].trim_start().starts_with('.'))
}

fn constructor_call_index(line: &str, constructor: &str) -> Option<usize> {
    segment_indices(line, constructor)
        .into_iter()
        .find(|index| {
            line[index + constructor.len()..]
                .trim_start()
                .starts_with('(')
        })
}

fn source_lines(source: &str) -> impl Iterator<Item = (usize, &str)> {
    source.split_inclusive('\n').scan(0, |offset, line| {
        let start = *offset;
        *offset += line.len();
        Some((start, line.trim_end_matches('\n')))
    })
}

fn line_start(source: &str, index: usize) -> usize {
    source[..index]
        .rfind('\n')
        .map_or(0, |position| position + 1)
}

fn first_non_whitespace(line: &str) -> usize {
    line.find(|character: char| !character.is_whitespace())
        .unwrap_or(0)
}

fn identifier_prefix(text: &str) -> &str {
    text.trim_start()
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .next()
        .unwrap_or_default()
}

fn identifier_suffix(text: &str) -> &str {
    text.rsplit(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .next()
        .unwrap_or_default()
}
