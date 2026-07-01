use std::collections::BTreeSet;

mod audit_risk;
mod dependency_findings;
mod duplication_findings;
mod feature_flag_findings;
mod format;
mod graph_findings;
mod health_findings;
mod html;
mod human;
mod human_details;
mod next_steps;
mod route_findings;
mod runtime_coverage;
mod scope;
mod security_findings;
mod security_sarif;
mod suppressions;
mod symbol_findings;
mod types;
mod widget_findings;

use crate::{
    BoundaryCallViolation, BoundaryCoverageGap, BoundaryViolation, DeadCodeReport, DependencyCycle,
    DependencyHygieneReport, DuplicateCodeReport, FeatureFlagReport, HealthReport, PolicyViolation,
    ReExportCycle, RouteCollisionReport, SecurityReport, SymbolReport, WidgetReport,
    scan::ScannedProject,
};
pub use audit_risk::apply_audit_risk;
use dependency_findings::add_dependency_hygiene_findings;
pub use duplication_findings::{JsonCloneGroup, JsonCloneInstance};
use duplication_findings::{add_duplication_findings, json_clone_groups};
pub use feature_flag_findings::{JsonFeatureFlag, JsonFeatureFlagOccurrence};
use feature_flag_findings::{add_feature_flag_findings, json_feature_flags};
use graph_findings::{
    add_boundary_call_findings, add_boundary_findings, add_cycle_findings, add_dead_code_findings,
    add_part_of_findings, add_policy_findings, add_re_export_cycle_findings,
    add_unresolved_findings, project_part_of_violation_count, project_unresolved_count,
};
pub use health_findings::{
    JsonComplexityContribution, JsonComplexityFinding, JsonEffectiveThresholds,
    JsonFileHealthScore, JsonHealthHotspot, JsonRefactoringTarget, JsonThresholdOverride,
};
use health_findings::{
    add_health_findings, json_complexity, json_file_scores, json_hotspots,
    json_refactoring_targets, json_threshold_overrides,
};
pub use html::{render_decision_surface_html_report, render_html_report};
pub use human::render_human_report;
use next_steps::next_steps;
use route_findings::add_route_findings;
pub use runtime_coverage::{
    JsonRuntimeBlastRadius, JsonRuntimeCoverage, JsonRuntimeCoverageActionable,
    JsonRuntimeCoverageFinding, JsonRuntimeCoverageIntelligence, JsonRuntimeCoverageProvenance,
    JsonRuntimeCoverageSummary, JsonRuntimeCoverageWatermark, JsonRuntimeHotPath,
    JsonRuntimeImportance, json_runtime_coverage,
};
use scope::{
    file_scope, finding_in_scope, health_file_score_count, project_file_scope_count,
    scope_attack_surface, scope_clone_groups, scope_complexity, scope_feature_flags,
    scope_file_scores, scope_hotspots, scope_refactoring_targets, scope_security_candidates,
    scoped_quality_score,
};
pub use security_findings::{
    JsonAttackSurfaceEntry, JsonSecurityCandidate, JsonSecurityOccurrence, JsonSecurityReachability,
};
use security_findings::{add_security_findings, json_attack_surface, json_security_candidates};
pub(crate) use security_sarif::render_sarif_report;
use suppressions::filter_suppressed_findings;
use symbol_findings::add_symbol_findings;
pub use types::{
    AuditAttribution, AuditAttributionCounts, AuditRiskLevel, Finding, FindingAction, FindingEdge,
    FindingKind, JsonReport, NextStep, ReportCommand, ReportSummary, Severity, Verdict,
};
use widget_findings::add_widget_findings;

/// Stable JSON schema version for agent consumers.
pub const SCHEMA_VERSION: &str = "dart-decimate.report.v1";
pub const TRACE_SCHEMA_VERSION: &str = "dart-decimate.trace.v1";

