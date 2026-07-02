use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::finding_identity::finding_identity;
use crate::output::{Finding, FindingKind, JsonAttackSurfaceEntry, JsonReport, Severity, Verdict};
use crate::{JsonCloneGroup, JsonComplexityFinding, JsonFeatureFlag, JsonSecurityCandidate};

pub const BASELINE_SCHEMA_VERSION: &str = "dart-decimate.baseline.v1";
pub const REGRESSION_BASELINE_SCHEMA_VERSION: &str = "dart-decimate.regression-baseline.v1";

/// Identity-based finding baseline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Baseline {
    /// Baseline schema identifier.
    pub schema_version: String,
    /// Tool that created this baseline.
    pub tool: String,
    /// Stable finding identities captured by the baseline.
    pub findings: Vec<BaselineFinding>,
}

/// One persisted finding identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaselineFinding {
    /// Stable identity string used for matching future findings.
    pub identity: String,
    /// Stable rule id.
    pub rule_id: String,
    /// Rule-provided fingerprint, when the finding exposes one.
    pub fingerprint: Option<String>,
    /// Finding category.
    pub kind: FindingKind,
    /// Root-relative finding path.
    pub path: String,
}

/// Count-based regression baseline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegressionBaseline {
    /// Baseline schema identifier.
    pub schema_version: String,
    /// Tool that created this baseline.
    pub tool: String,
    /// Command that produced the baseline.
    pub command: String,
    /// Per-rule and total finding counts.
    pub counts: RegressionCounts,
}

/// Count snapshot used for regression comparison.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegressionCounts {
    /// Total visible finding count.
    pub findings: usize,
    /// Visible finding counts keyed by stable rule id.
    pub rules: BTreeMap<String, usize>,
}

/// Allowed count increase before a regression is reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegressionTolerance {
    /// Absolute issue-count increase.
    Absolute(usize),
    /// Percentage increase in basis points, where 10000 is 100%.
    PercentBasisPoints(u32),
}

impl Default for RegressionTolerance {
    fn default() -> Self {
        Self::Absolute(0)
    }
}

/// Count-based regression comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegressionComparison {
    /// Total finding-count change.
    pub total: RegressionCountDelta,
    /// Per-rule count changes.
    pub rules: Vec<RegressionCountDelta>,
}

impl RegressionComparison {
    /// Whether any count increased beyond tolerance.
    #[must_use]
    pub fn regressed(&self) -> bool {
        self.total.regressed || self.rules.iter().any(|delta| delta.regressed)
    }
}

/// One regression count delta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegressionCountDelta {
    /// Count name or rule id.
    pub name: String,
    /// Baseline count.
    pub baseline: usize,
    /// Current count.
    pub current: usize,
    /// Allowed increase.
    pub allowed_increase: usize,
    /// Whether current count exceeded baseline plus allowed increase.
    pub regressed: bool,
}

