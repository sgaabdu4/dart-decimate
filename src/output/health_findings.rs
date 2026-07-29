use std::path::Path;

use serde::{Deserialize, Serialize};

use super::format::display_path;
use super::{Finding, FindingAction, FindingKind, Severity};
use crate::{
    ComplexityContribution, ComplexityFinding, ComplexityFunctionKind, ComplexityRule,
    CoverageGapFinding, CoverageGapReason, CrapFinding, EffectiveThresholds, FileCoverageStatus,
    FileHealthScore, HealthHotspot, HealthReport, HealthThresholdOverrideReport,
    HealthThresholdOverrideStatus, LargeFunction, RefactoringTarget, ThresholdSource,
};

/// Complexity finding serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonComplexityFinding {
    /// Rule identifier.
    pub rule_id: String,
    /// Dart file path, root-relative where possible.
    pub path: String,
    /// Function, method, getter, setter, constructor, or closure name.
    pub symbol: String,
    /// Function-like declaration kind.
    pub kind: String,
    /// 1-based start line.
    pub line: usize,
    /// 0-based byte column.
    pub column: usize,
    /// Cyclomatic complexity score.
    pub cyclomatic_complexity: usize,
    /// Cognitive complexity score.
    pub cognitive_complexity: usize,
    /// Rounded line coverage percentage when coverage data is available.
    pub line_coverage_percent: Option<usize>,
    /// Covered executable lines when coverage data is available.
    pub covered_lines: Option<usize>,
    /// Executable lines when coverage data is available.
    pub executable_lines: Option<usize>,
    /// CRAP score when coverage data is available.
    pub crap_score: Option<usize>,
    /// Coverage status for this health record.
    pub coverage_status: Option<String>,
    /// Effective thresholds used by a matching override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_thresholds: Option<JsonEffectiveThresholds>,
    /// Source of effective thresholds when not global defaults.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold_source: Option<String>,
    /// Configured reason for the threshold override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold_reason: Option<String>,
    /// Decision-point breakdown when requested.
    pub contributions: Vec<JsonComplexityContribution>,
}

/// Advisory large function serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonLargeFunction {
    /// Stable advisory identifier.
    pub rule_id: String,
    /// Dart file path, root-relative where possible.
    pub path: String,
    /// Function-like declaration name.
    pub symbol: String,
    /// Function-like declaration kind.
    pub kind: String,
    /// 1-based start line.
    pub line: usize,
    /// 0-based byte column.
    pub column: usize,
    /// Inclusive 1-based ending line.
    pub end_line: usize,
    /// Inclusive physical line count.
    pub line_count: usize,
    /// Effective inclusive physical-line ceiling.
    pub max_unit_size: usize,
    /// Source of the effective threshold when overridden.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold_source: Option<String>,
    /// Configured reason for the threshold override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold_reason: Option<String>,
    /// Refactoring guidance appropriate to the declaration kind.
    pub guidance: String,
}

/// Effective thresholds serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonEffectiveThresholds {
    /// Cyclomatic ceiling used for this function.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_cyclomatic: Option<usize>,
    /// Cognitive ceiling used for this function.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_cognitive: Option<usize>,
    /// CRAP ceiling used for this function.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_crap: Option<usize>,
    /// Inclusive physical-line ceiling used for this function.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_unit_size: Option<usize>,
}

/// Threshold override state serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonThresholdOverride {
    /// Zero-based index in `health.thresholdOverrides`.
    pub index: usize,
    /// File globs from config.
    pub files: Vec<String>,
    /// Exact function names from config.
    pub functions: Vec<String>,
    /// Local cyclomatic ceiling.
    pub max_cyclomatic: Option<usize>,
    /// Local cognitive ceiling.
    pub max_cognitive: Option<usize>,
    /// Local CRAP ceiling.
    pub max_crap: Option<usize>,
    /// Local inclusive physical-line ceiling.
    pub max_unit_size: Option<usize>,
    /// Configured reason.
    pub reason: Option<String>,
    /// Whether this override currently changes or explains threshold output.
    pub active: bool,
    /// Whether this override matched code but is no longer needed.
    pub stale: bool,
    /// Whether this override matched no analyzed functions.
    pub no_match: bool,
    /// Matched root-relative `file:symbol` entries.
    pub matched_functions: Vec<String>,
}

/// One decision point serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonComplexityContribution {
    /// 1-based line.
    pub line: usize,
    /// 0-based byte column.
    pub column: usize,
    /// Decision-point kind.
    pub kind: String,
    /// Cyclomatic score added by this point.
    pub cyclomatic: usize,
    /// Cognitive score added by this point.
    pub cognitive: usize,
    /// Nesting depth at this point.
    pub nesting: usize,
}

