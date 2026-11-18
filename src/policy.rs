use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use glob::Pattern;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use tree_sitter::Node;

use crate::graph::normalize_against;
use crate::{DartFile, DependencyKind, Location, ScannedProject};

mod packs;
pub use packs::load_policy_pack;

/// Stable JSON schema version for Dart Decimate policy rule packs.
pub const RULE_PACK_SCHEMA_VERSION: &str = "dart-decimate.rule-pack.v1";

/// Boundary-local forbidden call rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryCallRule {
    /// Source zone covered by the rule.
    pub from: PathBuf,
    /// Forbidden direct call patterns.
    pub forbidden: Vec<String>,
}

impl BoundaryCallRule {
    /// Create a boundary call rule.
    #[must_use]
    pub fn new(from: impl Into<PathBuf>, forbidden: Vec<String>) -> Self {
        Self {
            from: from.into(),
            forbidden,
        }
    }
}

/// A direct call made from a zone where that call is forbidden.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryCallViolation {
    /// File containing the call.
    pub path: PathBuf,
    /// Rule source zone.
    pub from_boundary: PathBuf,
    /// Call text extracted from syntax.
    pub callee: String,
    /// Pattern that matched the call.
    pub pattern: String,
    /// Location of the call expression.
    pub location: Location,
}

/// Declarative policy pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyPack {
    /// Stable pack name used in finding rule ids.
    pub name: String,
    /// Path to the pack file, when loaded from disk.
    pub path: Option<PathBuf>,
    /// Pack rules.
    pub rules: Vec<PolicyRule>,
}

/// Declarative policy rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRule {
    /// Rule id unique within the pack.
    pub id: String,
    /// Human-readable rule description.
    pub message: Option<String>,
    /// Optional per-rule severity override.
    pub severity: Option<PolicySeverity>,
    /// Rule matcher.
    pub kind: PolicyRuleKind,
}

/// Policy rule severity declared inside a rule pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolicySeverity {
    /// Fails the command verdict.
    Error,
    /// Reports without failing the command verdict.
    Warn,
}

/// Supported declarative policy rule families.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyRuleKind {
    /// Ban Dart import/export URI patterns.
    BannedImport { patterns: Vec<String> },
    /// Ban direct syntactic call patterns.
    BannedCall { patterns: Vec<String> },
    /// Catalogue/effect rule retained for pack compatibility.
    BannedEffect { effects: Vec<String> },
}

/// Policy violation detected from a rule pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyViolation {
    /// Pack name.
    pub pack: String,
    /// Rule id inside the pack.
    pub rule: String,
    /// Stable rule id for output.
    pub rule_id: String,
    /// File containing the violation.
    pub path: PathBuf,
    /// Matched import/export URI or call text.
    pub target: String,
    /// Matched policy pattern.
    pub pattern: String,
    /// Location of the import/export directive or call expression.
    pub location: Location,
    /// Message from the rule pack, if any.
    pub message: Option<String>,
    /// Per-rule severity override, if any.
    pub severity: Option<PolicySeverity>,
}

