use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::graph::normalize_against;

use super::{PolicyError, PolicyPack, PolicyRule, PolicyRuleKind, PolicySeverity};

/// # Errors
///
/// Returns [`PolicyError`] when the file is missing, unreadable, malformed, or contains a rule without an id, family, or pattern list.
pub fn load_policy_pack(
    root: impl AsRef<Path>,
    path: impl AsRef<Path>,
) -> Result<PolicyPack, PolicyError> {
    let root = root.as_ref();
    let path = normalize_against(root, path.as_ref());
    if !path.is_file() {
        return Err(PolicyError::NotFound { path });
    }
    let source = fs::read_to_string(&path).map_err(|source| PolicyError::ReadPack {
        path: path.clone(),
        source,
    })?;
    let raw = parse_raw_policy_pack(&path, &source)?;
    raw.into_pack(&path)
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawPolicyPack {
    name: Option<String>,
    rules: Vec<RawPolicyRule>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawPolicyRule {
    id: String,
    #[serde(rename = "type", alias = "kind")]
    kind: String,
    message: Option<String>,
    severity: Option<PolicySeverity>,
    pattern: Option<String>,
    patterns: Vec<String>,
    effect: Option<String>,
    effects: Vec<String>,
}

impl RawPolicyPack {
    fn into_pack(self, path: &Path) -> Result<PolicyPack, PolicyError> {
        let name = self.name.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("policy")
                .to_owned()
        });
        let mut rules = Vec::new();
        for raw in self.rules {
            rules.push(raw.into_rule(path)?);
        }
        Ok(PolicyPack {
            name,
            path: Some(path.to_path_buf()),
            rules,
        })
    }
}

impl RawPolicyRule {
    fn into_rule(self, path: &Path) -> Result<PolicyRule, PolicyError> {
        if self.id.is_empty() {
            return invalid_pack(path, "policy rule id is required");
        }
        let kind = match self.kind.as_str() {
            "banned-import" | "banned_import" => PolicyRuleKind::BannedImport {
                patterns: non_empty_patterns(path, self.pattern, self.patterns)?,
            },
            "banned-call" | "banned_call" => PolicyRuleKind::BannedCall {
                patterns: non_empty_patterns(path, self.pattern, self.patterns)?,
            },
            "banned-effect" | "banned_effect" => PolicyRuleKind::BannedEffect {
                effects: non_empty_patterns(path, self.effect, self.effects)?,
            },
            _ => {
                return invalid_pack(
                    path,
                    "policy rule type must be banned-import, banned-call, or banned-effect",
                );
            }
        };
        Ok(PolicyRule {
            id: self.id,
            message: self.message,
            severity: self.severity,
            kind,
        })
    }
}

fn non_empty_patterns(
    path: &Path,
    single: Option<String>,
    mut many: Vec<String>,
) -> Result<Vec<String>, PolicyError> {
    if let Some(single) = single {
        many.push(single);
    }
    many.retain(|pattern| !pattern.trim().is_empty());
    if many.is_empty() {
        return invalid_pack(path, "policy rule requires pattern(s)");
    }
    Ok(many)
}

fn invalid_pack<T>(path: &Path, message: impl Into<String>) -> Result<T, PolicyError> {
    Err(PolicyError::InvalidPack {
        path: path.to_path_buf(),
        message: message.into(),
    })
}

fn parse_raw_policy_pack(path: &Path, source: &str) -> Result<RawPolicyPack, PolicyError> {
    match policy_pack_format(path, source) {
        PolicyPackFormat::Json => {
            serde_json::from_str(source).map_err(|source| PolicyError::ParseJson {
                path: path.to_path_buf(),
                source,
            })
        }
        PolicyPackFormat::Jsonc => {
            serde_json::from_str(&strip_json_comments(source)).map_err(|source| {
                PolicyError::ParseJson {
                    path: path.to_path_buf(),
                    source,
                }
            })
        }
        PolicyPackFormat::Toml => toml::from_str(source).map_err(|source| PolicyError::ParseToml {
            path: path.to_path_buf(),
            source,
        }),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PolicyPackFormat {
    Json,
    Jsonc,
    Toml,
}

fn policy_pack_format(path: &Path, source: &str) -> PolicyPackFormat {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("json") => PolicyPackFormat::Json,
        Some("jsonc") => PolicyPackFormat::Jsonc,
        Some("toml") => PolicyPackFormat::Toml,
        _ if source.trim_start().starts_with('{') => PolicyPackFormat::Jsonc,
        _ => PolicyPackFormat::Toml,
    }
}

fn strip_json_comments(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(ch) = chars.next() {
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => {
                in_string = true;
                output.push(ch);
            }
            '/' if chars.peek() == Some(&'/') => {
                let _ = chars.next();
                skip_line_comment(&mut chars, &mut output);
            }
            '/' if chars.peek() == Some(&'*') => {
                let _ = chars.next();
                skip_block_comment(&mut chars, &mut output);
            }
            _ => output.push(ch),
        }
    }

    output
}

fn skip_line_comment<I>(chars: &mut std::iter::Peekable<I>, output: &mut String)
where
    I: Iterator<Item = char>,
{
    for ch in chars.by_ref() {
        if ch == '\n' {
            output.push('\n');
            break;
        }
    }
}

fn skip_block_comment<I>(chars: &mut std::iter::Peekable<I>, output: &mut String)
where
    I: Iterator<Item = char>,
{
    let mut previous = '\0';
    for ch in chars.by_ref() {
        if ch == '\n' {
            output.push('\n');
        }
        if previous == '*' && ch == '/' {
            break;
        }
        previous = ch;
    }
}
