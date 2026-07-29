use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::Location;

use super::runtime_intelligence::{
    RuntimeBlastRadius, RuntimeCoverageIntelligence, RuntimeImportance,
};
use super::threshold_types::{
    EffectiveThresholds, HealthThresholdOverride, HealthThresholdOverrideReport, ThresholdSource,
};

/// Complexity detector options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthOptions {
    /// Maximum cyclomatic complexity before reporting.
    pub max_cyclomatic: usize,
    /// Maximum cognitive complexity before reporting.
    pub max_cognitive: usize,
    /// Maximum inclusive physical lines in one function-like declaration.
    pub max_unit_size: usize,
    /// Limit output to the N highest complexity findings.
    pub top: Option<usize>,
    /// Include per-decision-point contribution records.
    pub complexity_breakdown: HealthToggle,
    /// LCOV file used for coverage-aware health checks.
    pub coverage_path: Option<PathBuf>,
    /// Report source files with no covered executable lines.
    pub coverage_gaps: HealthToggle,
    /// Maximum CRAP score before reporting.
    pub max_crap: Option<usize>,
    /// Runtime coverage JSON file or directory.
    pub runtime_coverage_path: Option<PathBuf>,
    /// Minimum runtime invocations before a file is a hot path.
    pub min_invocations_hot: usize,
    /// Minimum total runtime observations for high-confidence cleanup signals.
    pub min_observation_volume: usize,
    /// Fraction of total runtime traffic considered low traffic.
    pub low_traffic_threshold: LowTrafficThreshold,
    /// Include per-file health scores.
    pub file_scores: HealthToggle,
    /// Report low-scoring complexity hotspots.
    pub hotspots: HealthToggle,
    /// Report prioritized refactoring targets.
    pub targets: HealthToggle,
    /// Attach CODEOWNERS ownership metadata to health inventories.
    pub ownership: HealthToggle,
    /// Include advisory Flutter theme/style analysis.
    pub flutter_style: HealthToggle,
    /// Minimum file health score before hotspot reporting.
    pub min_score: usize,
    /// Per-file/function local complexity ceilings.
    pub threshold_overrides: Vec<HealthThresholdOverride>,
}

impl Default for HealthOptions {
    fn default() -> Self {
        Self {
            max_cyclomatic: 20,
            max_cognitive: 15,
            max_unit_size: 60,
            top: None,
            complexity_breakdown: HealthToggle::Off,
            coverage_path: None,
            coverage_gaps: HealthToggle::Off,
            max_crap: None,
            runtime_coverage_path: None,
            min_invocations_hot: 100,
            min_observation_volume: 5_000,
            low_traffic_threshold: LowTrafficThreshold::default(),
            file_scores: HealthToggle::Off,
            hotspots: HealthToggle::Off,
            targets: HealthToggle::Off,
            ownership: HealthToggle::Off,
            flutter_style: HealthToggle::Off,
            min_score: 70,
            threshold_overrides: Vec::new(),
        }
    }
}

/// Fraction of runtime observations considered low traffic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LowTrafficThreshold(u32);

impl LowTrafficThreshold {
    const SCALE: u32 = 1_000_000;

    /// Build a threshold from a ratio in the inclusive range `0.0..=1.0`.
    #[must_use]
    pub fn from_ratio(value: f64) -> Self {
        if !value.is_finite() || value <= 0.0 {
            return Self(0);
        }
        if value >= 1.0 {
            return Self(Self::SCALE);
        }
        let scaled = (value * f64::from(Self::SCALE)).round();
        let micros = format!("{scaled:.0}").parse::<u32>().unwrap_or(Self::SCALE);
        Self(micros)
    }

    /// Return the threshold as a ratio.
    #[must_use]
    pub fn ratio(self) -> f64 {
        f64::from(self.0) / f64::from(Self::SCALE)
    }