/// Analysis values to serialize.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisResults {
    /// Command that produced these values.
    pub command: ReportCommand,
    /// Dead-code reachability report, when run.
    pub dead_code: Option<DeadCodeReport>,
    /// Symbol-level dead-code report, when run.
    pub symbols: Option<SymbolReport>,
    /// Circular dependencies.
    pub cycles: Vec<DependencyCycle>,
    /// Export-only circular dependencies.
    pub re_export_cycles: Vec<ReExportCycle>,
    /// Architecture boundary violations.
    pub boundary_violations: Vec<BoundaryViolation>,
    /// Files not covered by any configured architecture boundary zone.
    pub boundary_coverage: Vec<BoundaryCoverageGap>,
    /// Forbidden calls from configured architecture zones.
    pub boundary_call_violations: Vec<BoundaryCallViolation>,
    /// Declarative policy pack violations.
    pub policy_violations: Vec<PolicyViolation>,
    /// Pub dependency hygiene report, when run.
    pub dependency_hygiene: Option<DependencyHygieneReport>,
    /// Code duplication report, when run.
    pub duplicates: Option<DuplicateCodeReport>,
    pub health: Option<HealthReport>,
    pub feature_flags: Option<FeatureFlagReport>,
    pub security: Option<SecurityReport>,
    pub routes: Option<RouteCollisionReport>,
    pub widgets: Option<WidgetReport>,
    pub file_scope: Option<Vec<std::path::PathBuf>>,
    pub require_suppression_reasons: bool,
}

/// Build the stable JSON report for a project analysis.
#[must_use]
pub fn build_json_report(project: &ScannedProject, results: &AnalysisResults) -> JsonReport {
    let scope = file_scope(project, results.file_scope.as_ref());
    let findings = report_findings(project, results, scope.as_ref());
    let mut summary = report_summary(project, results, &findings, scope.as_ref());
    let clone_groups = scope_clone_groups(
        results
            .duplicates
            .as_ref()
            .map_or_else(Vec::new, |report| json_clone_groups(&project.root, report)),
        scope.as_ref(),
    );
    let complexity = scope_complexity(
        results
            .health
            .as_ref()
            .map_or_else(Vec::new, |report| json_complexity(&project.root, report)),
        scope.as_ref(),
    );
    let file_scores = scope_file_scores(
        results
            .health
            .as_ref()
            .map_or_else(Vec::new, |report| json_file_scores(&project.root, report)),
        scope.as_ref(),
    );
    let hotspots = scope_hotspots(
        results
            .health
            .as_ref()
            .map_or_else(Vec::new, |report| json_hotspots(&project.root, report)),
        scope.as_ref(),
    );
    let refactoring_targets = scope_refactoring_targets(
        results.health.as_ref().map_or_else(Vec::new, |report| {
            json_refactoring_targets(&project.root, report)
        }),
        scope.as_ref(),
    );
    let threshold_overrides = json_threshold_overrides_for(results);
    let runtime_coverage = json_runtime_coverage_for(project, results);
    let feature_flags = scope_feature_flags(
        results
            .feature_flags
            .as_ref()
            .map_or_else(Vec::new, |report| json_feature_flags(&project.root, report)),
        scope.as_ref(),
    );
    let security_candidates = scope_security_candidates(
        results.security.as_ref().map_or_else(Vec::new, |report| {
            json_security_candidates(&project.root, report)
        }),
        scope.as_ref(),
    );
    let attack_surface = scope_attack_surface(
        results.security.as_ref().map_or_else(Vec::new, |report| {
            json_attack_surface(&project.root, report)
        }),
        scope.as_ref(),
    );
    let next_steps = if scope.is_some() {
        Vec::new()
    } else {
        next_steps(&project.root, results)
    };
    if scope.is_some() {
        summary.code_duplications = clone_groups.len();
        summary.file_scores = file_scores.len();
        summary.hotspots = hotspots.len();
        summary.refactoring_targets = refactoring_targets.len();
        summary.feature_flags = feature_flags.len();
        summary.feature_flag_occurrences = feature_flags
            .iter()
            .map(|flag| flag.occurrences.len())
            .sum();
        summary.security_candidates = security_candidates.len();
        summary.security_candidate_occurrences = security_candidates
            .iter()
            .map(|candidate| candidate.occurrences.len())
            .sum();
        summary.attack_surface = attack_surface.len();
    }

    JsonReport {
        schema_version: SCHEMA_VERSION.to_owned(),
        kind: results.command.kind().to_owned(),
        tool: "dart-decimate".to_owned(),
        command: results.command,
        verdict: report_verdict(&findings),
        summary,
        findings,
        clone_groups,
        complexity,
        file_scores,
        hotspots,
        refactoring_targets,
        threshold_overrides,
        feature_flags,
        security_candidates,
        attack_surface,
        runtime_coverage,
        next_steps,
    }
}

