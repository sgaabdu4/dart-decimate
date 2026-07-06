use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::{CodeClone, CodeCloneInstance};

pub(super) struct DeclarationCloneFilter {
    files: BTreeMap<PathBuf, Option<DeclarationFileContext>>,
}

impl DeclarationCloneFilter {
    pub(super) fn new() -> Self {
        Self {
            files: BTreeMap::new(),
        }
    }

    pub(super) fn is_declaration_only_clone(&mut self, group: &CodeClone) -> bool {
        group
            .instances
            .iter()
            .all(|instance| self.clone_instance_is_declaration_only(instance))
    }

    fn clone_instance_is_declaration_only(&mut self, instance: &CodeCloneInstance) -> bool {
        let path = instance.path.clone();
        let Some(file) = self
            .files
            .entry(path)
            .or_insert_with(|| DeclarationFileContext::read(&instance.path))
            .as_ref()
        else {
            return false;
        };
        let lines = file
            .source
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
                declaration_only_line(
                    line,
                    file.context.direct_type_body_lines.contains(line_number),
                )
            })
    }

    #[cfg(test)]
    fn cached_file_count(&self) -> usize {
        self.files.len()
    }
}

struct DeclarationFileContext {
    source: String,
    context: DeclarationContext,
}

impl DeclarationFileContext {
    fn read(path: &Path) -> Option<Self> {
        let source = fs::read_to_string(path).ok()?;
        let context = DeclarationContext::from_source(&source);
        Some(Self { source, context })
    }
}

fn strip_line_comment(line: &str) -> &str {
    line.split_once("//").map_or(line, |(code, _)| code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declaration_filter_reuses_file_contexts_by_path() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = tempfile::tempdir()?;
        let path = fixture.path().join("contract.dart");
        fs::write(
            &path,
            "abstract class Contract {\n  String get title;\n  void save();\n}\n",
        )?;
        let group = CodeClone {
            fingerprint: "contract".to_owned(),
            instances: vec![
                CodeCloneInstance {
                    path: path.clone(),
                    start_line: 1,
                    end_line: 4,
                    column: 0,
                },
                CodeCloneInstance {
                    path,
                    start_line: 1,
                    end_line: 4,
                    column: 0,
                },
            ],
            line_count: 4,
            token_count: 8,
        };
        let mut filter = DeclarationCloneFilter::new();

        assert!(filter.is_declaration_only_clone(&group));
        assert_eq!(filter.cached_file_count(), 1);
        Ok(())
    }

    #[test]
    fn declaration_filter_treats_operator_signatures_as_declarations()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = tempfile::tempdir()?;
        let path = fixture.path().join("contract.dart");
        fs::write(
            &path,
            "abstract interface class Contract {\n  bool operator ==(Object other);\n  String operator [](int index);\n  void operator []=(int index, String value);\n}\n",
        )?;
        let group = CodeClone {
            fingerprint: "operators".to_owned(),
            instances: vec![CodeCloneInstance {
                path,
                start_line: 1,
                end_line: 5,
                column: 0,
            }],
            line_count: 5,
            token_count: 12,
        };
        let mut filter = DeclarationCloneFilter::new();

        assert!(filter.is_declaration_only_clone(&group));
        Ok(())
    }

    #[test]
    fn declaration_filter_rejects_defaults_and_assignments()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = tempfile::tempdir()?;
        let path = fixture.path().join("contract.dart");
        fs::write(
            &path,
            "abstract class Contract {\n  void save({String label = 'x'});\n  String title = 'x';\n}\n",
        )?;
        let group = CodeClone {
            fingerprint: "defaults".to_owned(),
            instances: vec![CodeCloneInstance {
                path,
                start_line: 1,
                end_line: 4,
                column: 0,
            }],
            line_count: 4,
            token_count: 12,
        };
        let mut filter = DeclarationCloneFilter::new();

        assert!(!filter.is_declaration_only_clone(&group));
        Ok(())
    }

    #[test]
    fn declaration_filter_accepts_modified_type_headers() -> Result<(), Box<dyn std::error::Error>>
    {
        let fixture = tempfile::tempdir()?;
        let path = fixture.path().join("contract.dart");
        fs::write(
            &path,
            "abstract final class UtilityContract {\n  String get title;\n  void save();\n}\nabstract base class BaseContract {\n  String get label;\n  void load();\n}\nbase mixin class BehaviorContract {\n  String get name;\n  void apply();\n}\nsealed class SealedContract {\n  String get value;\n  void seal();\n}\n",
        )?;
        let group = CodeClone {
            fingerprint: "modified-contracts".to_owned(),
            instances: vec![CodeCloneInstance {
                path,
                start_line: 1,
                end_line: 16,
                column: 0,
            }],
            line_count: 16,
            token_count: 32,
        };
        let mut filter = DeclarationCloneFilter::new();

        assert!(filter.is_declaration_only_clone(&group));
        Ok(())
    }
}

fn declaration_only_line(line: &str, in_declaration_body: bool) -> bool {
    abstract_type_header(line)
        || line == "}"
        || line == "};"
        || (in_declaration_body && abstract_member_signature(line))
}

fn abstract_type_header(line: &str) -> bool {
    let Some(header) = line.strip_suffix('{').map(str::trim) else {
        return false;
    };
    declaration_type_header(header)
}

fn declaration_type_header(header: &str) -> bool {
    let tokens = header.split_whitespace().collect::<Vec<_>>();
    if tokens.is_empty() {
        return false;
    }
    for (index, token) in tokens.iter().enumerate() {
        match *token {
            "class" => {
                if index > 0
                    && tokens.get(index.saturating_sub(1)) == Some(&"mixin")
                    && type_modifiers(&tokens[..index.saturating_sub(1)])
                {
                    return true;
                }
                return index > 0 && type_modifiers(&tokens[..index]);
            }
            "mixin" => {
                if tokens.get(index + 1) == Some(&"class") {
                    continue;
                }
                return type_modifiers(&tokens[..index]);
            }
            "enum" => return type_modifiers(&tokens[..index]),
            "extension" => return type_modifiers(&tokens[..index]),
            _ => {}
        }
    }
    false
}

fn type_modifiers(tokens: &[&str]) -> bool {
    tokens.iter().all(|token| {
        matches!(
            *token,
            "abstract" | "base" | "final" | "interface" | "sealed"
        )
    })
}

fn abstract_member_signature(line: &str) -> bool {
    let Some(signature) = line.strip_suffix(';').map(str::trim) else {
        return false;
    };
    if signature_contains_non_operator_equals(signature)
        || signature.starts_with("import ")
        || signature.starts_with("export ")
        || signature.starts_with("part ")
    {
        return false;
    }
    getter_signature(signature) || callable_member_signature(signature)
}

fn signature_contains_non_operator_equals(signature: &str) -> bool {
    let Some(paren) = signature.find('(') else {
        return signature.contains('=');
    };
    let before_parameters = signature[..paren].trim();
    let tokens = before_parameters.split_whitespace().collect::<Vec<_>>();
    if tokens.len() >= 2 && tokens.get(tokens.len() - 2) == Some(&"operator") {
        let Some(operator_token) = tokens.last() else {
            return signature.contains('=');
        };
        let Some(operator_start) = before_parameters.rfind(operator_token) else {
            return signature.contains('=');
        };
        return before_parameters[..operator_start].contains('=')
            || signature[paren..].contains('=');
    }
    signature.contains('=')
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