    /// Build a threshold from an integer fraction.
    #[must_use]
    pub fn from_fraction(numerator: usize, denominator: usize) -> Self {
        if denominator == 0 || numerator == 0 {
            return Self(0);
        }
        let numerator = u64::try_from(numerator).unwrap_or(u64::MAX);
        let denominator = u64::try_from(denominator).unwrap_or(u64::MAX);
        let scaled =
            u128::from(numerator).saturating_mul(u128::from(Self::SCALE)) / u128::from(denominator);
        Self(u32::try_from(scaled).unwrap_or(Self::SCALE))
    }
}

impl Default for LowTrafficThreshold {
    fn default() -> Self {
        Self::from_ratio(0.001)
    }
}

impl Serialize for LowTrafficThreshold {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_f64(self.ratio())
    }
}

impl<'de> Deserialize<'de> for LowTrafficThreshold {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        f64::deserialize(deserializer).map(Self::from_ratio)
    }
}

/// Boolean health-analysis toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HealthToggle {
    /// Toggle disabled.
    #[default]
    Off,
    /// Toggle enabled.
    On,
}

impl HealthToggle {
    /// Whether this toggle is enabled.
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        matches!(self, Self::On)
    }
}

impl From<bool> for HealthToggle {
    fn from(value: bool) -> Self {
        if value { Self::On } else { Self::Off }
    }
}

impl Serialize for HealthToggle {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bool(self.is_enabled())
    }
}

impl<'de> Deserialize<'de> for HealthToggle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        bool::deserialize(deserializer).map(Self::from)
    }
}

/// Dart code health report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthReport {
    /// Options used to compute this report.
    pub options: HealthOptions,
    /// Aggregate 0-100 project health score; higher is healthier.
    pub quality_score: usize,
    /// Number of source files included in health analysis.
    pub analyzed_files: usize,
    /// Number of function-like declarations scored.
    pub functions: usize,
    /// Highest cyclomatic complexity found.
    pub max_cyclomatic_complexity: usize,
    /// Highest cognitive complexity found.
    pub max_cognitive_complexity: usize,
    /// Number of files represented in the loaded LCOV report.
    pub coverage_files: usize,
    /// Highest CRAP score found from complexity and coverage data.
    pub max_crap_score: usize,
    /// Functions exceeding configured thresholds.
    pub complexity: Vec<ComplexityFinding>,
    /// Function-like declarations exceeding the effective unit-size threshold.
    pub large_functions: Vec<LargeFunction>,
    /// Opt-in advisory Flutter style analysis.
    pub flutter_style: Option<crate::FlutterStyleReport>,
    /// Source files with no covered executable lines.
    pub coverage_gaps: Vec<CoverageGapFinding>,
    /// Functions exceeding configured CRAP threshold.
    pub crap: Vec<CrapFinding>,
    /// Configured threshold override statuses.
    pub threshold_overrides: Vec<HealthThresholdOverrideReport>,
    /// Runtime coverage evidence, when requested.
    pub runtime_coverage: Option<RuntimeCoverageReport>,
    /// Per-file health scores.
    pub file_scores: Vec<FileHealthScore>,
    /// Low-scoring files selected as hotspots.
    pub hotspots: Vec<HealthHotspot>,
    /// Files selected as refactoring targets.
    pub refactoring_targets: Vec<RefactoringTarget>,
}

/// Runtime coverage report derived from local V8 or Istanbul coverage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCoverageReport {
    /// Runtime coverage input path.
    pub source_path: PathBuf,
    /// Runtime coverage source format.
    pub source_format: RuntimeCoverageFormat,
    /// Stable hash of the coverage input payload.
    pub source_hash: String,
    /// Files observed in runtime coverage.
    pub observed_files: usize,
    /// Total observed runtime invocations.
    pub total_invocations: usize,
    /// Hot path threshold used for this run.
    pub min_invocations_hot: usize,
    /// Observation volume threshold used for this run.
    pub min_observation_volume: usize,
    /// Low-traffic threshold used for this run.
    pub low_traffic_threshold: LowTrafficThreshold,
    /// Files meeting the hot path threshold.
    pub hot_paths: Vec<RuntimeHotPath>,
    /// Runtime-derived cleanup/review signals.
    pub findings: Vec<RuntimeCoverageFinding>,
    /// Static/runtime recommendations for agent review.
    pub coverage_intelligence: Vec<RuntimeCoverageIntelligence>,
    /// Caller blast-radius rows for hot runtime paths.
    pub blast_radius: Vec<RuntimeBlastRadius>,
    /// Runtime-weighted production importance rows.
    pub importance: Vec<RuntimeImportance>,
    /// Non-fatal parsing or mapping warnings.
    pub warnings: Vec<String>,
}