/// Keep only selected finding kinds and recompute visible report counts.
pub fn filter_report_findings(report: &mut JsonReport, allowed: &[FindingKind]) {
    if allowed.is_empty() {
        return;
    }
    report
        .findings
        .retain(|finding| allowed.contains(&finding.kind));
    if !allowed.contains(&FindingKind::CodeDuplication) {
        report.clone_groups.clear();
    }
    if !allowed.iter().any(|kind| is_complexity_kind(*kind)) {
        report.complexity.clear();
        report.file_scores.clear();
        report.hotspots.clear();
        report.refactoring_targets.clear();
        report.threshold_overrides.clear();
        report.runtime_coverage = None;
    }
    if !allowed.contains(&FindingKind::FeatureFlag) {
        report.feature_flags.clear();
    }
    if !allowed.contains(&FindingKind::SecurityCandidate) {
        report.security_candidates.clear();
        report.attack_surface.clear();
    }
    report.next_steps.clear();
    report.summary.findings = report.findings.len();
    apply_scoped_counts(&mut report.summary, &report.findings);
    report.verdict = report_verdict(&report.findings);
}

fn json_runtime_coverage_for(
    project: &ScannedProject,
    results: &AnalysisResults,
) -> Option<JsonRuntimeCoverage> {
    results.health.as_ref().and_then(|report| {
        report
            .runtime_coverage
            .as_ref()
            .map(|runtime| json_runtime_coverage(&project.root, runtime))
    })
}

fn json_threshold_overrides_for(results: &AnalysisResults) -> Vec<JsonThresholdOverride> {
    results
        .health
        .as_ref()
        .map_or_else(Vec::new, json_threshold_overrides)
}

fn report_verdict(findings: &[Finding]) -> Verdict {
    if findings
        .iter()
        .any(|finding| finding.severity == Severity::Error)
    {
        Verdict::Fail
    } else {
        Verdict::Pass
    }
}

fn report_findings(
    project: &ScannedProject,
    results: &AnalysisResults,
    scope: Option<&BTreeSet<String>>,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    add_unresolved_findings(project, &mut findings);
    add_part_of_findings(project, &mut findings);

    if let Some(dead_code) = &results.dead_code {
        add_dead_code_findings(&project.root, dead_code, &mut findings);
    }
    if let Some(symbols) = &results.symbols {
        add_symbol_findings(&project.root, symbols, &mut findings);
    }

    add_cycle_findings(&project.root, &results.cycles, &mut findings);
    add_re_export_cycle_findings(&project.root, &results.re_export_cycles, &mut findings);
    add_boundary_findings(&project.root, &results.boundary_violations, &mut findings);
    graph_findings::add_boundary_coverage_findings(
        &project.root,
        &results.boundary_coverage,
        &mut findings,
    );
    add_boundary_call_findings(
        &project.root,
        &results.boundary_call_violations,
        &mut findings,
    );
    add_policy_findings(&project.root, &results.policy_violations, &mut findings);
    if let Some(dependency_hygiene) = &results.dependency_hygiene {
        add_dependency_hygiene_findings(&project.root, dependency_hygiene, &mut findings);
    }
    if let Some(duplicates) = &results.duplicates {
        add_duplication_findings(&project.root, duplicates, &mut findings);
    }
    if let Some(health) = &results.health {
        add_health_findings(&project.root, health, &mut findings);
    }
    if let Some(feature_flags) = &results.feature_flags {
        add_feature_flag_findings(&project.root, feature_flags, &mut findings);
    }
    if let Some(security) = &results.security {
        add_security_findings(&project.root, security, &mut findings);
    }
    if let Some(routes) = &results.routes {
        add_route_findings(&project.root, routes, &mut findings);
    }
    if let Some(widgets) = &results.widgets {
        add_widget_findings(&project.root, widgets, &mut findings);
    }

    let mut findings = filter_suppressed_findings(
        &project.root,
        &project.files,
        findings,
        results.require_suppression_reasons,
    )
    .into_iter()
    .filter(|finding| finding_in_scope(finding, scope))
    .collect::<Vec<_>>();
    findings.sort_by(|left, right| {
        (
            &left.rule_id,
            &left.path,
            left.line,
            left.column,
            &left.message,
        )
            .cmp(&(
                &right.rule_id,
                &right.path,
                right.line,
                right.column,
                &right.message,
            ))
    });

    findings
}

