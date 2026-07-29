use serde::{Deserialize, Serialize};

/// Per-file/function complexity threshold override.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthThresholdOverride {
    /// File globs matched against root-relative Dart paths.
    pub files: Vec<String>,
    /// Exact function names. Empty means every function in matching files.
    #[serde(default)]
    pub functions: Vec<String>,
    /// Local cyclomatic ceiling.
    #[serde(default, alias = "maxCyclomatic")]
    pub max_cyclomatic: Option<usize>,
    /// Local cognitive ceiling.
    #[serde(default, alias = "maxCognitive")]
    pub max_cognitive: Option<usize>,
    /// Local CRAP ceiling.
    #[serde(default, alias = "maxCrap")]
    pub max_crap: Option<usize>,
    /// Local inclusive physical-line ceiling.
    #[serde(default, alias = "maxUnitSize")]
    pub max_unit_size: Option<usize>,
    /// Reason shown to agents when this override is active.
    #[serde(default)]
    pub reason: Option<String>,
}

impl HealthThresholdOverride {
    /// Whether this override contains at least one local ceiling.
    #[must_use]
    pub const fn has_threshold(&self) -> bool {
        self.max_cyclomatic.is_some()
            || self.max_cognitive.is_some()
            || self.max_crap.is_some()
            || self.max_unit_size.is_some()
    }

    pub(super) const fn has_static_threshold(&self) -> bool {
        self.max_cyclomatic.is_some() || self.max_cognitive.is_some()
    }

    pub(super) const fn has_crap_threshold(&self) -> bool {
        self.max_crap.is_some()
    }

    pub(super) const fn has_unit_size_threshold(&self) -> bool {
        self.max_unit_size.is_some()
    }
}

/// Status for one configured threshold override.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthThresholdOverrideStatus {
    /// Override matched a function and changed or explained threshold output.
    Active,
    /// Override matched code, but no current function needs its local ceiling.
    Stale,
    /// Override did not match any analyzed function.
    NoMatch,
}

/// Runtime report state for one threshold override.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthThresholdOverrideReport {
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
    /// Override status.
    pub status: HealthThresholdOverrideStatus,
    /// Matched root-relative `file:symbol` entries.
    pub matched_functions: Vec<String>,
}

/// Effective thresholds used for a finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectiveThresholds {
    /// Cyclomatic ceiling used for this function.
    pub max_cyclomatic: Option<usize>,
    /// Cognitive ceiling used for this function.
    pub max_cognitive: Option<usize>,
    /// CRAP ceiling used for this function.
    pub max_crap: Option<usize>,
    /// Inclusive physical-line ceiling used for this function.
    pub max_unit_size: Option<usize>,
}

/// Source of the effective thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThresholdSource {
    /// Threshold came from `health.thresholdOverrides`.
    Override,
}

/// Thresholds applied to a function while evaluating findings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AppliedThresholds {
    pub(super) source: Option<ThresholdSource>,
    pub(super) reason: Option<String>,
    pub(super) effective: EffectiveThresholds,
}

impl AppliedThresholds {
    pub(super) fn default_static(max_cyclomatic: usize, max_cognitive: usize) -> Self {
        Self {
            source: None,
            reason: None,
            effective: EffectiveThresholds {
                max_cyclomatic: Some(max_cyclomatic),
                max_cognitive: Some(max_cognitive),
                max_crap: None,
                max_unit_size: None,
            },
        }
    }

    pub(super) fn default_crap(max_crap: usize) -> Self {
        Self {
            source: None,
            reason: None,
            effective: EffectiveThresholds {
                max_cyclomatic: None,
                max_cognitive: None,
                max_crap: Some(max_crap),
                max_unit_size: None,
            },
        }
    }

    pub(super) fn default_unit_size(max_unit_size: usize) -> Self {
        Self {
            source: None,
            reason: None,
            effective: EffectiveThresholds {
                max_cyclomatic: None,
                max_cognitive: None,
                max_crap: None,
                max_unit_size: Some(max_unit_size),
            },
        }
    }
}

pub(super) fn override_report(
    index: usize,
    rule: &HealthThresholdOverride,
    status: HealthThresholdOverrideStatus,
    matched_functions: Vec<String>,
) -> HealthThresholdOverrideReport {
    HealthThresholdOverrideReport {
        index,
        files: rule.files.clone(),
        functions: rule.functions.clone(),
        max_cyclomatic: rule.max_cyclomatic,
        max_cognitive: rule.max_cognitive,
        max_crap: rule.max_crap,
        max_unit_size: rule.max_unit_size,
        reason: rule.reason.clone(),
        status,
        matched_functions,
    }
}

pub(super) fn function_label(
    root: &std::path::Path,
    path: &std::path::Path,
    symbol: &str,
) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    format!("{}:{symbol}", path_to_slash_string(relative))
}

fn path_to_slash_string(path: &std::path::Path) -> String {
    path.components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/")
}
