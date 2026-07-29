use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use glob::Pattern;

use super::FunctionMetrics;
use super::threshold_types::{
    AppliedThresholds, EffectiveThresholds, HealthThresholdOverride, HealthThresholdOverrideReport,
    HealthThresholdOverrideStatus, ThresholdSource, function_label, override_report,
};
use super::types::HealthOptions;

#[derive(Debug, Clone)]
pub(super) struct ThresholdContext {
    root: PathBuf,
    max_cyclomatic: usize,
    max_cognitive: usize,
    max_crap: Option<usize>,
    max_unit_size: usize,
    overrides: Vec<CompiledOverride>,
    states: Vec<OverrideState>,
}

impl ThresholdContext {
    pub(super) fn new(root: &Path, options: &HealthOptions) -> Self {
        let overrides = options
            .threshold_overrides
            .iter()
            .map(CompiledOverride::new)
            .collect::<Vec<_>>();
        Self {
            root: root.to_path_buf(),
            max_cyclomatic: options.max_cyclomatic,
            max_cognitive: options.max_cognitive,
            max_crap: options.max_crap,
            max_unit_size: options.max_unit_size,
            states: vec![OverrideState::default(); overrides.len()],
            overrides,
        }
    }

    pub(super) fn static_thresholds(&mut self, function: &FunctionMetrics) -> AppliedThresholds {
        let global = AppliedThresholds::default_static(self.max_cyclomatic, self.max_cognitive);
        let mut applied = None;

        for index in self.static_matches(function) {
            let rule = &self.overrides[index].rule;
            let max_cyclomatic = rule.max_cyclomatic.unwrap_or(self.max_cyclomatic);
            let max_cognitive = rule.max_cognitive.unwrap_or(self.max_cognitive);
            let reason = rule.reason.clone();
            self.mark_matched(index, function);
            let thresholds = AppliedThresholds {
                source: Some(ThresholdSource::Override),
                reason,
                effective: EffectiveThresholds {
                    max_cyclomatic: Some(max_cyclomatic),
                    max_cognitive: Some(max_cognitive),
                    max_crap: None,
                    max_unit_size: None,
                },
            };
            if self.static_active(function, &thresholds) {
                self.states[index].active = true;
            }
            if applied.is_none() {
                applied = Some(thresholds);
            }
        }

        applied.unwrap_or(global)
    }

    pub(super) fn crap_thresholds(
        &mut self,
        function: &FunctionMetrics,
        crap_score: usize,
    ) -> Option<AppliedThresholds> {
        let mut applied = self.max_crap.map(AppliedThresholds::default_crap);

        for index in self.crap_matches(function) {
            let rule = &self.overrides[index].rule;
            let max_crap = rule.max_crap;
            let reason = rule.reason.clone();
            self.mark_matched(index, function);
            let Some(max_crap) = max_crap else {
                continue;
            };
            let thresholds = AppliedThresholds {
                source: Some(ThresholdSource::Override),
                reason,
                effective: EffectiveThresholds {
                    max_cyclomatic: None,
                    max_cognitive: None,
                    max_crap: Some(max_crap),
                    max_unit_size: None,
                },
            };
            if self.crap_active(crap_score, max_crap) {
                self.states[index].active = true;
            }
            if applied
                .as_ref()
                .is_none_or(|current| current.source.is_none())
            {
                applied = Some(thresholds);
            }
        }

        applied
    }

    pub(super) fn unit_size_thresholds(
        &mut self,
        function: &FunctionMetrics,
        line_count: usize,
    ) -> AppliedThresholds {
        let global = AppliedThresholds::default_unit_size(self.max_unit_size);
        let mut applied = None;

        for index in self.unit_size_matches(function) {
            let rule = &self.overrides[index].rule;
            let max_unit_size = rule.max_unit_size.unwrap_or(self.max_unit_size);
            let reason = rule.reason.clone();
            self.mark_matched(index, function);
            let thresholds = AppliedThresholds {
                source: Some(ThresholdSource::Override),
                reason,
                effective: EffectiveThresholds {
                    max_cyclomatic: None,
                    max_cognitive: None,
                    max_crap: None,
                    max_unit_size: Some(max_unit_size),
                },
            };
            if line_count > self.max_unit_size || line_count > max_unit_size {
                self.states[index].active = true;
            }
            if applied.is_none() {
                applied = Some(thresholds);
            }
        }

        applied.unwrap_or(global)
    }