fn report_summary(
    project: &ScannedProject,
    results: &AnalysisResults,
    findings: &[Finding],
    scope: Option<&BTreeSet<String>>,
) -> ReportSummary {
    let mut summary = ReportSummary {
        findings: findings.len(),
        ..ReportSummary::default()
    };
    let scoped = results.file_scope.is_some();
    apply_project_summary(&mut summary, project);
    apply_dependency_summary(&mut summary, results, findings);
    apply_cleanup_summary(&mut summary, results, findings);
    apply_widget_summary(&mut summary, findings);
    apply_quality_summary(&mut summary, project, results, findings, scope);
    apply_feature_flag_summary(&mut summary, results);
    apply_security_summary(&mut summary, results);
    apply_graph_policy_summary(&mut summary, results, findings);

    if scoped {
        if let Some(scope) = scope {
            summary.files = project_file_scope_count(project, scope);
        }
        apply_scoped_counts(&mut summary, findings);
    }
    summary
}

fn apply_project_summary(summary: &mut ReportSummary, project: &ScannedProject) {
    summary.files = project.files.len();
    summary.edges = project.graph.edge_count();
    summary.unresolved_dependencies = project_unresolved_count(project);
    summary.part_of_violations = project_part_of_violation_count(project);
}

fn apply_dependency_summary(
    summary: &mut ReportSummary,
    results: &AnalysisResults,
    findings: &[Finding],
) {
    summary.unused_dependencies = results
        .dependency_hygiene
        .as_ref()
        .map_or(0, |report| report.unused_dependencies.len());
    summary.unused_dev_dependencies = kind_count(findings, FindingKind::UnusedDevDependency);
    summary.test_only_dependencies = kind_count(findings, FindingKind::TestOnlyDependency);
    summary.dependency_overrides = dependency_override_count(findings);
    summary.unused_dependency_overrides =
        kind_count(findings, FindingKind::UnusedDependencyOverride);
    summary.misconfigured_dependency_overrides =
        kind_count(findings, FindingKind::MisconfiguredDependencyOverride);
    summary.unlisted_dependencies = results
        .dependency_hygiene
        .as_ref()
        .map_or(0, |report| report.unlisted_dependencies.len());
    summary.private_src_imports = results
        .dependency_hygiene
        .as_ref()
        .map_or(0, |report| report.private_src_imports.len());
}

fn apply_cleanup_summary(
    summary: &mut ReportSummary,
    results: &AnalysisResults,
    findings: &[Finding],
) {
    summary.dead_files = results
        .dead_code
        .as_ref()
        .map_or(0, |dead_code| dead_code.dead_files.len());
    summary.unused_exports = kind_count(findings, FindingKind::UnusedExport);
    summary.unused_types = kind_count(findings, FindingKind::UnusedType);
    summary.private_type_leaks = kind_count(findings, FindingKind::PrivateTypeLeak);
    summary.unused_enum_members = kind_count(findings, FindingKind::UnusedEnumMember);
    summary.unused_class_members = kind_count(findings, FindingKind::UnusedClassMember);
    summary.duplicate_exports = kind_count(findings, FindingKind::DuplicateExport);
    summary.route_collisions = kind_count(findings, FindingKind::RouteCollision);
}

fn apply_widget_summary(summary: &mut ReportSummary, findings: &[Finding]) {
    summary.private_widget_classes = kind_count(findings, FindingKind::PrivateWidgetClass);
    summary.widget_top_level_functions =
        kind_count(findings, FindingKind::WidgetTopLevelFunctionBoundary);
    summary.unused_widget_params = kind_count(findings, FindingKind::UnusedWidgetParam);
    summary.unrendered_widgets = kind_count(findings, FindingKind::UnrenderedWidget);
    summary.missing_context_mounted_after_await =
        kind_count(findings, FindingKind::MissingContextMountedAfterAwait);
}

