use std::collections::BTreeSet;
use std::fs;

use super::{CodeClone, CodeCloneInstance};

pub(super) fn is_declaration_only_clone(group: &CodeClone) -> bool {
    group
        .instances
        .iter()
        .all(clone_instance_is_declaration_only)
}

fn clone_instance_is_declaration_only(instance: &CodeCloneInstance) -> bool {
    let Ok(source) = fs::read_to_string(&instance.path) else {
        return false;
    };
    let context = DeclarationContext::from_source(&source);
    let lines = source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_number = index + 1;
            (line_number >= instance.start_line && line_number <= instance.end_line)
                .then_some((line_number, strip_line_comment(line).trim()))
        })
        .filter(|(_, line)| !line.is_empty())
        .collect::<Vec<_>>();
    !lines.is_empty()
        && lines.iter().all(|(line_number, line)| {
            declaration_only_line(line, context.direct_type_body_lines.contains(line_number))
        })
}

fn strip_line_comment(line: &str) -> &str {
    line.split_once("//").map_or(line, |(code, _)| code)
}

fn declaration_only_line(line: &str, in_declaration_body: bool) -> bool {
    abstract_type_header(line)
        || line == "}"
        || line == "};"
        || (in_declaration_body && abstract_member_signature(line))
}

fn abstract_type_header(line: &str) -> bool {
    line.ends_with('{')
        && (line.starts_with("abstract interface class ")
            || line.starts_with("abstract class ")
            || line.starts_with("interface class ")
            || line.starts_with("mixin ")
            || line.starts_with("abstract mixin class "))
}

fn abstract_member_signature(line: &str) -> bool {
    let Some(signature) = line.strip_suffix(';').map(str::trim) else {
        return false;
    };
    if signature.contains('=')
        || signature.starts_with("import ")
        || signature.starts_with("export ")
        || signature.starts_with("part ")
    {
        return false;
    }
    getter_signature(signature) || callable_member_signature(signature)
}

fn getter_signature(signature: &str) -> bool {
    let tokens = signature.split_whitespace().collect::<Vec<_>>();
    let Some(get_index) = tokens.iter().position(|token| *token == "get") else {
        return false;
    };
    get_index + 1 < tokens.len()
        && tokens[..get_index]
            .iter()
            .all(|token| !EXPRESSION_STARTERS.contains(token))
        && identifier_like(tokens[get_index + 1])
}

fn callable_member_signature(signature: &str) -> bool {
    let Some(paren) = signature.find('(') else {
        return false;
    };
    let before_parameters = signature[..paren].trim();
    if before_parameters.contains('.') {
        return false;
    }
    let tokens = before_parameters.split_whitespace().collect::<Vec<_>>();
    if tokens.len() < 2
        || tokens
            .iter()
            .any(|token| EXPRESSION_STARTERS.contains(token))
    {
        return false;
    }
    let Some(member_name) = tokens.last() else {
        return false;
    };
    identifier_like(member_name) || tokens.get(tokens.len().saturating_sub(2)) == Some(&"operator")
}

fn identifier_like(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|character| {
            character == '_' || character == '$' || character.is_ascii_alphanumeric()
        })
}

const EXPRESSION_STARTERS: &[&str] = &[
    "assert", "await", "break", "continue", "do", "for", "if", "return", "switch", "throw",
    "while", "yield",
];

struct DeclarationContext {
    direct_type_body_lines: BTreeSet<usize>,
}

impl DeclarationContext {
    fn from_source(source: &str) -> Self {
        let mut direct_type_body_lines = BTreeSet::new();
        let mut type_body_depths = Vec::<usize>::new();
        let mut brace_depth = 0usize;

        for (index, raw_line) in source.lines().enumerate() {
            let line_number = index + 1;
            let line = strip_line_comment(raw_line).trim();
            if type_body_depths
                .last()
                .is_some_and(|type_depth| brace_depth == *type_depth)
            {
                direct_type_body_lines.insert(line_number);
            }

            let opens_type = abstract_type_header(line);
            let opens = line.chars().filter(|character| *character == '{').count();
            let closes = line.chars().filter(|character| *character == '}').count();
            if opens_type && opens > 0 {
                type_body_depths.push(brace_depth + 1);
            }
            brace_depth = brace_depth.saturating_add(opens).saturating_sub(closes);
            while type_body_depths
                .last()
                .is_some_and(|type_depth| brace_depth < *type_depth)
            {
                type_body_depths.pop();
            }
        }

        Self {
            direct_type_body_lines,
        }
    }
}