/// Errors returned while loading policy packs or source needed for call checks.
#[derive(Debug, Error)]
pub enum PolicyError {
    /// Policy pack path did not exist.
    #[error("policy pack not found: {path}")]
    NotFound {
        /// Missing pack path.
        path: PathBuf,
    },
    /// Policy pack file could not be read.
    #[error("failed to read policy pack {path}: {source}")]
    ReadPack {
        /// Pack path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// JSON or JSONC pack could not be parsed.
    #[error("failed to parse policy pack {path}: {source}")]
    ParseJson {
        /// Pack path.
        path: PathBuf,
        /// JSON parse error.
        source: serde_json::Error,
    },
    /// TOML pack could not be parsed.
    #[error("failed to parse policy pack {path}: {source}")]
    ParseToml {
        /// Pack path.
        path: PathBuf,
        /// TOML parse error.
        source: toml::de::Error,
    },
    /// Policy pack rule was invalid.
    #[error("invalid policy pack {path}: {message}")]
    InvalidPack {
        /// Pack path.
        path: PathBuf,
        /// Validation message.
        message: String,
    },
    /// Dart source could not be read for call analysis.
    #[error("failed to read Dart file {path}: {source}")]
    ReadSource {
        /// Source path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Tree-Sitter rejected the Dart grammar.
    #[error("failed to load Dart grammar: {0}")]
    Language(#[from] tree_sitter::LanguageError),
    /// Tree-Sitter did not produce a parse tree.
    #[error("tree-sitter did not return a parse tree for {path}")]
    ParseCancelled {
        /// Path being parsed.
        path: PathBuf,
    },
}

/// # Errors
///
/// Returns [`PolicyError`] if a checked Dart source file cannot be read or parsed for call expressions.
pub fn detect_boundary_call_violations(
    project: &ScannedProject,
    rules: &[BoundaryCallRule],
) -> Result<Vec<BoundaryCallViolation>, PolicyError> {
    if rules.is_empty() {
        return Ok(Vec::new());
    }

    let normalized_rules = rules
        .iter()
        .map(|rule| {
            (
                normalize_against(&project.root, &rule.from),
                compile_patterns(&rule.forbidden),
            )
        })
        .collect::<Vec<_>>();

    let mut source_cache = SourceCallCache::default();
    let mut violations = Vec::new();
    for file in &project.files {
        let path = normalize_against(&project.root, &file.path);
        for (from_boundary, patterns) in &normalized_rules {
            if !path.starts_with(from_boundary) {
                continue;
            }
            let calls = source_cache.calls_for(&path)?;
            for call in calls {
                for pattern in patterns {
                    if pattern.matches(&call.callee) {
                        violations.push(BoundaryCallViolation {
                            path: path.clone(),
                            from_boundary: from_boundary.clone(),
                            callee: call.callee.clone(),
                            pattern: pattern.source.clone(),
                            location: call.location,
                        });
                    }
                }
            }
        }
    }

    violations.sort_by(|left, right| {
        (
            &left.path,
            left.location.line,
            left.location.column,
            &left.callee,
            &left.pattern,
        )
            .cmp(&(
                &right.path,
                right.location.line,
                right.location.column,
                &right.callee,
                &right.pattern,
            ))
    });
    Ok(violations)
}

/// # Errors
///
/// Returns [`PolicyError`] if a checked Dart source file cannot be read or parsed for call-expression rules.
pub fn detect_policy_violations(
    project: &ScannedProject,
    packs: &[PolicyPack],
) -> Result<Vec<PolicyViolation>, PolicyError> {
    if packs.is_empty() {
        return Ok(Vec::new());
    }

    let compiled = compile_policy_packs(packs);
    let mut source_cache = SourceCallCache::default();
    let mut violations = Vec::new();
    for file in &project.files {
        let path = normalize_against(&project.root, &file.path);
        if !path.starts_with(&project.root) {
            continue;
        }
        detect_import_policy(file, &path, &compiled, &mut violations);
        detect_call_policy(&mut source_cache, &path, &compiled, &mut violations)?;
    }

    violations.sort_by(|left, right| {
        (
            &left.path,
            left.location.line,
            left.location.column,
            &left.rule_id,
            &left.target,
        )
            .cmp(&(
                &right.path,
                right.location.line,
                right.location.column,
                &right.rule_id,
                &right.target,
            ))
    });
    Ok(violations)
}

/// Return the JSON schema for declarative policy rule packs.
#[must_use]
pub fn rule_pack_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "schema_version": RULE_PACK_SCHEMA_VERSION,
        "title": "Dart Decimate policy rule pack",
        "type": "object",
        "additionalProperties": false,
        "required": ["rules"],
        "properties": {
            "name": { "type": "string" },
            "rules": {
                "type": "array",
                "items": { "$ref": "#/$defs/rule" }
            }
        },
        "$defs": {
            "rule": {
                "type": "object",
                "additionalProperties": false,
                "required": ["id", "type"],
                "properties": {
                    "id": { "type": "string", "minLength": 1 },
                    "type": {
                        "type": "string",
                        "enum": ["banned-import", "banned-call", "banned-effect"]
                    },
                    "kind": {
                        "type": "string",
                        "enum": ["banned-import", "banned-call", "banned-effect"]
                    },
                    "message": { "type": "string" },
                    "severity": {
                        "type": "string",
                        "enum": ["warn", "error"]
                    },
                    "pattern": { "type": "string" },
                    "patterns": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "effect": { "type": "string" },
                    "effects": {
                        "type": "array",
                        "items": { "type": "string" }
                    }
                }
            }
        }
    })
}