fn apply_quality_summary(
    summary: &mut ReportSummary,
    project: &ScannedProject,
    results: &AnalysisResults,
    findings: &[Finding],
    scope: Option<&BTreeSet<String>>,
) {
    let health = health_summary_counts(project, results, scope);
    if let Some(report) = &results.duplicates {
        summary.code_duplications = report.clone_groups.len();
        summary.duplication_analyzed_lines = report.stats.analyzed_lines;
        summary.duplicated_lines = report.stats.duplicated_lines;
        summary.duplication_percentage_basis_points =
            report.stats.duplication_percentage_basis_points;
        summary.duplication_threshold_basis_points = report
            .options
            .threshold
            .map(crate::DuplicationThreshold::basis_points);
        summary.duplication_threshold_exceeded = report.stats.threshold_exceeded;
    }
    summary.quality_score = health.quality_score;
    summary.health_files = health.files;
    summary.functions = health.functions;
    summary.complex_functions = health.complex_functions;
    summary.max_cyclomatic_complexity = health.max_cyclomatic_complexity;
    summary.max_cognitive_complexity = health.max_cognitive_complexity;
    summary.coverage_files = health.coverage_files;
    summary.coverage_gaps = kind_count(findings, FindingKind::CoverageGap);
    summary.crap_functions = kind_count(findings, FindingKind::HighCrapScore);
    summary.max_crap_score = health.max_crap_score;
    summary.file_scores = health.file_scores;
    summary.hotspots = kind_count(findings, FindingKind::HealthHotspot);
    summary.refactoring_targets = kind_count(findings, FindingKind::RefactoringTarget);
}

fn apply_feature_flag_summary(summary: &mut ReportSummary, results: &AnalysisResults) {
    summary.feature_flags = results
        .feature_flags
        .as_ref()
        .map_or(0, |report| report.flags.len());
    summary.feature_flag_occurrences = results
        .feature_flags
        .as_ref()
        .map_or(0, |report| report.total_occurrences);
}

fn apply_security_summary(summary: &mut ReportSummary, results: &AnalysisResults) {
    summary.security_candidates = results
        .security
        .as_ref()
        .map_or(0, |report| report.candidates.len());
    summary.security_candidate_occurrences = results
        .security
        .as_ref()
        .map_or(0, |report| report.total_occurrences);
    summary.attack_surface = results
        .security
        .as_ref()
        .map_or(0, |report| report.attack_surface.len());
}