/// File health score serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonFileHealthScore {
    /// Dart file path, root-relative where possible.
    pub path: String,
    /// 0-100 score; higher is healthier.
    pub score: usize,
    /// Function-like declarations in the file.
    pub functions: usize,
    /// Functions exceeding static complexity or CRAP thresholds.
    pub complex_functions: usize,
    /// Highest cyclomatic complexity in the file.
    pub max_cyclomatic_complexity: usize,
    /// Highest cognitive complexity in the file.
    pub max_cognitive_complexity: usize,
    /// Highest CRAP score in the file.
    pub max_crap_score: usize,
    /// Coverage status.
    pub coverage_status: String,
    /// Covered executable lines when coverage data is available.
    pub covered_lines: Option<usize>,
    /// Executable lines when coverage data is available.
    pub executable_lines: Option<usize>,
    /// Rounded line coverage percentage when coverage data is available.
    pub line_coverage_percent: Option<usize>,
    /// Agent-readable score reasons.
    pub reasons: Vec<String>,
    /// CODEOWNERS owners, when `--ownership` is enabled.
    pub owners: Vec<String>,
    /// CODEOWNERS file that produced the owner match.
    pub owner_source: Option<String>,
    /// GitLab CODEOWNERS section, when present.
    pub owner_section: Option<String>,
}

/// Health hotspot serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonHealthHotspot {
    /// Dart file path, root-relative where possible.
    pub path: String,
    /// 1-based line.
    pub line: usize,
    /// 0-based byte column.
    pub column: usize,
    /// 0-100 score; higher is healthier.
    pub score: usize,
    /// Agent-readable hotspot reasons.
    pub reasons: Vec<String>,
    /// CODEOWNERS owners, when `--ownership` is enabled.
    pub owners: Vec<String>,
    /// CODEOWNERS file that produced the owner match.
    pub owner_source: Option<String>,
    /// GitLab CODEOWNERS section, when present.
    pub owner_section: Option<String>,
}

/// Refactoring target serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRefactoringTarget {
    /// Dart file path, root-relative where possible.
    pub path: String,
    /// 1-based line.
    pub line: usize,
    /// 0-based byte column.
    pub column: usize,
    /// 0-100 score; higher is healthier.
    pub score: usize,
    /// Priority score used for deterministic ordering.
    pub priority: usize,
    /// Agent-readable target reasons.
    pub reasons: Vec<String>,
    /// CODEOWNERS owners, when `--ownership` is enabled.
    pub owners: Vec<String>,
    /// CODEOWNERS file that produced the owner match.
    pub owner_source: Option<String>,
    /// GitLab CODEOWNERS section, when present.
    pub owner_section: Option<String>,
}

pub(super) fn add_health_findings(root: &Path, report: &HealthReport, findings: &mut Vec<Finding>) {
    findings.extend(
        report
            .complexity
            .iter()
            .map(|finding| complexity_finding(root, finding)),
    );
    findings.extend(
        report
            .coverage_gaps
            .iter()
            .map(|finding| coverage_gap_finding(root, finding)),
    );
    findings.extend(
        report
            .crap
            .iter()
            .map(|finding| crap_finding(root, finding)),
    );
    findings.extend(
        report
            .hotspots
            .iter()
            .map(|finding| hotspot_finding(root, finding)),
    );
    findings.extend(
        report
            .refactoring_targets
            .iter()
            .map(|finding| refactoring_target_finding(root, finding)),
    );
    if let Some(style) = &report.flutter_style {
        super::flutter_style::add_flutter_style_findings(root, style, findings);
    }
}

pub(super) fn json_complexity(root: &Path, report: &HealthReport) -> Vec<JsonComplexityFinding> {
    let mut findings = report
        .complexity
        .iter()
        .map(|finding| json_complexity_finding(root, finding))
        .collect::<Vec<_>>();
    findings.extend(
        report
            .crap
            .iter()
            .map(|finding| json_crap_finding(root, finding)),
    );
    findings
}

pub(super) fn json_large_functions(root: &Path, report: &HealthReport) -> Vec<JsonLargeFunction> {
    report
        .large_functions
        .iter()
        .map(|function| json_large_function(root, function))
        .collect()
}

pub(super) fn json_file_scores(root: &Path, report: &HealthReport) -> Vec<JsonFileHealthScore> {
    report
        .file_scores
        .iter()
        .map(|score| json_file_score(root, score))
        .collect()
}

