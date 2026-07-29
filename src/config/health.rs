use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{HealthOptions, HealthThresholdOverride, LowTrafficThreshold};

use super::ConfigError;

/// Health analyzer config defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HealthConfig {
    /// Maximum cyclomatic complexity before reporting.
    #[serde(default, alias = "maxCyclomatic")]
    pub max_cyclomatic: Option<usize>,
    /// Maximum cognitive complexity before reporting.
    #[serde(default, alias = "maxCognitive")]
    pub max_cognitive: Option<usize>,
    /// Maximum inclusive physical lines in one function-like declaration.
    #[serde(default, alias = "maxUnitSize")]
    pub max_unit_size: Option<usize>,
    /// Limit output to the N highest complexity findings.
    #[serde(default)]
    pub top: Option<usize>,
    /// Include per-decision-point complexity contributions.
    #[serde(default, alias = "complexityBreakdown")]
    pub complexity_breakdown: Option<bool>,
    /// LCOV file for coverage-aware health checks.
    #[serde(default, alias = "coveragePath")]
    pub coverage_path: Option<PathBuf>,
    /// Report Dart files with no covered executable lines.
    #[serde(default, alias = "coverageGaps")]
    pub coverage_gaps: Option<bool>,
    /// Maximum CRAP score before reporting.
    #[serde(default, alias = "maxCrap")]
    pub max_crap: Option<usize>,
    /// Runtime coverage JSON file or directory.
    #[serde(default, alias = "runtimeCoverage")]
    pub runtime_coverage: Option<PathBuf>,
    /// Minimum runtime invocations before a file is a hot path.
    #[serde(default, alias = "minInvocationsHot")]
    pub min_invocations_hot: Option<usize>,
    /// Minimum runtime observations for high-confidence signals.
    #[serde(default, alias = "minObservationVolume")]
    pub min_observation_volume: Option<usize>,
    /// Fraction of runtime traffic considered low traffic.
    #[serde(default, alias = "lowTrafficThreshold")]
    pub low_traffic_threshold: Option<LowTrafficThreshold>,
    /// Include per-file health scores.
    #[serde(default, alias = "fileScores")]
    pub file_scores: Option<bool>,
    /// Report low-scoring complexity hotspots.
    #[serde(default)]
    pub hotspots: Option<bool>,
    /// Report prioritized refactoring targets.
    #[serde(default)]
    pub targets: Option<bool>,
    /// Attach CODEOWNERS ownership metadata to health output.
    #[serde(default)]
    pub ownership: Option<bool>,
    /// Include advisory Flutter theme/style analysis.
    #[serde(default, alias = "flutterStyle")]
    pub flutter_style: Option<bool>,
    /// Minimum file health score before hotspot reporting.
    #[serde(default, alias = "minScore")]
    pub min_score: Option<usize>,
    /// Per-file/function local complexity ceilings.
    #[serde(default, alias = "thresholdOverrides")]
    pub threshold_overrides: Vec<HealthThresholdOverride>,
}

impl HealthConfig {
    pub(super) fn validate(&self) -> Result<(), ConfigError> {
        for (index, rule) in self.threshold_overrides.iter().enumerate() {
            if rule.files.is_empty() {
                return Err(ConfigError::HealthThresholdOverride {
                    index,
                    message: "files must contain at least one glob".to_owned(),
                });
            }
            if !rule.has_threshold() {
                return Err(ConfigError::HealthThresholdOverride {
                    index,
                    message: "set maxCyclomatic, maxCognitive, maxCrap, or maxUnitSize".to_owned(),
                });
            }
            for pattern in &rule.files {
                if let Err(error) = glob::Pattern::new(pattern) {
                    return Err(ConfigError::HealthThresholdOverride {
                        index,
                        message: format!("invalid files glob {pattern:?}: {error}"),
                    });
                }
            }
        }
        Ok(())
    }