/// Errors returned while loading or saving baselines.
#[derive(Debug, Error)]
pub enum BaselineError {
    /// Baseline file could not be read.
    #[error("failed to read baseline {path}: {source}")]
    Read {
        /// Baseline path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Baseline file could not be parsed.
    #[error("failed to parse baseline {path}: {source}")]
    Parse {
        /// Baseline path.
        path: PathBuf,
        /// JSON parse error.
        source: serde_json::Error,
    },
    /// Baseline file had an unsupported schema.
    #[error("unsupported baseline schema {schema_version:?} in {path}")]
    Schema {
        /// Baseline path.
        path: PathBuf,
        /// Unsupported schema version.
        schema_version: String,
    },
    /// Baseline parent directory could not be created.
    #[error("failed to create baseline directory {path}: {source}")]
    CreateDir {
        /// Directory path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Baseline file could not be written.
    #[error("failed to write baseline {path}: {source}")]
    Write {
        /// Baseline path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Baseline JSON could not be serialized.
    #[error("failed to render baseline {path}: {source}")]
    Json {
        /// Baseline path.
        path: PathBuf,
        /// JSON serialization error.
        source: serde_json::Error,
    },
}

/// Build a baseline from the visible findings in a report.
#[must_use]
pub fn baseline_from_report(report: &JsonReport) -> Baseline {
    let mut findings = report
        .findings
        .iter()
        .map(BaselineFinding::from)
        .collect::<Vec<_>>();
    findings.sort_by(|left, right| left.identity.cmp(&right.identity));
    findings.dedup_by(|left, right| left.identity == right.identity);

    Baseline {
        schema_version: BASELINE_SCHEMA_VERSION.to_owned(),
        tool: "dart-decimate".to_owned(),
        findings,
    }
}

/// Build a count-based regression baseline from the visible findings in a report.
#[must_use]
pub fn regression_baseline_from_report(report: &JsonReport) -> RegressionBaseline {
    RegressionBaseline {
        schema_version: REGRESSION_BASELINE_SCHEMA_VERSION.to_owned(),
        tool: "dart-decimate".to_owned(),
        command: report.command.as_str().to_owned(),
        counts: regression_counts(report),
    }
}

/// Load a baseline from disk.
///
/// # Errors
///
/// Returns [`BaselineError`] when the file is unreadable, malformed, or uses an
/// unsupported schema version.
pub fn load_baseline(path: impl AsRef<Path>) -> Result<Baseline, BaselineError> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|source| BaselineError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let baseline =
        serde_json::from_str::<Baseline>(&source).map_err(|source| BaselineError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
    if baseline.schema_version != BASELINE_SCHEMA_VERSION {
        return Err(BaselineError::Schema {
            path: path.to_path_buf(),
            schema_version: baseline.schema_version,
        });
    }
    Ok(baseline)
}

/// Load a count-based regression baseline from disk.
///
/// # Errors
///
/// Returns [`BaselineError`] when the file is unreadable, malformed, or uses an
/// unsupported schema version.
pub fn load_regression_baseline(
    path: impl AsRef<Path>,
) -> Result<RegressionBaseline, BaselineError> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|source| BaselineError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let baseline = serde_json::from_str::<RegressionBaseline>(&source).map_err(|source| {
        BaselineError::Parse {
            path: path.to_path_buf(),
            source,
        }
    })?;
    if baseline.schema_version != REGRESSION_BASELINE_SCHEMA_VERSION {
        return Err(BaselineError::Schema {
            path: path.to_path_buf(),
            schema_version: baseline.schema_version,
        });
    }
    Ok(baseline)
}

/// Save a baseline to disk.
///
/// # Errors
///
/// Returns [`BaselineError`] when the parent directory cannot be created or the
/// file cannot be written.
pub fn save_baseline(path: impl AsRef<Path>, baseline: &Baseline) -> Result<(), BaselineError> {
    save_json(path, baseline)
}

/// Save a count-based regression baseline to disk.
///
/// # Errors
///
/// Returns [`BaselineError`] when the parent directory cannot be created or the
/// file cannot be written.
pub fn save_regression_baseline(
    path: impl AsRef<Path>,
    baseline: &RegressionBaseline,
) -> Result<(), BaselineError> {
    save_json(path, baseline)
}

fn save_json<T: Serialize>(path: impl AsRef<Path>, value: &T) -> Result<(), BaselineError> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|source| BaselineError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }
    }
    let mut json = serde_json::to_string_pretty(value).map_err(|source| BaselineError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    json.push('\n');
    fs::write(path, json).map_err(|source| BaselineError::Write {
        path: path.to_path_buf(),
        source,
    })
}

/// Compare visible report counts against a regression baseline.
#[must_use]
pub fn compare_regression_baseline(
    report: &JsonReport,
    baseline: &RegressionBaseline,
    tolerance: RegressionTolerance,
) -> RegressionComparison {
    let current = regression_counts(report);
    let mut rules = baseline
        .counts
        .rules
        .keys()
        .chain(current.rules.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|rule| {
            regression_delta(
                rule,
                baseline.counts.rules.get(rule).copied().unwrap_or_default(),
                current.rules.get(rule).copied().unwrap_or_default(),
                tolerance,
            )
        })
        .collect::<Vec<_>>();
    rules.sort_by(|left, right| left.name.cmp(&right.name));

    RegressionComparison {
        total: regression_delta(
            "findings",
            baseline.counts.findings,
            current.findings,
            tolerance,
        ),
        rules,
    }
}

