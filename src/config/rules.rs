use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::SecurityCategory;
use crate::output::{Finding, FindingKind, JsonAttackSurfaceEntry, JsonReport, Severity, Verdict};

use super::rule_aliases::{
    aliases, known_rule, missing_suppression_reason_aliases, private_type_leak_aliases,
};

/// Per-rule configured severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuleLevel {
    Error,
    Warn,
    /// Remove the finding from reports and gates.
    Off,
}

/// Rule severity configuration keyed by rule id or alias.
pub type RuleConfig = BTreeMap<String, RuleLevel>;

/// Validate all configured rule names.
///
/// # Errors
///
/// Returns [`RuleError`] when a rule key is not supported by Dart Decimate.
pub fn validate_rules(rules: &RuleConfig) -> Result<(), RuleError> {
    let _ = RuleMatcher::new(rules)?;
    Ok(())
}

/// Whether the opt-in private-type-leak rule was explicitly enabled.
#[must_use]
pub fn private_type_leaks_enabled(rules: &RuleConfig) -> bool {
    private_type_leak_aliases().into_iter().any(|alias| {
        rules
            .get(alias)
            .is_some_and(|level| *level != RuleLevel::Off)
    })
}

/// Whether suppression comments must include a reason.
#[must_use]
pub fn missing_suppression_reasons_enabled(rules: &RuleConfig) -> bool {
    missing_suppression_reason_aliases()
        .into_iter()
        .any(|alias| {
            rules
                .get(alias)
                .is_some_and(|level| *level != RuleLevel::Off)
        })
}

/// Apply configured rule severities to an already-built report.
///
/// # Errors
///
/// Returns [`RuleError`] when a rule key is not supported by Dart Decimate.
pub fn apply_rules_to_report(report: &mut JsonReport, rules: &RuleConfig) -> Result<(), RuleError> {
    let rules = RuleMatcher::new(rules)?;
    if rules.is_empty() {
        report
            .findings
            .retain(|finding| default_rule_level(finding.kind) != RuleLevel::Off);
        recompute_summary(report);
        return Ok(());
    }

    report
        .findings
        .retain_mut(|finding| apply_finding_level(finding, &rules) != RuleLevel::Off);
    report.clone_groups.retain(|_| {
        rules.level(
            "dart-decimate/code-duplication",
            FindingKind::CodeDuplication,
        ) != RuleLevel::Off
    });
    report.complexity.retain(|finding| {
        rules.level(&finding.rule_id, complexity_kind(&finding.rule_id)) != RuleLevel::Off
    });
    report.hotspots.retain(|_| {
        rules.level("dart-decimate/health-hotspot", FindingKind::HealthHotspot) != RuleLevel::Off
    });
    report.refactoring_targets.retain(|_| {
        rules.level(
            "dart-decimate/refactoring-target",
            FindingKind::RefactoringTarget,
        ) != RuleLevel::Off
    });
    report.feature_flags.retain(|_| {
        rules.level("dart-decimate/feature-flag", FindingKind::FeatureFlag) != RuleLevel::Off
    });
    report.security_candidates.retain(|candidate| {
        rules.level(&candidate.rule_id, FindingKind::SecurityCandidate) != RuleLevel::Off
    });
    report
        .attack_surface
        .retain(|entry| security_surface_enabled(entry, &rules));

    recompute_summary(report);
    let has_hidden_security_surface = report.has_hidden_security_candidate_occurrences();
    report
        .next_steps
        .retain(|step| next_step_enabled(&step.id, &report.findings, has_hidden_security_surface));
    report.verdict = if report
        .findings
        .iter()
        .any(|finding| finding.severity == Severity::Error)
    {
        Verdict::Fail
    } else {
        Verdict::Pass
    };
    Ok(())
}

/// Error returned for invalid rule names.
#[derive(Debug, thiserror::Error)]
pub enum RuleError {
    /// A configured rule key is not supported.
    #[error("unknown config rule {rule:?}")]
    UnknownRule {
        /// Unknown rule key.
        rule: String,
    },
}

fn apply_finding_level(finding: &mut Finding, rules: &RuleMatcher) -> RuleLevel {
    let level = rules.level(&finding.rule_id, finding.kind);
    finding.severity = match level {
        RuleLevel::Error => Severity::Error,
        RuleLevel::Warn => Severity::Warning,
        RuleLevel::Off => finding.severity,
    };
    level
}