    pub(super) fn apply_to(&self, options: &mut HealthOptions) {
        if let Some(max_cyclomatic) = self.max_cyclomatic {
            options.max_cyclomatic = max_cyclomatic;
        }
        if let Some(max_cognitive) = self.max_cognitive {
            options.max_cognitive = max_cognitive;
        }
        if let Some(max_unit_size) = self.max_unit_size {
            options.max_unit_size = max_unit_size;
        }
        if self.top.is_some() {
            options.top = self.top;
        }
        if let Some(complexity_breakdown) = self.complexity_breakdown {
            options.complexity_breakdown = complexity_breakdown.into();
        }
        if self.coverage_path.is_some() {
            options.coverage_path.clone_from(&self.coverage_path);
        }
        if let Some(coverage_gaps) = self.coverage_gaps {
            options.coverage_gaps = coverage_gaps.into();
        }
        if self.max_crap.is_some() {
            options.max_crap = self.max_crap;
        }
        if self.runtime_coverage.is_some() {
            options
                .runtime_coverage_path
                .clone_from(&self.runtime_coverage);
        }
        if let Some(min_invocations_hot) = self.min_invocations_hot {
            options.min_invocations_hot = min_invocations_hot.max(1);
        }
        if let Some(min_observation_volume) = self.min_observation_volume {
            options.min_observation_volume = min_observation_volume.max(1);
        }
        if let Some(low_traffic_threshold) = self.low_traffic_threshold {
            options.low_traffic_threshold = low_traffic_threshold;
        }
        if let Some(file_scores) = self.file_scores {
            options.file_scores = file_scores.into();
        }
        if let Some(hotspots) = self.hotspots {
            options.hotspots = hotspots.into();
        }
        if let Some(targets) = self.targets {
            options.targets = targets.into();
        }
        if let Some(ownership) = self.ownership {
            options.ownership = ownership.into();
        }
        if let Some(flutter_style) = self.flutter_style {
            options.flutter_style = flutter_style.into();
        }
        if let Some(min_score) = self.min_score {
            options.min_score = min_score.min(100);
        }
        options
            .threshold_overrides
            .clone_from(&self.threshold_overrides);
    }
}

pub(super) fn health_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "max_cyclomatic": positive_integer_schema(),
            "maxCyclomatic": positive_integer_schema(),
            "max_cognitive": positive_integer_schema(),
            "maxCognitive": positive_integer_schema(),
            "max_unit_size": positive_integer_schema(),
            "maxUnitSize": positive_integer_schema(),
            "top": positive_integer_schema(),
            "complexity_breakdown": { "type": "boolean" },
            "complexityBreakdown": { "type": "boolean" },
            "coverage_path": { "type": "string" },
            "coveragePath": { "type": "string" },
            "coverage_gaps": { "type": "boolean" },
            "coverageGaps": { "type": "boolean" },
            "max_crap": positive_integer_schema(),
            "maxCrap": positive_integer_schema(),
            "runtime_coverage": { "type": "string" },
            "runtimeCoverage": { "type": "string" },
            "min_invocations_hot": positive_integer_schema(),
            "minInvocationsHot": positive_integer_schema(),
            "min_observation_volume": positive_integer_schema(),
            "minObservationVolume": positive_integer_schema(),
            "low_traffic_threshold": threshold_schema(),
            "lowTrafficThreshold": threshold_schema(),
            "file_scores": { "type": "boolean" },
            "fileScores": { "type": "boolean" },
            "hotspots": { "type": "boolean" },
            "targets": { "type": "boolean" },
            "ownership": { "type": "boolean" },
            "flutter_style": { "type": "boolean" },
            "flutterStyle": { "type": "boolean" },
            "min_score": score_schema(),
            "minScore": score_schema(),
            "threshold_overrides": threshold_overrides_schema(),
            "thresholdOverrides": threshold_overrides_schema()
        }
    })
}

fn positive_integer_schema() -> Value {
    json!({ "type": "integer", "minimum": 1 })
}

fn score_schema() -> Value {
    json!({ "type": "integer", "minimum": 0, "maximum": 100 })
}

fn threshold_schema() -> Value {
    json!({ "type": "number", "minimum": 0, "maximum": 1 })
}

fn threshold_overrides_schema() -> Value {
    json!({
        "type": "array",
        "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["files"],
            "anyOf": [
                { "required": ["max_cyclomatic"] },
                { "required": ["maxCyclomatic"] },
                { "required": ["max_cognitive"] },
                { "required": ["maxCognitive"] },
                { "required": ["max_crap"] },
                { "required": ["maxCrap"] },
                { "required": ["max_unit_size"] },
                { "required": ["maxUnitSize"] }
            ],
            "properties": {
                "files": {
                    "type": "array",
                    "minItems": 1,
                    "items": { "type": "string" }
                },
                "functions": {
                    "type": "array",
                    "items": { "type": "string" }
                },
                "max_cyclomatic": positive_integer_schema(),
                "maxCyclomatic": positive_integer_schema(),
                "max_cognitive": positive_integer_schema(),
                "maxCognitive": positive_integer_schema(),
                "max_crap": positive_integer_schema(),
                "maxCrap": positive_integer_schema(),
                "max_unit_size": positive_integer_schema(),
                "maxUnitSize": positive_integer_schema(),
                "reason": { "type": ["string", "null"] }
            }
        }
    })
}