pub(super) fn json_hotspots(root: &Path, report: &HealthReport) -> Vec<JsonHealthHotspot> {
    report
        .hotspots
        .iter()
        .map(|hotspot| JsonHealthHotspot {
            path: display_path(root, &hotspot.path),
            line: hotspot.location.line,
            column: hotspot.location.column,
            score: hotspot.score,
            reasons: hotspot.reasons.clone(),
            owners: hotspot.owners.clone(),
            owner_source: hotspot.owner_source.clone(),
            owner_section: hotspot.owner_section.clone(),
        })
        .collect()
}

pub(super) fn json_refactoring_targets(
    root: &Path,
    report: &HealthReport,
) -> Vec<JsonRefactoringTarget> {
    report
        .refactoring_targets
        .iter()
        .map(|target| JsonRefactoringTarget {
            path: display_path(root, &target.path),
            line: target.location.line,
            column: target.location.column,
            score: target.score,
            priority: target.priority,
            reasons: target.reasons.clone(),
            owners: target.owners.clone(),
            owner_source: target.owner_source.clone(),
            owner_section: target.owner_section.clone(),
        })
        .collect()
}

pub(super) fn json_threshold_overrides(report: &HealthReport) -> Vec<JsonThresholdOverride> {
    report
        .threshold_overrides
        .iter()
        .map(json_threshold_override)
        .collect()
}

fn complexity_finding(root: &Path, finding: &ComplexityFinding) -> Finding {
    let path = display_path(root, &finding.path);
    Finding {
        rule_id: finding.rule.rule_id().to_owned(),
        fingerprint: None,
        kind: finding_kind(finding.rule),
        severity: Severity::Error,
        message: format!(
            "{} has cyclomatic complexity {} and cognitive complexity {}",
            finding.symbol, finding.cyclomatic_complexity, finding.cognitive_complexity
        ),
        path: path.clone(),
        line: finding.location.line,
        column: finding.location.column,
        safe_to_delete: false,
        files: Vec::new(),
        edge: None,
        actions: vec![
            FindingAction::new(
                "refactor-function",
                "Split branches into smaller functions or move policy to the owning module",
                false,
            )
            .with_target_path(path.clone())
            .with_target_symbol(finding.symbol.clone())
            .with_dart_decimate_args(["inspect", "--format", "json", "--file", path.as_str()])
            .with_suppression_comment("// dart-decimate-ignore-next-line complexity"),
        ],
    }
}

fn coverage_gap_finding(root: &Path, finding: &CoverageGapFinding) -> Finding {
    let path = display_path(root, &finding.path);
    Finding {
        rule_id: "dart-decimate/coverage-gap".to_owned(),
        fingerprint: Some(format!("coverage-gap:{path}")),
        kind: FindingKind::CoverageGap,
        severity: Severity::Error,
        message: format!(
            "{} has no covered executable lines ({})",
            path,
            coverage_gap_reason(finding.reason)
        ),
        path: path.clone(),
        line: finding.location.line,
        column: finding.location.column,
        safe_to_delete: false,
        files: Vec::new(),
        edge: None,
        actions: vec![
            FindingAction::new(
                "add-test",
                "Add or run tests that execute this Dart file, then refresh LCOV",
                false,
            )
            .with_target_path(path.clone())
            .with_dart_decimate_args([
                "inspect",
                "--format",
                "json",
                "--file",
                path.as_str(),
            ]),
        ],
    }
}

fn crap_finding(root: &Path, finding: &CrapFinding) -> Finding {
    let path = display_path(root, &finding.path);
    Finding {
        rule_id: "dart-decimate/high-crap-score".to_owned(),
        fingerprint: Some(format!(
            "high-crap-score:{}:{}:{:?}",
            path, finding.symbol, finding.kind
        )),
        kind: FindingKind::HighCrapScore,
        severity: Severity::Error,
        message: format!(
            "{} has CRAP score {} with {}% line coverage",
            finding.symbol, finding.crap_score, finding.line_coverage_percent
        ),
        path: path.clone(),
        line: finding.location.line,
        column: finding.location.column,
        safe_to_delete: false,
        files: Vec::new(),
        edge: None,
        actions: vec![
            FindingAction::new(
                "refactor-or-test",
                "Reduce branching or add targeted tests covering this function",
                false,
            )
            .with_target_path(path.clone())
            .with_target_symbol(finding.symbol.clone())
            .with_dart_decimate_args(["inspect", "--format", "json", "--file", path.as_str()])
            .with_suppression_comment("// dart-decimate-ignore-next-line complexity"),
        ],
    }
}