fn recompute_summary(report: &mut JsonReport) {
    let summary = &mut report.summary;
    let previous_feature_flags = summary.feature_flags;
    let previous_feature_flag_occurrences = summary.feature_flag_occurrences;
    let previous_security_candidates = summary.security_candidates;
    let previous_security_candidate_occurrences = summary.security_candidate_occurrences;
    summary.unresolved_dependencies =
        kind_count(&report.findings, FindingKind::UnresolvedDependency);
    summary.part_of_violations = kind_count(&report.findings, FindingKind::PartOfViolation);
    summary.unused_dependencies = dependency_count(&report.findings);
    summary.unused_dev_dependencies =
        kind_count(&report.findings, FindingKind::UnusedDevDependency);
    summary.test_only_dependencies = kind_count(&report.findings, FindingKind::TestOnlyDependency);
    summary.dependency_overrides = dependency_override_count(&report.findings);
    summary.unused_dependency_overrides =
        kind_count(&report.findings, FindingKind::UnusedDependencyOverride);
    summary.misconfigured_dependency_overrides = kind_count(
        &report.findings,
        FindingKind::MisconfiguredDependencyOverride,
    );
    summary.unlisted_dependencies = kind_count(&report.findings, FindingKind::UnlistedDependency);
    summary.private_src_imports = kind_count(&report.findings, FindingKind::PrivateSrcImport);
    summary.dead_files = kind_count(&report.findings, FindingKind::DeadFile);
    summary.unused_exports = kind_count(&report.findings, FindingKind::UnusedExport);
    summary.unused_types = kind_count(&report.findings, FindingKind::UnusedType);
    summary.private_type_leaks = kind_count(&report.findings, FindingKind::PrivateTypeLeak);
    summary.unused_enum_members = kind_count(&report.findings, FindingKind::UnusedEnumMember);
    summary.unused_class_members = kind_count(&report.findings, FindingKind::UnusedClassMember);
    summary.duplicate_exports = kind_count(&report.findings, FindingKind::DuplicateExport);
    summary.route_collisions = kind_count(&report.findings, FindingKind::RouteCollision);
    summary.private_widget_classes = kind_count(&report.findings, FindingKind::PrivateWidgetClass);
    summary.widget_top_level_functions = kind_count(
        &report.findings,
        FindingKind::WidgetTopLevelFunctionBoundary,
    );
    summary.unused_widget_params = kind_count(&report.findings, FindingKind::UnusedWidgetParam);
    summary.unrendered_widgets = kind_count(&report.findings, FindingKind::UnrenderedWidget);
    summary.missing_context_mounted_after_await = kind_count(
        &report.findings,
        FindingKind::MissingContextMountedAfterAwait,
    );
    summary.code_duplications = report.clone_groups.len();
    summary.complex_functions = report.complexity.len();
    summary.coverage_gaps = kind_count(&report.findings, FindingKind::CoverageGap);
    summary.crap_functions = kind_count(&report.findings, FindingKind::HighCrapScore);
    summary.file_scores = report.file_scores.len();
    summary.hotspots = report.hotspots.len();
    summary.refactoring_targets = report.refactoring_targets.len();
    summary.feature_flags = report.feature_flags.len();
    summary.feature_flag_occurrences = if previous_feature_flags == report.feature_flags.len() {
        previous_feature_flag_occurrences
    } else {
        report
            .feature_flags
            .iter()
            .map(|flag| flag.occurrences.len())
            .sum()
    };
    summary.security_candidates = report.security_candidates.len();
    summary.security_candidate_occurrences =
        if previous_security_candidates == report.security_candidates.len() {
            previous_security_candidate_occurrences
        } else {
            report
                .security_candidates
                .iter()
                .map(|candidate| candidate.occurrences.len())
                .sum()
        };
    summary.attack_surface = report.attack_surface.len();
    summary.missing_entry_points = kind_count(&report.findings, FindingKind::MissingEntryPoint);
    summary.cycles = kind_count(&report.findings, FindingKind::CircularDependency);
    summary.re_export_cycles = kind_count(&report.findings, FindingKind::ReExportCycle);
    summary.boundary_violations = kind_count(&report.findings, FindingKind::BoundaryViolation);
    summary.boundary_coverage = kind_count(&report.findings, FindingKind::BoundaryCoverage);
    summary.boundary_call_violations =
        kind_count(&report.findings, FindingKind::BoundaryCallViolation);
    summary.policy_violations = kind_count(&report.findings, FindingKind::PolicyViolation);
    summary.missing_suppression_reasons =
        kind_count(&report.findings, FindingKind::MissingSuppressionReason);
    summary.findings = report.findings.len();
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

fn complexity_kind(rule_id: &str) -> FindingKind {
    match rule_id.rsplit('/').next() {
        Some("high-cyclomatic-complexity") => FindingKind::HighCyclomaticComplexity,
        Some("high-cognitive-complexity") => FindingKind::HighCognitiveComplexity,
        Some("high-crap-score") => FindingKind::HighCrapScore,
        _ => FindingKind::HighComplexity,
    }
}

fn security_surface_enabled(entry: &JsonAttackSurfaceEntry, rules: &RuleMatcher) -> bool {
    let rule_id = match entry.category {
        SecurityCategory::HardcodedSecret => "dart-decimate/security-hardcoded-secret",
        SecurityCategory::FirebaseApiKey => "dart-decimate/security-firebase-api-key",
        SecurityCategory::InsecureTransport => "dart-decimate/security-insecure-transport",
        SecurityCategory::TlsBypass => "dart-decimate/security-tls-bypass",
        SecurityCategory::WebViewRisk => "dart-decimate/security-webview-risk",
        SecurityCategory::ProcessExecution => "dart-decimate/security-process-execution",
        SecurityCategory::RawSql => "dart-decimate/security-raw-sql",
        SecurityCategory::PlainSecretStorage => "dart-decimate/security-plain-secret-storage",
    };
    rules.level(rule_id, FindingKind::SecurityCandidate) != RuleLevel::Off
}

fn next_step_enabled(id: &str, findings: &[Finding], has_hidden_security_surface: bool) -> bool {
    match id {
        "trace-unused-export" => has_kind(findings, FindingKind::UnusedExport),
        "trace-unused-type" => has_kind(findings, FindingKind::UnusedType),
        "trace-unused-dependency" => findings
            .iter()
            .any(|finding| is_dependency_hygiene_kind(finding.kind)),
        "trace-code-duplication" => has_kind(findings, FindingKind::CodeDuplication),
        "complexity-breakdown" => findings.iter().any(|finding| {
            matches!(
                finding.kind,
                FindingKind::HighCyclomaticComplexity
                    | FindingKind::HighCognitiveComplexity
                    | FindingKind::HighComplexity
                    | FindingKind::HighCrapScore
            )
        }),
        "review-security-surface" => {
            has_hidden_security_surface && has_kind(findings, FindingKind::SecurityCandidate)
        }
        _ => true,
    }
}

fn has_kind(findings: &[Finding], kind: FindingKind) -> bool {
    findings.iter().any(|finding| finding.kind == kind)
}

struct RuleMatcher<'a> {
    rules: &'a RuleConfig,
}

impl<'a> RuleMatcher<'a> {
    fn new(rules: &'a RuleConfig) -> Result<Self, RuleError> {
        for key in rules.keys() {
            if !known_rule(key) {
                return Err(RuleError::UnknownRule { rule: key.clone() });
            }
        }
        Ok(Self { rules })
    }

    fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    fn level(&self, rule_id: &str, kind: FindingKind) -> RuleLevel {
        if let Some(level) = self.rules.get(rule_id).copied() {
            return level;
        }
        aliases(rule_id, kind)
            .into_iter()
            .find_map(|alias| self.rules.get(alias).copied())
            .unwrap_or_else(|| default_rule_level(kind))
    }
}

const fn default_rule_level(kind: FindingKind) -> RuleLevel {
    match kind {
        FindingKind::UnusedDependencyOverride
        | FindingKind::CodeDuplication
        | FindingKind::UnusedWidgetParam
        | FindingKind::UnrenderedWidget => RuleLevel::Warn,
        FindingKind::PrivateWidgetClass
        | FindingKind::WidgetTopLevelFunctionBoundary
        | FindingKind::MissingContextMountedAfterAwait => RuleLevel::Off,
        _ => RuleLevel::Error,
    }
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