fn regression_counts(report: &JsonReport) -> RegressionCounts {
    let mut rules = BTreeMap::<String, usize>::new();
    for finding in &report.findings {
        *rules.entry(finding.rule_id.clone()).or_default() += 1;
    }
    RegressionCounts {
        findings: report.findings.len(),
        rules,
    }
}

fn regression_delta(
    name: &str,
    baseline: usize,
    current: usize,
    tolerance: RegressionTolerance,
) -> RegressionCountDelta {
    let allowed_increase = allowed_increase(baseline, tolerance);
    RegressionCountDelta {
        name: name.to_owned(),
        baseline,
        current,
        allowed_increase,
        regressed: current > baseline.saturating_add(allowed_increase),
    }
}

fn allowed_increase(baseline: usize, tolerance: RegressionTolerance) -> usize {
    match tolerance {
        RegressionTolerance::Absolute(value) => value,
        RegressionTolerance::PercentBasisPoints(value) => {
            let numerator = baseline.saturating_mul(value as usize);
            numerator.saturating_add(9999) / 10000
        }
    }
}

/// Remove findings already present in `baseline` and recompute report verdicts.
pub fn apply_baseline_to_report(report: &mut JsonReport, baseline: &Baseline) {
    let known = baseline
        .findings
        .iter()
        .map(|finding| finding.identity.as_str())
        .collect::<BTreeSet<_>>();
    if known.is_empty() {
        return;
    }

    report
        .findings
        .retain(|finding| !known.contains(finding_identity(finding).as_str()));
    filter_detail_sections(report);
    recompute_summary(report);
    let has_hidden_security_surface = report.has_hidden_security_candidate_occurrences();
    report
        .next_steps
        .retain(|step| next_step_live(&step.id, &report.findings, has_hidden_security_surface));
    report.verdict = if report
        .findings
        .iter()
        .any(|finding| finding.severity == Severity::Error)
    {
        Verdict::Fail
    } else {
        Verdict::Pass
    };
}

impl From<&Finding> for BaselineFinding {
    fn from(finding: &Finding) -> Self {
        Self {
            identity: finding_identity(finding),
            rule_id: finding.rule_id.clone(),
            fingerprint: finding.fingerprint.clone(),
            kind: finding.kind,
            path: finding.path.clone(),
        }
    }
}

fn filter_detail_sections(report: &mut JsonReport) {
    let remaining = report
        .findings
        .iter()
        .map(|finding| {
            (
                finding.kind,
                finding.path.clone(),
                finding.fingerprint.clone(),
            )
        })
        .collect::<Vec<_>>();

    report
        .clone_groups
        .retain(|group| clone_group_live(group, &remaining));
    report
        .complexity
        .retain(|finding| complexity_live(finding, &remaining));
    report
        .hotspots
        .retain(|hotspot| detail_path_live(FindingKind::HealthHotspot, &hotspot.path, &remaining));
    report.refactoring_targets.retain(|target| {
        detail_path_live(FindingKind::RefactoringTarget, &target.path, &remaining)
    });
    report
        .feature_flags
        .retain(|flag| feature_flag_live(flag, &remaining));
    report
        .security_candidates
        .retain(|candidate| security_live(candidate, &remaining));
    report
        .attack_surface
        .retain(|entry| attack_surface_live(entry, &report.security_candidates));
}

fn clone_group_live(
    group: &JsonCloneGroup,
    remaining: &[(FindingKind, String, Option<String>)],
) -> bool {
    remaining.iter().any(|(kind, _, fingerprint)| {
        *kind == FindingKind::CodeDuplication && fingerprint.as_deref() == Some(&group.fingerprint)
    })
}

fn complexity_live(
    finding: &JsonComplexityFinding,
    remaining: &[(FindingKind, String, Option<String>)],
) -> bool {
    remaining.iter().any(|(kind, path, _)| {
        complexity_kind(*kind)
            && path == &finding.path
            && finding.rule_id.ends_with(kind_suffix(*kind))
    })
}

