use serde::{Deserialize, Serialize};

use super::{
    JsonAttackSurfaceEntry, JsonCloneGroup, JsonComplexityFinding, JsonFeatureFlag,
    JsonFileHealthScore, JsonHealthHotspot, JsonRefactoringTarget, JsonRuntimeCoverage,
    JsonSecurityCandidate, JsonThresholdOverride,
};

/// Command that produced a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReportCommand {
    /// Combined project check.
    Check,
    /// Changed-code project audit.
    Audit,
    /// Dead-code reachability command.
    DeadCode,
    /// Circular dependency command.
    Cycles,
    /// File trace command.
    TraceFile,
    /// Symbol trace command.
    TraceSymbol,
    /// Pub dependency trace command.
    TraceDependency,
    /// Code duplication command.
    Dupes,
    /// Code health command.
    Health,
    /// Clone trace command.
    TraceClone,
    /// Targeted evidence bundle command.
    Inspect,
    /// Feature flag inventory command.
    Flags,
    /// Security candidate command.
    Security,
}

impl ReportCommand {
    /// CLI command name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Audit => "audit",
            Self::DeadCode => "dead-code",
            Self::Cycles => "cycles",
            Self::TraceFile => "trace-file",
            Self::TraceSymbol => "trace-symbol",
            Self::TraceDependency => "trace-dependency",
            Self::Dupes => "dupes",
            Self::Health => "health",
            Self::TraceClone => "trace-clone",
            Self::Inspect => "inspect",
            Self::Flags => "flags",
            Self::Security => "security",
        }
    }

    /// Typed JSON envelope discriminator.
    #[must_use]
    pub const fn kind(self) -> &'static str {
        match self {
            Self::Check => "combined",
            Self::Audit => "audit",
            Self::DeadCode => "dead-code",
            Self::Cycles => "cycles",
            Self::Dupes => "dupes",
            Self::Health => "health",
            Self::Flags => "flags",
            Self::Security => "security",
            Self::TraceFile
            | Self::TraceSymbol
            | Self::TraceDependency
            | Self::TraceClone
            | Self::Inspect => self.as_str(),
        }
    }
}

/// Pass/fail verdict for CI and agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// No findings were produced.
    Pass,
    /// One or more findings were produced.
    Fail,
}

/// Audit review risk band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuditRiskLevel {
    /// No visible audit risk.
    Pass,
    /// Visible findings need review but were not introduced as errors.
    Warn,
    /// The change introduced at least one error-severity finding.
    Fail,
}

/// Introduced/pre-existing audit finding split.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditAttribution {
    /// Findings touching files changed since the audit base.
    pub introduced: AuditAttributionCounts,
    /// Findings surfaced only through related unchanged files.
    pub pre_existing: AuditAttributionCounts,
}

/// Audit attribution counters.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditAttributionCounts {
    /// Findings in this attribution bucket.
    pub findings: usize,
    /// Error-severity findings.
    pub error_findings: usize,
    /// Warning-severity findings.
    pub warning_findings: usize,
    /// Findings marked safe to delete from graph evidence alone.
    pub safe_to_delete: usize,
    /// Distinct files referenced by these findings.
    pub files: usize,
}

/// JSON report emitted by `--format json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonReport {
    /// Schema identifier.
    pub schema_version: String,
    /// Typed JSON envelope discriminator.
    pub kind: String,
    /// Tool name.
    pub tool: String,
    /// Command that produced this report.
    pub command: ReportCommand,
    /// CI and agent verdict.
    pub verdict: Verdict,
    /// Numeric rollup.
    pub summary: ReportSummary,
    /// Machine-actionable findings.
    pub findings: Vec<Finding>,
    /// Duplicate-code clone groups, populated by `check` and `dupes`.
    pub clone_groups: Vec<JsonCloneGroup>,
    /// Complexity findings, populated by `check` and `health`.
    pub complexity: Vec<JsonComplexityFinding>,
    /// File health scores, populated by `health --file-scores`.
    pub file_scores: Vec<JsonFileHealthScore>,
    /// Health hotspots, populated by `health --hotspots`.
    pub hotspots: Vec<JsonHealthHotspot>,
    /// Refactoring targets, populated by `health --targets`.
    pub refactoring_targets: Vec<JsonRefactoringTarget>,
    /// Health threshold override states, populated when configured.
    pub threshold_overrides: Vec<JsonThresholdOverride>,
    /// Feature flag inventory, populated by `flags`.
    pub feature_flags: Vec<JsonFeatureFlag>,
    /// Security review candidates, populated by `security`.
    pub security_candidates: Vec<JsonSecurityCandidate>,
    /// Attack-surface inventory, populated by `security --surface`.
    pub attack_surface: Vec<JsonAttackSurfaceEntry>,
    /// Runtime coverage intelligence, populated by `--runtime-coverage`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_coverage: Option<JsonRuntimeCoverage>,
    /// Read-only follow-up commands agents should run before acting.
    pub next_steps: Vec<NextStep>,
}