fn hotspot_finding(root: &Path, finding: &HealthHotspot) -> Finding {
    let path = display_path(root, &finding.path);
    Finding {
        rule_id: "dart-decimate/health-hotspot".to_owned(),
        fingerprint: Some(format!("health-hotspot:{path}")),
        kind: FindingKind::HealthHotspot,
        severity: Severity::Error,
        message: format!("{} has health score {}", path, finding.score),
        path: path.clone(),
        line: finding.location.line,
        column: finding.location.column,
        safe_to_delete: false,
        files: Vec::new(),
        edge: None,
        actions: vec![
            FindingAction::new(
                "review-file-health",
                "Review the file score reasons before refactoring",
                false,
            )
            .with_target_path(path.clone())
            .with_dart_decimate_args([
                "inspect",
                "--format",
                "json",
                "--file",
                path.as_str(),
            ]),
        ],
    }
}

fn refactoring_target_finding(root: &Path, finding: &RefactoringTarget) -> Finding {
    let path = display_path(root, &finding.path);
    Finding {
        rule_id: "dart-decimate/refactoring-target".to_owned(),
        fingerprint: Some(format!("refactoring-target:{path}")),
        kind: FindingKind::RefactoringTarget,
        severity: Severity::Error,
        message: format!(
            "{} is a refactoring target with priority {} and health score {}",
            path, finding.priority, finding.score
        ),
        path: path.clone(),
        line: finding.location.line,
        column: finding.location.column,
        safe_to_delete: false,
        files: Vec::new(),
        edge: None,
        actions: vec![
            FindingAction::new(
                "refactor-target",
                "Split complex functions or isolate policy before editing",
                false,
            )
            .with_target_path(path.clone())
            .with_dart_decimate_args([
                "inspect",
                "--format",
                "json",
                "--file",
                path.as_str(),
            ]),
        ],
    }
}

fn finding_kind(rule: ComplexityRule) -> FindingKind {
    match rule {
        ComplexityRule::HighCyclomaticComplexity => FindingKind::HighCyclomaticComplexity,
        ComplexityRule::HighCognitiveComplexity => FindingKind::HighCognitiveComplexity,
        ComplexityRule::HighComplexity => FindingKind::HighComplexity,
    }
}

fn json_complexity_finding(root: &Path, finding: &ComplexityFinding) -> JsonComplexityFinding {
    JsonComplexityFinding {
        rule_id: finding.rule.rule_id().to_owned(),
        path: display_path(root, &finding.path),
        symbol: finding.symbol.clone(),
        kind: function_kind(finding.kind).to_owned(),
        line: finding.location.line,
        column: finding.location.column,
        cyclomatic_complexity: finding.cyclomatic_complexity,
        cognitive_complexity: finding.cognitive_complexity,
        line_coverage_percent: None,
        covered_lines: None,
        executable_lines: None,
        crap_score: None,
        coverage_status: None,
        effective_thresholds: finding
            .effective_thresholds
            .as_ref()
            .map(json_effective_thresholds),
        threshold_source: finding
            .threshold_source
            .map(threshold_source)
            .map(str::to_owned),
        threshold_reason: finding.threshold_reason.clone(),
        contributions: finding
            .contributions
            .iter()
            .map(json_contribution)
            .collect(),
    }
}

fn json_crap_finding(root: &Path, finding: &CrapFinding) -> JsonComplexityFinding {
    JsonComplexityFinding {
        rule_id: "dart-decimate/high-crap-score".to_owned(),
        path: display_path(root, &finding.path),
        symbol: finding.symbol.clone(),
        kind: function_kind(finding.kind).to_owned(),
        line: finding.location.line,
        column: finding.location.column,
        cyclomatic_complexity: finding.cyclomatic_complexity,
        cognitive_complexity: finding.cognitive_complexity,
        line_coverage_percent: Some(finding.line_coverage_percent),
        covered_lines: Some(finding.covered_lines),
        executable_lines: Some(finding.executable_lines),
        crap_score: Some(finding.crap_score),
        coverage_status: Some("covered".to_owned()),
        effective_thresholds: finding
            .effective_thresholds
            .as_ref()
            .map(json_effective_thresholds),
        threshold_source: finding
            .threshold_source
            .map(threshold_source)
            .map(str::to_owned),
        threshold_reason: finding.threshold_reason.clone(),
        contributions: Vec::new(),
    }
}