fn detail_path_live(
    expected: FindingKind,
    detail_path: &str,
    remaining: &[(FindingKind, String, Option<String>)],
) -> bool {
    remaining
        .iter()
        .any(|(kind, path, _)| *kind == expected && path == detail_path)
}

fn feature_flag_live(
    flag: &JsonFeatureFlag,
    remaining: &[(FindingKind, String, Option<String>)],
) -> bool {
    flag.occurrences.iter().any(|occurrence| {
        remaining
            .iter()
            .any(|(kind, path, _)| *kind == FindingKind::FeatureFlag && path == &occurrence.path)
    })
}

fn security_live(
    candidate: &JsonSecurityCandidate,
    remaining: &[(FindingKind, String, Option<String>)],
) -> bool {
    remaining.iter().any(|(kind, _, fingerprint)| {
        *kind == FindingKind::SecurityCandidate
            && fingerprint.as_deref() == Some(candidate.fingerprint.as_str())
    })
}

fn attack_surface_live(
    entry: &JsonAttackSurfaceEntry,
    candidates: &[JsonSecurityCandidate],
) -> bool {
    candidates
        .iter()
        .any(|candidate| candidate.category == entry.category)
}

fn next_step_live(id: &str, findings: &[Finding], has_hidden_security_surface: bool) -> bool {
    match id {
        "trace-unused-export" => has_finding_kind(findings, FindingKind::UnusedExport),
        "trace-unused-type" => has_finding_kind(findings, FindingKind::UnusedType),
        "trace-unused-dependency" => findings
            .iter()
            .any(|finding| is_dependency_hygiene_kind(finding.kind)),
        "trace-code-duplication" => has_finding_kind(findings, FindingKind::CodeDuplication),
        "complexity-breakdown" => findings.iter().any(|finding| complexity_kind(finding.kind)),
        "review-security-surface" => {
            has_hidden_security_surface
                && has_finding_kind(findings, FindingKind::SecurityCandidate)
        }
        _ => true,
    }
}

fn has_finding_kind(findings: &[Finding], expected: FindingKind) -> bool {
    findings.iter().any(|finding| finding.kind == expected)
}