impl JsonReport {
    pub(crate) fn has_hidden_security_candidate_occurrences(&self) -> bool {
        !self.security_candidates.is_empty()
            && self.summary.security_candidate_occurrences > self.security_candidates.len()
    }
}

/// Report summary.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportSummary {
    /// Dart files parsed.
    pub files: usize,
    /// Resolved graph edges.
    pub edges: usize,
    /// Local dependencies that did not resolve to parsed files.
    pub unresolved_dependencies: usize,
    /// Dart part files whose `part of` relationship is invalid.
    pub part_of_violations: usize,
    /// Declared pub dependencies not imported by Dart files.
    pub unused_dependencies: usize,
    /// Declared dev dependencies not imported by Dart files.
    pub unused_dev_dependencies: usize,
    /// Runtime dependencies imported only from dev/test files.
    pub test_only_dependencies: usize,
    /// Dependency override findings across all override hygiene classes.
    pub dependency_overrides: usize,
    /// Dependency overrides absent from the resolved lockfile package graph.
    pub unused_dependency_overrides: usize,
    /// Dependency override entries Pub cannot honor.
    pub misconfigured_dependency_overrides: usize,
    /// Imported pub packages absent from pubspec dependencies.
    pub unlisted_dependencies: usize,
    /// Imports into another package's private `lib/src` implementation tree.
    pub private_src_imports: usize,
    /// Unreachable files.
    pub dead_files: usize,
    /// Unused public top-level declarations.
    pub unused_exports: usize,
    /// Unused public top-level type aliases.
    pub unused_types: usize,
    /// Public signatures exposing same-library private Dart types.
    pub private_type_leaks: usize,
    /// Unused enum constants.
    pub unused_enum_members: usize,
    /// Unused private class-like members.
    pub unused_class_members: usize,
    /// Public API symbols exported from multiple declarations.
    pub duplicate_exports: usize,
    /// Typed `GoRouter` route path or name collisions.
    pub route_collisions: usize,
    /// Private classes extending Flutter widget bases.
    pub private_widget_classes: usize,
    /// Top-level Flutter widget helper functions.
    pub widget_top_level_functions: usize,
    /// Flutter widget constructor params that are never read.
    pub unused_widget_params: usize,
    /// Flutter widget classes with no reachable object construction.
    pub unrendered_widgets: usize,
    /// Widget or `State` awaits missing an immediate `context.mounted` guard.
    pub missing_context_mounted_after_await: usize,
    /// Duplicated Dart code clone groups.
    pub code_duplications: usize,
    /// Dart source lines included in duplicate analysis.
    pub duplication_analyzed_lines: usize,
    /// Unique Dart lines covered by duplicate clone groups.
    pub duplicated_lines: usize,
    /// Duplicated line percentage in percentage basis points.
    pub duplication_percentage_basis_points: u32,
    /// Duplicate threshold percentage in percentage basis points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duplication_threshold_basis_points: Option<u32>,
    /// Whether duplicated lines exceeded the configured threshold.
    pub duplication_threshold_exceeded: bool,
    /// Aggregate 0-100 project health score; higher is healthier.
    pub quality_score: usize,
    /// Dart source files included in health analysis.
    pub health_files: usize,
    /// Function-like declarations included in health analysis.
    pub functions: usize,
    /// Functions exceeding complexity thresholds.
    pub complex_functions: usize,
    /// Highest cyclomatic complexity.
    pub max_cyclomatic_complexity: usize,
    /// Highest cognitive complexity.
    pub max_cognitive_complexity: usize,
    /// Files represented in the loaded LCOV report.
    pub coverage_files: usize,
    /// Dart files with no observed covered executable lines.
    pub coverage_gaps: usize,
    /// Functions exceeding the CRAP threshold.
    pub crap_functions: usize,
    /// Highest CRAP score.
    pub max_crap_score: usize,
    /// File health scores reported.
    pub file_scores: usize,
    /// Health hotspots reported.
    pub hotspots: usize,
    /// Refactoring targets reported.
    pub refactoring_targets: usize,
    /// Feature flags reported.
    pub feature_flags: usize,
    /// Total feature flag occurrences detected before `--top` truncation.
    pub feature_flag_occurrences: usize,
    /// Security candidate groups reported.
    pub security_candidates: usize,
    /// Total security candidate occurrences detected before `--top` truncation.
    pub security_candidate_occurrences: usize,
    /// Attack-surface inventory entries.
    pub attack_surface: usize,
    /// Missing requested entry points.
    pub missing_entry_points: usize,
    /// Circular dependency components.
    pub cycles: usize,
    /// Export-only circular dependency components.
    pub re_export_cycles: usize,
    /// Architecture boundary violations.
    pub boundary_violations: usize,
    /// Dart files outside every configured architecture boundary zone.
    pub boundary_coverage: usize,
    /// Forbidden direct calls from configured architecture zones.
    pub boundary_call_violations: usize,
    /// Declarative policy rule-pack violations.
    pub policy_violations: usize,
    /// Suppression comments missing required justification text.
    pub missing_suppression_reasons: usize,
    /// Changed-code audit risk score from 0-100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_score: Option<usize>,
    /// Changed-code audit risk band.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_level: Option<AuditRiskLevel>,
    /// Changed-code finding attribution split.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<AuditAttribution>,
    /// Total finding count.
    pub findings: usize,
}