/// Runtime coverage source format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeCoverageFormat {
    /// Istanbul `coverage-final.json` coverage map.
    Istanbul,
    /// V8 coverage JSON file or directory.
    V8,
    /// Mixed coverage directory containing more than one supported format.
    Mixed,
}

/// Runtime hot path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeHotPath {
    /// Dart file path.
    pub path: PathBuf,
    /// 1-based line when the coverage input exposes one.
    pub line: Option<usize>,
    /// Optional function name from runtime coverage.
    pub symbol: Option<String>,
    /// Observed runtime invocations.
    pub invocations: usize,
    /// Source map confidence.
    pub source_map_confidence: SourceMapConfidence,
}

/// Runtime cleanup or review signal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCoverageFinding {
    /// Dart file path.
    pub path: PathBuf,
    /// Runtime coverage finding kind.
    pub kind: RuntimeCoverageFindingKind,
    /// 1-based line.
    pub line: usize,
    /// Observed runtime invocations.
    pub invocations: usize,
    /// Fraction of all runtime observations.
    pub traffic_fraction: LowTrafficThreshold,
    /// Whether graph plus runtime evidence supports deletion.
    pub safe_to_delete: bool,
    /// Whether a human or agent should review before changing.
    pub review_required: bool,
    /// Runtime confidence for the signal.
    pub confidence: RuntimeCoverageConfidence,
    /// Agent-readable reason.
    pub reason: String,
}

/// Runtime coverage signal kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeCoverageFindingKind {
    /// File had runtime coverage below the low-traffic threshold.
    LowTraffic,
    /// File was part of the scanned project but absent from runtime coverage.
    CoverageUnavailable,
}

/// Runtime coverage confidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeCoverageConfidence {
    /// Enough runtime observations to trust aggregate signals.
    High,
    /// Runtime evidence exists but volume is below the high-confidence threshold.
    Medium,
    /// Runtime evidence was absent or unmapped.
    Low,
}

/// Confidence that a runtime coverage entry maps to the reported source path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceMapConfidence {
    /// Path resolved to a local Dart source file.
    Resolved,
    /// Path was normalized but not found under the scanned project.
    Fallback,
    /// Path could not be mapped.
    Unresolved,
}

/// One function-like Dart declaration exceeding health thresholds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplexityFinding {
    /// Dart file path.
    pub path: PathBuf,
    /// Function, method, getter, setter, constructor, or anonymous closure name.
    pub symbol: String,
    /// Function-like declaration kind.
    pub kind: ComplexityFunctionKind,
    /// Location of the function-like declaration.
    pub location: Location,
    /// Cyclomatic complexity score.
    pub cyclomatic_complexity: usize,
    /// Cognitive complexity score.
    pub cognitive_complexity: usize,
    /// Rule represented by this finding.
    pub rule: ComplexityRule,
    /// Effective thresholds used when a local override matched.
    pub effective_thresholds: Option<EffectiveThresholds>,
    /// Source of the effective thresholds when not global defaults.
    pub threshold_source: Option<ThresholdSource>,
    /// Configured reason for the threshold override.
    pub threshold_reason: Option<String>,
    /// Optional decision-point breakdown.
    pub contributions: Vec<ComplexityContribution>,
}