fn complexity_kind(kind: FindingKind) -> bool {
    matches!(
        kind,
        FindingKind::HighCyclomaticComplexity
            | FindingKind::HighCognitiveComplexity
            | FindingKind::HighComplexity
            | FindingKind::HighCrapScore
    )
}
fn kind_suffix(kind: FindingKind) -> &'static str {
    match kind {
        FindingKind::HighCyclomaticComplexity => "high-cyclomatic-complexity",
        FindingKind::HighCognitiveComplexity => "high-cognitive-complexity",
        FindingKind::HighComplexity => "high-complexity",
        FindingKind::HighCrapScore => "high-crap-score",
        _ => "",
    }
}
fn recompute_summary(report: &mut JsonReport) {
    report.summary.unresolved_dependencies =
        kind_count(&report.findings, FindingKind::UnresolvedDependency);
    report.summary.part_of_violations = kind_count(&report.findings, FindingKind::PartOfViolation);
    report.summary.unused_dependencies = dependency_count(&report.findings);
    report.summary.unused_dev_dependencies =
        kind_count(&report.findings, FindingKind::UnusedDevDependency);
    report.summary.test_only_dependencies =
        kind_count(&report.findings, FindingKind::TestOnlyDependency);
    report.summary.dependency_overrides = dependency_override_count(&report.findings);
    report.summary.unused_dependency_overrides =
        kind_count(&report.findings, FindingKind::UnusedDependencyOverride);
    report.summary.misconfigured_dependency_overrides = kind_count(
        &report.findings,
        FindingKind::MisconfiguredDependencyOverride,
    );
    report.summary.unlisted_dependencies =
        kind_count(&report.findings, FindingKind::UnlistedDependency);
    report.summary.private_src_imports =
        kind_count(&report.findings, FindingKind::PrivateSrcImport);
    report.summary.dead_files = kind_count(&report.findings, FindingKind::DeadFile);
    report.summary.unused_exports = kind_count(&report.findings, FindingKind::UnusedExport);
    report.summary.unused_types = kind_count(&report.findings, FindingKind::UnusedType);
    report.summary.private_type_leaks = kind_count(&report.findings, FindingKind::PrivateTypeLeak);
    report.summary.unused_enum_members =
        kind_count(&report.findings, FindingKind::UnusedEnumMember);
    report.summary.unused_class_members =
        kind_count(&report.findings, FindingKind::UnusedClassMember);
    report.summary.duplicate_exports = kind_count(&report.findings, FindingKind::DuplicateExport);
    report.summary.route_collisions = kind_count(&report.findings, FindingKind::RouteCollision);
    report.summary.private_widget_classes =
        kind_count(&report.findings, FindingKind::PrivateWidgetClass);
    report.summary.widget_top_level_functions = kind_count(
        &report.findings,
        FindingKind::WidgetTopLevelFunctionBoundary,
    );
    report.summary.unused_widget_params =
        kind_count(&report.findings, FindingKind::UnusedWidgetParam);
    report.summary.unrendered_widgets = kind_count(&report.findings, FindingKind::UnrenderedWidget);
    report.summary.missing_context_mounted_after_await = kind_count(
        &report.findings,
        FindingKind::MissingContextMountedAfterAwait,
    );
    report.summary.code_duplications = report.clone_groups.len();
    report.summary.complex_functions = report.complexity.len();
    report.summary.coverage_gaps = kind_count(&report.findings, FindingKind::CoverageGap);
    report.summary.crap_functions = kind_count(&report.findings, FindingKind::HighCrapScore);
    report.summary.file_scores = report.file_scores.len();
    report.summary.hotspots = report.hotspots.len();
    report.summary.refactoring_targets = report.refactoring_targets.len();
    report.summary.feature_flags = report.feature_flags.len();
    report.summary.feature_flag_occurrences = report
        .feature_flags
        .iter()
        .map(|flag| flag.occurrences.len())
        .sum();
    report.summary.security_candidates = report.security_candidates.len();
    report.summary.security_candidate_occurrences = report
        .security_candidates
        .iter()
        .map(|candidate| candidate.occurrences.len())
        .sum();
    report.summary.attack_surface = report.attack_surface.len();
    report.summary.missing_entry_points =
        kind_count(&report.findings, FindingKind::MissingEntryPoint);
    report.summary.cycles = kind_count(&report.findings, FindingKind::CircularDependency);
    report.summary.re_export_cycles = kind_count(&report.findings, FindingKind::ReExportCycle);
    report.summary.boundary_violations =
        kind_count(&report.findings, FindingKind::BoundaryViolation);
    report.summary.boundary_coverage = kind_count(&report.findings, FindingKind::BoundaryCoverage);
    report.summary.boundary_call_violations =
        kind_count(&report.findings, FindingKind::BoundaryCallViolation);
    report.summary.policy_violations = kind_count(&report.findings, FindingKind::PolicyViolation);
    report.summary.missing_suppression_reasons =
        kind_count(&report.findings, FindingKind::MissingSuppressionReason);
    report.summary.findings = report.findings.len();
}

fn kind_count(findings: &[Finding], kind: FindingKind) -> usize {
    findings
        .iter()
        .filter(|finding| finding.kind == kind)
        .count()
}

fn dependency_count(findings: &[Finding]) -> usize {
    findings
        .iter()
        .filter(|finding| is_dependency_hygiene_kind(finding.kind))
        .count()
}

fn dependency_override_count(findings: &[Finding]) -> usize {
    kind_count(findings, FindingKind::UnusedDependencyOverride)
        + kind_count(findings, FindingKind::MisconfiguredDependencyOverride)
}

const fn is_dependency_hygiene_kind(kind: FindingKind) -> bool {
    matches!(
        kind,
        FindingKind::UnusedDependency
            | FindingKind::UnusedDevDependency
            | FindingKind::TestOnlyDependency
            | FindingKind::UnusedDependencyOverride
    )
}