/// Agent-actionable finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    /// Stable rule id.
    pub rule_id: String,
    /// Stable finding fingerprint when the rule exposes one.
    pub fingerprint: Option<String>,
    /// Finding category.
    pub kind: FindingKind,
    /// Severity for gates.
    pub severity: Severity,
    /// Human-readable description.
    pub message: String,
    /// Primary file path, root-relative where possible.
    pub path: String,
    /// 1-based line.
    pub line: usize,
    /// 0-based byte column.
    pub column: usize,
    /// Whether the finding can be deleted safely from graph evidence alone.
    pub safe_to_delete: bool,
    /// Related files, root-relative where possible.
    pub files: Vec<String>,
    /// Related dependency edge when applicable.
    pub edge: Option<FindingEdge>,
    /// Suggested agent actions.
    pub actions: Vec<FindingAction>,
}

/// Finding category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FindingKind {
    /// Unreachable Dart file.
    DeadFile,
    /// Unused public top-level declaration.
    UnusedExport,
    /// Unused public top-level type alias.
    UnusedType,
    /// Public API signature exposes a same-library private type.
    PrivateTypeLeak,
    /// Unused enum constant.
    UnusedEnumMember,
    /// Unused private class-like member.
    UnusedClassMember,
    /// Public API entry exposes multiple declarations with the same name.
    DuplicateExport,
    /// Typed `GoRouter` routes declare the same path or route name.
    RouteCollision,
    /// Private class extends a Flutter widget base.
    PrivateWidgetClass,
    /// Top-level function belongs behind a Flutter widget boundary.
    WidgetTopLevelFunctionBoundary,
    /// Flutter widget constructor param is never read by the widget.
    UnusedWidgetParam,
    /// Flutter widget class is never constructed from reachable production code.
    UnrenderedWidget,
    /// Widget or `State` await lacks an immediate `context.mounted` guard.
    MissingContextMountedAfterAwait,
    /// Missing entry point.
    MissingEntryPoint,
    /// Strongly connected dependency component.
    CircularDependency,
    /// Strongly connected component composed only of export edges.
    ReExportCycle,
    /// Directory boundary violation.
    BoundaryViolation,
    /// Source file not covered by a configured boundary zone.
    BoundaryCoverage,
    /// Zoned file calls a forbidden callee.
    BoundaryCallViolation,
    /// Declarative policy pack violation.
    PolicyViolation,
    /// Local import/export/part/augment target was absent.
    UnresolvedDependency,
    /// Dart `part` and `part of` directives disagree about library membership.
    PartOfViolation,
    /// Declared pub dependency has no Dart import/export usage.
    UnusedDependency,
    /// Declared dev dependency has no Dart import/export usage.
    UnusedDevDependency,
    /// Runtime dependency is imported only from dev/test files.
    TestOnlyDependency,
    /// Dependency override is absent from the resolved lockfile package graph.
    UnusedDependencyOverride,
    /// Dependency override key or value cannot be honored by Pub.
    MisconfiguredDependencyOverride,
    /// Imported pub package is missing from pubspec.
    UnlistedDependency,
    /// Import reaches into another package's private `lib/src` implementation tree.
    PrivateSrcImport,
    /// Duplicated Dart code block.
    CodeDuplication,
    /// Function exceeded the cyclomatic complexity threshold.
    HighCyclomaticComplexity,
    /// Function exceeded the cognitive complexity threshold.
    HighCognitiveComplexity,
    /// Function exceeded both complexity thresholds.
    HighComplexity,
    /// Dart file has no observed covered executable lines.
    CoverageGap,
    /// Function exceeded the CRAP threshold.
    HighCrapScore,
    /// File is a low-scoring health hotspot.
    HealthHotspot,
    /// File is a prioritized refactoring target.
    RefactoringTarget,
    /// Feature flag reference.
    FeatureFlag,
    /// Security review candidate.
    SecurityCandidate,
    /// Suppression comment did not match any finding.
    StaleSuppression,
    /// Suppression comment is missing a required reason.
    MissingSuppressionReason,
}