    pub(super) fn reports(&self) -> Vec<HealthThresholdOverrideReport> {
        self.overrides
            .iter()
            .zip(&self.states)
            .enumerate()
            .map(|(index, (compiled, state))| {
                override_report(
                    index,
                    &compiled.rule,
                    state.status(),
                    state.matched_functions.iter().cloned().collect(),
                )
            })
            .collect()
    }

    fn static_matches(&self, function: &FunctionMetrics) -> Vec<usize> {
        self.overrides
            .iter()
            .enumerate()
            .filter(|(_, compiled)| {
                compiled.rule.has_static_threshold() && compiled.matches(&self.root, function)
            })
            .map(|(index, _)| index)
            .collect()
    }

    fn crap_matches(&self, function: &FunctionMetrics) -> Vec<usize> {
        self.overrides
            .iter()
            .enumerate()
            .filter(|(_, compiled)| {
                compiled.rule.has_crap_threshold() && compiled.matches(&self.root, function)
            })
            .map(|(index, _)| index)
            .collect()
    }

    fn unit_size_matches(&self, function: &FunctionMetrics) -> Vec<usize> {
        self.overrides
            .iter()
            .enumerate()
            .filter(|(_, compiled)| {
                compiled.rule.has_unit_size_threshold() && compiled.matches(&self.root, function)
            })
            .map(|(index, _)| index)
            .collect()
    }

    fn static_active(&self, function: &FunctionMetrics, thresholds: &AppliedThresholds) -> bool {
        let max_cyclomatic = thresholds
            .effective
            .max_cyclomatic
            .unwrap_or(self.max_cyclomatic);
        let max_cognitive = thresholds
            .effective
            .max_cognitive
            .unwrap_or(self.max_cognitive);
        function.cyclomatic > self.max_cyclomatic
            || function.cognitive > self.max_cognitive
            || function.cyclomatic > max_cyclomatic
            || function.cognitive > max_cognitive
    }

    fn crap_active(&self, crap_score: usize, max_crap: usize) -> bool {
        self.max_crap.is_some_and(|global| crap_score > global) || crap_score > max_crap
    }

    fn mark_matched(&mut self, index: usize, function: &FunctionMetrics) {
        self.states[index].matched_functions.insert(function_label(
            &self.root,
            &function.path,
            &function.symbol,
        ));
    }
}

#[derive(Debug, Clone)]
struct CompiledOverride {
    rule: HealthThresholdOverride,
    file_patterns: Vec<Pattern>,
}

impl CompiledOverride {
    fn new(rule: &HealthThresholdOverride) -> Self {
        Self {
            file_patterns: rule
                .files
                .iter()
                .filter_map(|pattern| Pattern::new(pattern).ok())
                .collect(),
            rule: rule.clone(),
        }
    }

    fn matches(&self, root: &Path, function: &FunctionMetrics) -> bool {
        let relative = function
            .path
            .strip_prefix(root)
            .unwrap_or(&function.path)
            .components()
            .filter_map(|component| component.as_os_str().to_str())
            .collect::<Vec<_>>()
            .join("/");
        self.file_patterns
            .iter()
            .any(|pattern| pattern.matches(&relative))
            && (self.rule.functions.is_empty()
                || self
                    .rule
                    .functions
                    .iter()
                    .any(|name| name == &function.symbol))
    }
}

#[derive(Debug, Clone, Default)]
struct OverrideState {
    active: bool,
    matched_functions: BTreeSet<String>,
}

impl OverrideState {
    fn status(&self) -> HealthThresholdOverrideStatus {
        if self.matched_functions.is_empty() {
            HealthThresholdOverrideStatus::NoMatch
        } else if self.active {
            HealthThresholdOverrideStatus::Active
        } else {
            HealthThresholdOverrideStatus::Stale
        }
    }
}