/// One Dart file with no observed covered executable lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageGapFinding {
    /// Dart file path.
    pub path: PathBuf,
    /// Location used for the file-level finding.
    pub location: Location,
    /// Why the file is considered a coverage gap.
    pub reason: CoverageGapReason,
    /// Covered executable lines in LCOV.
    pub covered_lines: usize,
    /// Executable lines in LCOV.
    pub executable_lines: usize,
}

/// Coverage-gap reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoverageGapReason {
    /// LCOV had no record for the Dart file.
    MissingFromCoverage,
    /// LCOV had a file record but no executable lines.
    NoExecutableLines,
    /// LCOV had executable lines, none of which were covered.
    ZeroCoveredLines,
}

/// One function-like declaration exceeding the configured CRAP threshold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrapFinding {
    /// Dart file path.
    pub path: PathBuf,
    /// Function, method, getter, setter, constructor, or closure name.
    pub symbol: String,
    /// Function-like declaration kind.
    pub kind: ComplexityFunctionKind,
    /// Location of the function-like declaration.
    pub location: Location,
    /// Cyclomatic complexity score.
    pub cyclomatic_complexity: usize,
    /// Cognitive complexity score.
    pub cognitive_complexity: usize,
    /// Covered executable lines in the function range.
    pub covered_lines: usize,
    /// Executable lines in the function range.
    pub executable_lines: usize,
    /// Rounded line coverage percentage for the function range.
    pub line_coverage_percent: usize,
    /// CRAP score, rounded up for stable thresholds.
    pub crap_score: usize,
    /// Effective thresholds used when a local override matched.
    pub effective_thresholds: Option<EffectiveThresholds>,
    /// Source of the effective thresholds when not global defaults.
    pub threshold_source: Option<ThresholdSource>,
    /// Configured reason for the threshold override.
    pub threshold_reason: Option<String>,
}

/// One file-level health score.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileHealthScore {
    /// Dart file path.
    pub path: PathBuf,
    /// 0-100 health score; higher is healthier.
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
    /// Coverage status for the file.
    pub coverage_status: FileCoverageStatus,
    /// Covered executable lines when LCOV has data.
    pub covered_lines: Option<usize>,
    /// Executable lines when LCOV has data.
    pub executable_lines: Option<usize>,
    /// Rounded line coverage percentage when LCOV has executable lines.
    pub line_coverage_percent: Option<usize>,
    /// Agent-readable reasons contributing to the score.
    pub reasons: Vec<String>,
    /// Owners matched from CODEOWNERS, when requested.
    pub owners: Vec<String>,
    /// CODEOWNERS file used for this owner match.
    pub owner_source: Option<String>,
    /// GitLab CODEOWNERS section, when present.
    pub owner_section: Option<String>,
}

/// File-level coverage status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileCoverageStatus {
    /// Coverage was not requested for this run.
    NotRequested,
    /// Coverage was requested but this file had no LCOV record.
    Missing,
    /// LCOV had no executable lines for this file.
    NoExecutableLines,
    /// LCOV had executable lines but none were covered.
    Uncovered,
    /// LCOV had at least one covered executable line.
    Covered,
}

/// Low-scoring file-level hotspot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthHotspot {
    /// Dart file path.
    pub path: PathBuf,
    /// Location used for the file-level finding.
    pub location: Location,
    /// 0-100 health score.
    pub score: usize,
    /// Reasons contributing to hotspot selection.
    pub reasons: Vec<String>,
    /// Owners matched from CODEOWNERS, when requested.
    pub owners: Vec<String>,
    /// CODEOWNERS file used for this owner match.
    pub owner_source: Option<String>,
    /// GitLab CODEOWNERS section, when present.
    pub owner_section: Option<String>,
}

/// Ranked refactoring target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefactoringTarget {
    /// Dart file path.
    pub path: PathBuf,
    /// Location used for the file-level finding.
    pub location: Location,
    /// 0-100 health score.
    pub score: usize,
    /// Priority score used for deterministic target ordering.
    pub priority: usize,
    /// Reasons contributing to target selection.
    pub reasons: Vec<String>,
    /// Owners matched from CODEOWNERS, when requested.
    pub owners: Vec<String>,
    /// CODEOWNERS file used for this owner match.
    pub owner_source: Option<String>,
    /// GitLab CODEOWNERS section, when present.
    pub owner_section: Option<String>,
}