/// Finding severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// Fails the command verdict.
    Error,
    /// Reports without failing the command verdict.
    Warning,
}

/// Dependency edge attached to a finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingEdge {
    /// Source file.
    pub from: String,
    /// Target file when resolved, or attempted target when unresolved.
    pub to: String,
    /// Raw import/export/part/augment URI.
    pub specifier: String,
    /// `import`, `export`, `part`, or `augment`.
    pub kind: String,
}

/// Suggested follow-up action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingAction {
    /// Stable action id.
    pub action: String,
    /// Fallow-compatible action discriminator.
    #[serde(rename = "type")]
    action_type: String,
    /// Human-readable action description.
    pub description: String,
    /// Whether an agent can apply the action without semantic review.
    pub auto_fixable: bool,
    /// Read-only command to collect more evidence before acting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Argument vector for executing `command` without shell parsing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub argv: Vec<String>,
    /// Root-relative file path targeted by the action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_path: Option<String>,
    /// Top-level symbol targeted by the action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_symbol: Option<String>,
    /// Pub dependency targeted by the action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_dependency: Option<String>,
    /// 1-based last source line targeted by the action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_end_line: Option<usize>,
    /// Inline suppression comment an agent can insert after review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppression_comment: Option<String>,
    /// Configuration key targeted by the action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_key: Option<String>,
    /// Human-readable value schema for config-edit actions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_schema: Option<String>,
}

impl FindingAction {
    /// Build a stable finding action.
    #[must_use]
    pub fn new(
        action: impl Into<String>,
        description: impl Into<String>,
        auto_fixable: bool,
    ) -> Self {
        let action = action.into();
        Self {
            action_type: action.clone(),
            action,
            description: description.into(),
            auto_fixable,
            command: None,
            argv: Vec::new(),
            target_path: None,
            target_symbol: None,
            target_dependency: None,
            target_end_line: None,
            suppression_comment: None,
            config_key: None,
            value_schema: None,
        }
    }

    /// Attach a Dart Decimate command with both shell-safe text and argv forms.
    #[must_use]
    pub fn with_dart_decimate_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.argv = std::iter::once("dart-decimate".to_owned())
            .chain(args.into_iter().map(Into::into))
            .collect();
        self.command = Some(shell_command(&self.argv));
        self
    }

    /// Attach a root-relative target file path.
    #[must_use]
    pub fn with_target_path(mut self, path: impl Into<String>) -> Self {
        self.target_path = Some(path.into());
        self
    }

    /// Attach a top-level target symbol.
    #[must_use]
    pub fn with_target_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.target_symbol = Some(symbol.into());
        self
    }

    /// Attach a pub dependency target.
    #[must_use]
    pub fn with_target_dependency(mut self, dependency: impl Into<String>) -> Self {
        self.target_dependency = Some(dependency.into());
        self
    }

    /// Attach a 1-based last source line for range edits.
    #[must_use]
    pub fn with_target_end_line(mut self, line: usize) -> Self {
        self.target_end_line = Some(line);
        self
    }

    /// Attach an inline suppression comment.
    #[must_use]
    pub fn with_suppression_comment(mut self, comment: impl Into<String>) -> Self {
        self.suppression_comment = Some(comment.into());
        self
    }

    /// Attach a config key.
    #[must_use]
    pub fn with_config_key(mut self, config_key: impl Into<String>) -> Self {
        self.config_key = Some(config_key.into());
        self
    }

    /// Attach a human-readable value schema for config-edit actions.
    #[must_use]
    pub fn with_value_schema(mut self, value_schema: impl Into<String>) -> Self {
        self.value_schema = Some(value_schema.into());
        self
    }
}

fn shell_command(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| shell_escape(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_escape(arg: &str) -> String {
    if arg.chars().all(is_shell_safe) {
        return arg.to_owned();
    }
    format!("'{}'", arg.replace('\'', "'\"'\"'"))
}

fn is_shell_safe(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(
            character,
            '/' | '.' | ':' | '_' | '-' | '=' | '+' | ',' | '@'
        )
}

/// Read-only follow-up command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NextStep {
    /// Stable next-step id.
    pub id: String,
    /// Runnable command from the project root.
    pub command: String,
    /// Why this command should be run.
    pub reason: String,
}