fn apply_graph_policy_summary(
    summary: &mut ReportSummary,
    results: &AnalysisResults,
    findings: &[Finding],
) {
    summary.missing_entry_points = results
        .dead_code
        .as_ref()
        .map_or(0, |dead_code| dead_code.missing_entry_points.len());
    summary.cycles = results.cycles.len();
    summary.re_export_cycles = results.re_export_cycles.len();
    summary.boundary_violations = results.boundary_violations.len();
    summary.boundary_coverage = kind_count(findings, FindingKind::BoundaryCoverage);
    summary.boundary_call_violations = kind_count(findings, FindingKind::BoundaryCallViolation);
    summary.policy_violations = kind_count(findings, FindingKind::PolicyViolation);
    summary.missing_suppression_reasons =
        kind_count(findings, FindingKind::MissingSuppressionReason);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HealthSummaryCounts {
    quality_score: usize,
    files: usize,
    functions: usize,
    complex_functions: usize,
    max_cyclomatic_complexity: usize,
    max_cognitive_complexity: usize,
    coverage_files: usize,
    max_crap_score: usize,
    file_scores: usize,
}
fn health_summary_counts(
    project: &ScannedProject,
    results: &AnalysisResults,
    scope: Option<&BTreeSet<String>>,
) -> HealthSummaryCounts {
    results.health.as_ref().map_or(
        HealthSummaryCounts {
            files: 0,
            quality_score: 0,
            functions: 0,
            complex_functions: 0,
            max_cyclomatic_complexity: 0,
            max_cognitive_complexity: 0,
            coverage_files: 0,
            max_crap_score: 0,
            file_scores: 0,
        },
        |report| HealthSummaryCounts {
            quality_score: scoped_quality_score(project, report, scope),
            files: report.analyzed_files,
            functions: report.functions,
            complex_functions: report.complexity.len() + report.crap.len(),
            max_cyclomatic_complexity: report.max_cyclomatic_complexity,
            max_cognitive_complexity: report.max_cognitive_complexity,
            coverage_files: report.coverage_files,
            max_crap_score: report.max_crap_score,
            file_scores: health_file_score_count(project, &report.file_scores, scope),
        },
    )
}

fn apply_scoped_counts(summary: &mut ReportSummary, findings: &[Finding]) {
    summary.unresolved_dependencies = kind_count(findings, FindingKind::UnresolvedDependency);
    summary.part_of_violations = kind_count(findings, FindingKind::PartOfViolation);
    summary.unused_dependencies = dependency_count(findings);
    summary.unused_dev_dependencies = kind_count(findings, FindingKind::UnusedDevDependency);
    summary.test_only_dependencies = kind_count(findings, FindingKind::TestOnlyDependency);
    summary.dependency_overrides = dependency_override_count(findings);
    summary.unused_dependency_overrides =
        kind_count(findings, FindingKind::UnusedDependencyOverride);
    summary.misconfigured_dependency_overrides =
        kind_count(findings, FindingKind::MisconfiguredDependencyOverride);
    summary.unlisted_dependencies = kind_count(findings, FindingKind::UnlistedDependency);
    summary.private_src_imports = kind_count(findings, FindingKind::PrivateSrcImport);
    summary.dead_files = kind_count(findings, FindingKind::DeadFile);
    summary.unused_exports = kind_count(findings, FindingKind::UnusedExport);
    summary.unused_types = kind_count(findings, FindingKind::UnusedType);
    summary.private_type_leaks = kind_count(findings, FindingKind::PrivateTypeLeak);
    summary.unused_enum_members = kind_count(findings, FindingKind::UnusedEnumMember);
    summary.unused_class_members = kind_count(findings, FindingKind::UnusedClassMember);
    summary.duplicate_exports = kind_count(findings, FindingKind::DuplicateExport);
    summary.route_collisions = kind_count(findings, FindingKind::RouteCollision);
    summary.private_widget_classes = kind_count(findings, FindingKind::PrivateWidgetClass);
    summary.widget_top_level_functions =
        kind_count(findings, FindingKind::WidgetTopLevelFunctionBoundary);
    summary.unused_widget_params = kind_count(findings, FindingKind::UnusedWidgetParam);
    summary.unrendered_widgets = kind_count(findings, FindingKind::UnrenderedWidget);
    summary.missing_context_mounted_after_await =
        kind_count(findings, FindingKind::MissingContextMountedAfterAwait);
    summary.code_duplications = kind_count(findings, FindingKind::CodeDuplication);
    summary.complex_functions = complexity_count(findings);
    summary.coverage_gaps = kind_count(findings, FindingKind::CoverageGap);
    summary.crap_functions = kind_count(findings, FindingKind::HighCrapScore);
    summary.hotspots = kind_count(findings, FindingKind::HealthHotspot);
    summary.refactoring_targets = kind_count(findings, FindingKind::RefactoringTarget);
    summary.feature_flags = kind_count(findings, FindingKind::FeatureFlag);
    summary.feature_flag_occurrences = summary.feature_flags;
    summary.security_candidates = kind_count(findings, FindingKind::SecurityCandidate);
    summary.security_candidate_occurrences = summary.security_candidates;
    summary.attack_surface = 0;
    summary.missing_entry_points = kind_count(findings, FindingKind::MissingEntryPoint);
    summary.cycles = kind_count(findings, FindingKind::CircularDependency);
    summary.re_export_cycles = kind_count(findings, FindingKind::ReExportCycle);
    summary.boundary_violations = kind_count(findings, FindingKind::BoundaryViolation);
    summary.boundary_coverage = kind_count(findings, FindingKind::BoundaryCoverage);
    summary.boundary_call_violations = kind_count(findings, FindingKind::BoundaryCallViolation);
    summary.policy_violations = kind_count(findings, FindingKind::PolicyViolation);
    summary.missing_suppression_reasons =
        kind_count(findings, FindingKind::MissingSuppressionReason);
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

const fn is_complexity_kind(kind: FindingKind) -> bool {
    matches!(
        kind,
        FindingKind::HighCyclomaticComplexity
            | FindingKind::HighCognitiveComplexity
            | FindingKind::HighComplexity
            | FindingKind::HighCrapScore
            | FindingKind::CoverageGap
            | FindingKind::HealthHotspot
            | FindingKind::RefactoringTarget
    )
}

fn complexity_count(findings: &[Finding]) -> usize {
    findings
        .iter()
        .filter(|finding| {
            matches!(
                finding.kind,
                FindingKind::HighCyclomaticComplexity
                    | FindingKind::HighCognitiveComplexity
                    | FindingKind::HighComplexity
                    | FindingKind::HighCrapScore
            )
        })
        .count()
}

#[cfg(test)]
mod tests;