/// Function-like declaration kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexityFunctionKind {
    /// Top-level function.
    Function,
    /// Getter declaration.
    Getter,
    /// Setter declaration.
    Setter,
    /// Class, mixin, extension, or enum method.
    Method,
    /// `build` method owned by a syntactically identified Flutter widget or `State`.
    FlutterBuildMethod,
    /// Constructor or factory constructor.
    Constructor,
    /// Operator overload.
    Operator,
    /// Anonymous function expression.
    Closure,
}

/// Advisory record for one function-like declaration above its unit-size threshold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LargeFunction {
    /// Dart file path.
    pub path: PathBuf,
    /// Function-like declaration name.
    pub symbol: String,
    /// Function-like declaration kind.
    pub kind: ComplexityFunctionKind,
    /// Location of the declaration.
    pub location: Location,
    /// Inclusive 1-based ending line.
    pub end_line: usize,
    /// Inclusive physical line count.
    pub line_count: usize,
    /// Effective unit-size ceiling.
    pub max_unit_size: usize,
    /// Source of the effective threshold when overridden.
    pub threshold_source: Option<ThresholdSource>,
    /// Configured reason for the threshold override.
    pub threshold_reason: Option<String>,
}

/// Complexity threshold rule exceeded by a function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComplexityRule {
    /// Cyclomatic threshold only.
    HighCyclomaticComplexity,
    /// Cognitive threshold only.
    HighCognitiveComplexity,
    /// Both cyclomatic and cognitive thresholds.
    HighComplexity,
}

impl ComplexityRule {
    /// Rule identifier used by JSON findings.
    #[must_use]
    pub const fn rule_id(self) -> &'static str {
        match self {
            Self::HighCyclomaticComplexity => "dart-decimate/high-cyclomatic-complexity",
            Self::HighCognitiveComplexity => "dart-decimate/high-cognitive-complexity",
            Self::HighComplexity => "dart-decimate/high-complexity",
        }
    }
}

/// One decision point contributing to complexity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComplexityContribution {
    /// Source location of the decision point.
    pub location: Location,
    /// Decision-point kind.
    pub kind: String,
    /// Cyclomatic score added by this point.
    pub cyclomatic: usize,
    /// Cognitive score added by this point.
    pub cognitive: usize,
    /// Nesting depth at the decision point.
    pub nesting: usize,
}

/// Errors returned while computing health metrics.
#[derive(Debug, Error)]
pub enum HealthError {
    /// A Dart file could not be read.
    #[error("failed to read Dart file {path}: {source}")]
    ReadFile {
        /// File path.
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
    /// Coverage-aware health was requested without an LCOV file.
    #[error(
        "coverage data not found under {root}; pass --coverage PATH or create coverage/lcov.info"
    )]
    MissingCoverageData {
        /// Project root.
        root: PathBuf,
    },
    /// LCOV file could not be read.
    #[error("failed to read coverage file {path}: {source}")]
    ReadCoverage {
        /// LCOV path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// LCOV file could not be parsed.
    #[error("failed to parse coverage file {path} at line {line}: {message}")]
    ParseCoverage {
        /// LCOV path.
        path: PathBuf,
        /// 1-based line number.
        line: usize,
        /// Parse failure details.
        message: String,
    },
    /// Runtime coverage file could not be read.
    #[error("failed to read runtime coverage {path}: {source}")]
    ReadRuntimeCoverage {
        /// Runtime coverage path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Runtime coverage JSON could not be parsed.
    #[error("failed to parse runtime coverage {path}: {message}")]
    ParseRuntimeCoverage {
        /// Runtime coverage path.
        path: PathBuf,
        /// Parse failure details.
        message: String,
    },
}