fn detect_import_policy(
    file: &DartFile,
    path: &Path,
    packs: &[CompiledPolicyPack],
    violations: &mut Vec<PolicyViolation>,
) {
    for directive in file
        .imports
        .iter()
        .map(|import| (DependencyKind::Import, import.uri.as_str(), import.location))
        .chain(
            file.exports
                .iter()
                .map(|export| (DependencyKind::Export, export.uri.as_str(), export.location)),
        )
    {
        let (_, uri, location) = directive;
        for pack in packs {
            for rule in &pack.import_rules {
                for pattern in &rule.patterns {
                    if pattern.matches(uri) {
                        violations.push(PolicyViolation {
                            pack: pack.name.clone(),
                            rule: rule.id.clone(),
                            rule_id: policy_rule_id(&pack.name, &rule.id),
                            path: path.to_path_buf(),
                            target: uri.to_owned(),
                            pattern: pattern.source.clone(),
                            location,
                            message: rule.message.clone(),
                            severity: rule.severity,
                        });
                    }
                }
            }
        }
    }
}

fn detect_call_policy(
    source_cache: &mut SourceCallCache,
    path: &Path,
    packs: &[CompiledPolicyPack],
    violations: &mut Vec<PolicyViolation>,
) -> Result<(), PolicyError> {
    if !packs.iter().any(|pack| !pack.call_rules.is_empty()) {
        return Ok(());
    }
    let calls = source_cache.calls_for(path)?;
    for call in calls {
        for pack in packs {
            for rule in &pack.call_rules {
                for pattern in &rule.patterns {
                    if pattern.matches(&call.callee) {
                        violations.push(PolicyViolation {
                            pack: pack.name.clone(),
                            rule: rule.id.clone(),
                            rule_id: policy_rule_id(&pack.name, &rule.id),
                            path: path.to_path_buf(),
                            target: call.callee.clone(),
                            pattern: pattern.source.clone(),
                            location: call.location,
                            message: rule.message.clone(),
                            severity: rule.severity,
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

fn policy_rule_id(pack: &str, rule: &str) -> String {
    format!("dart-decimate/policy/{pack}/{rule}")
}

#[derive(Debug, Clone)]
struct CompiledPolicyPack {
    name: String,
    import_rules: Vec<CompiledPolicyRule>,
    call_rules: Vec<CompiledPolicyRule>,
}

#[derive(Debug, Clone)]
struct CompiledPolicyRule {
    id: String,
    message: Option<String>,
    severity: Option<PolicySeverity>,
    patterns: Vec<CompiledPattern>,
}

fn compile_policy_packs(packs: &[PolicyPack]) -> Vec<CompiledPolicyPack> {
    packs
        .iter()
        .map(|pack| {
            let mut import_rules = Vec::new();
            let mut call_rules = Vec::new();
            for rule in &pack.rules {
                match &rule.kind {
                    PolicyRuleKind::BannedImport { patterns } => {
                        import_rules.push(CompiledPolicyRule {
                            id: rule.id.clone(),
                            message: rule.message.clone(),
                            severity: rule.severity,
                            patterns: compile_patterns(patterns),
                        });
                    }
                    PolicyRuleKind::BannedCall { patterns } => {
                        call_rules.push(CompiledPolicyRule {
                            id: rule.id.clone(),
                            message: rule.message.clone(),
                            severity: rule.severity,
                            patterns: compile_patterns(patterns),
                        });
                    }
                    PolicyRuleKind::BannedEffect { .. } => {}
                }
            }
            CompiledPolicyPack {
                name: pack.name.clone(),
                import_rules,
                call_rules,
            }
        })
        .collect()
}

#[derive(Debug, Clone)]
struct CompiledPattern {
    source: String,
    glob: Option<Pattern>,
}

impl CompiledPattern {
    fn matches(&self, value: &str) -> bool {
        self.glob
            .as_ref()
            .is_some_and(|pattern| pattern.matches(value))
            || self.source == value
    }
}

fn compile_patterns(patterns: &[String]) -> Vec<CompiledPattern> {
    patterns
        .iter()
        .map(|pattern| CompiledPattern {
            source: pattern.clone(),
            glob: Pattern::new(pattern).ok(),
        })
        .collect()
}

#[derive(Debug, Default)]
struct SourceCallCache {
    calls: BTreeMap<PathBuf, Vec<CallSite>>,
}

impl SourceCallCache {
    fn calls_for(&mut self, path: &Path) -> Result<Vec<CallSite>, PolicyError> {
        if let Some(calls) = self.calls.get(path) {
            return Ok(calls.clone());
        }
        let calls = extract_calls(path)?;
        self.calls.insert(path.to_path_buf(), calls.clone());
        Ok(calls)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CallSite {
    callee: String,
    location: Location,
}

fn extract_calls(path: &Path) -> Result<Vec<CallSite>, PolicyError> {
    let source = fs::read_to_string(path).map_err(|source| PolicyError::ReadSource {
        path: path.to_path_buf(),
        source,
    })?;
    let parsed =
        crate::dart_parser::parse_dart_source_lossy(path, &source).map_err(policy_parse_error)?;
    let mut calls = Vec::new();
    collect_calls(parsed.tree().root_node(), parsed.source(), &mut calls);
    calls.sort_by(|left, right| {
        (left.location.line, left.location.column, &left.callee).cmp(&(
            right.location.line,
            right.location.column,
            &right.callee,
        ))
    });
    calls.dedup();
    Ok(calls)
}

fn policy_parse_error(error: crate::dart_parser::DartParseError) -> PolicyError {
    match error {
        crate::dart_parser::DartParseError::Language(source) => PolicyError::Language(source),
        crate::dart_parser::DartParseError::ParseCancelled { path } => {
            PolicyError::ParseCancelled { path }
        }
        crate::dart_parser::DartParseError::Syntax { path } => PolicyError::ParseCancelled { path },
    }
}

fn collect_calls(node: Node<'_>, source: &str, calls: &mut Vec<CallSite>) {
    match node.kind() {
        "call_expression" => push_call_expression(node, source, calls),
        "cascade_call_expression" => push_cascade_call_expression(node, source, calls),
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_calls(child, source, calls);
    }
}

fn push_call_expression(node: Node<'_>, source: &str, calls: &mut Vec<CallSite>) {
    let Some(function) = node.child_by_field_name("function") else {
        return;
    };
    let Some(callee) = callee_text(function, source) else {
        return;
    };
    calls.push(CallSite {
        callee,
        location: node.start_position().into(),
    });
}

fn push_cascade_call_expression(node: Node<'_>, source: &str, calls: &mut Vec<CallSite>) {
    let property = node
        .child_by_field_name("property")
        .or_else(|| node.child_by_field_name("function"));
    let Some(property) = property else {
        return;
    };
    let Some(callee) = callee_text(property, source) else {
        return;
    };
    calls.push(CallSite {
        callee,
        location: node.start_position().into(),
    });
}

fn callee_text(node: Node<'_>, source: &str) -> Option<String> {
    match node.kind() {
        "identifier" => text(node, source),
        "member_expression" | "null_aware_member_expression" => {
            let object = node.child_by_field_name("object")?;
            let property = node.child_by_field_name("property")?;
            Some(format!(
                "{}.{}",
                callee_text(object, source)?,
                text(property, source)?
            ))
        }
        "instantiation_expression" => node
            .child_by_field_name("function")
            .and_then(|function| callee_text(function, source)),
        "null_assertion_expression" => node
            .child_by_field_name("value")
            .and_then(|value| callee_text(value, source)),
        "parenthesized_expression" => {
            first_named_child(node).and_then(|child| callee_text(child, source))
        }
        _ => text(node, source).map(|value| {
            value
                .split_once('(')
                .map_or(value.clone(), |(callee, _)| callee.trim().to_owned())
        }),
    }
}

fn first_named_child(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).next()
}

fn text(node: Node<'_>, source: &str) -> Option<String> {
    node.utf8_text(source.as_bytes())
        .ok()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests;