fn json_large_function(root: &Path, function: &LargeFunction) -> JsonLargeFunction {
    let flutter_build = function.kind == crate::ComplexityFunctionKind::FlutterBuildMethod;
    JsonLargeFunction {
        rule_id: "dart-decimate/large-function".to_owned(),
        path: display_path(root, &function.path),
        symbol: function.symbol.clone(),
        kind: function_kind(function.kind).to_owned(),
        line: function.location.line,
        column: function.location.column,
        end_line: function.end_line,
        line_count: function.line_count,
        max_unit_size: function.max_unit_size,
        threshold_source: function
            .threshold_source
            .map(threshold_source)
            .map(str::to_owned),
        threshold_reason: function.threshold_reason.clone(),
        guidance: if flutter_build {
            "Split the Flutter build method into focused widgets or helper owners".to_owned()
        } else {
            "Extract cohesive responsibilities into smaller functions or owning modules".to_owned()
        },
    }
}

fn json_effective_thresholds(thresholds: &EffectiveThresholds) -> JsonEffectiveThresholds {
    JsonEffectiveThresholds {
        max_cyclomatic: thresholds.max_cyclomatic,
        max_cognitive: thresholds.max_cognitive,
        max_crap: thresholds.max_crap,
        max_unit_size: thresholds.max_unit_size,
    }
}

fn json_threshold_override(report: &HealthThresholdOverrideReport) -> JsonThresholdOverride {
    JsonThresholdOverride {
        index: report.index,
        files: report.files.clone(),
        functions: report.functions.clone(),
        max_cyclomatic: report.max_cyclomatic,
        max_cognitive: report.max_cognitive,
        max_crap: report.max_crap,
        max_unit_size: report.max_unit_size,
        reason: report.reason.clone(),
        active: report.status == HealthThresholdOverrideStatus::Active,
        stale: report.status == HealthThresholdOverrideStatus::Stale,
        no_match: report.status == HealthThresholdOverrideStatus::NoMatch,
        matched_functions: report.matched_functions.clone(),
    }
}

const fn threshold_source(source: ThresholdSource) -> &'static str {
    match source {
        ThresholdSource::Override => "override",
    }
}

const fn function_kind(kind: ComplexityFunctionKind) -> &'static str {
    match kind {
        ComplexityFunctionKind::Function => "function",
        ComplexityFunctionKind::Getter => "getter",
        ComplexityFunctionKind::Setter => "setter",
        ComplexityFunctionKind::Method => "method",
        ComplexityFunctionKind::FlutterBuildMethod => "flutter-build-method",
        ComplexityFunctionKind::Constructor => "constructor",
        ComplexityFunctionKind::Operator => "operator",
        ComplexityFunctionKind::Closure => "closure",
    }
}

fn json_file_score(root: &Path, score: &FileHealthScore) -> JsonFileHealthScore {
    JsonFileHealthScore {
        path: display_path(root, &score.path),
        score: score.score,
        functions: score.functions,
        complex_functions: score.complex_functions,
        max_cyclomatic_complexity: score.max_cyclomatic_complexity,
        max_cognitive_complexity: score.max_cognitive_complexity,
        max_crap_score: score.max_crap_score,
        coverage_status: file_coverage_status(score.coverage_status).to_owned(),
        covered_lines: score.covered_lines,
        executable_lines: score.executable_lines,
        line_coverage_percent: score.line_coverage_percent,
        reasons: score.reasons.clone(),
        owners: score.owners.clone(),
        owner_source: score.owner_source.clone(),
        owner_section: score.owner_section.clone(),
    }
}

fn json_contribution(contribution: &ComplexityContribution) -> JsonComplexityContribution {
    JsonComplexityContribution {
        line: contribution.location.line,
        column: contribution.location.column,
        kind: contribution.kind.clone(),
        cyclomatic: contribution.cyclomatic,
        cognitive: contribution.cognitive,
        nesting: contribution.nesting,
    }
}

fn coverage_gap_reason(reason: CoverageGapReason) -> &'static str {
    match reason {
        CoverageGapReason::MissingFromCoverage => "missing from coverage",
        CoverageGapReason::NoExecutableLines => "no executable lines in LCOV",
        CoverageGapReason::ZeroCoveredLines => "zero covered lines",
    }
}

fn file_coverage_status(status: FileCoverageStatus) -> &'static str {
    match status {
        FileCoverageStatus::NotRequested => "not-requested",
        FileCoverageStatus::Missing => "missing",
        FileCoverageStatus::NoExecutableLines => "no-executable-lines",
        FileCoverageStatus::Uncovered => "uncovered",
        FileCoverageStatus::Covered => "covered",
    }
}
