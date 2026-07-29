use std::collections::BTreeSet;

use super::flutter_style::json_flutter_style;
use super::health_findings::{json_complexity, json_large_functions, json_threshold_overrides};
use super::runtime_coverage::{JsonRuntimeCoverage, json_runtime_coverage};
use super::scope::{scope_complexity, scope_flutter_style, scope_large_functions, scope_semantic};
use super::{
    AnalysisResults, JsonAttackSurfaceEntry, JsonCloneGroup, JsonComplexityFinding,
    JsonFeatureFlag, JsonFileHealthScore, JsonFlutterStyleFinding, JsonHealthHotspot,
    JsonLargeFunction, JsonRefactoringTarget, JsonSecurityBlindSpot, JsonSecurityCandidate,
    JsonThresholdOverride, ReportSummary,
};
use crate::ScannedProject;

pub(super) fn health_and_semantic_inventories(
    project: &ScannedProject,
    results: &AnalysisResults,
    scope: Option<&BTreeSet<String>>,
) -> (
    Vec<JsonComplexityFinding>,
    Vec<JsonLargeFunction>,
    Vec<JsonFlutterStyleFinding>,
    Option<crate::SemanticReport>,
) {
    let (complexity, large_functions, flutter_style) = results.health.as_ref().map_or_else(
        || (Vec::new(), Vec::new(), Vec::new()),
        |report| {
            (
                scope_complexity(json_complexity(&project.root, report), scope),
                scope_large_functions(json_large_functions(&project.root, report), scope),
                scope_flutter_style(json_flutter_style(&project.root, report), scope),
            )
        },
    );
    let semantic = scope_semantic(
        results
            .symbols
            .as_ref()
            .map(|report| report.semantic.clone()),
        scope,
    );
    (complexity, large_functions, flutter_style, semantic)
}

pub(super) fn apply_semantic_summary(
    summary: &mut ReportSummary,
    semantic: Option<&crate::SemanticReport>,
) {
    summary.semantic_evidence = semantic.map_or(0, |report| report.evidence.len());
    summary.type_couplings = semantic.map_or(0, |report| report.type_couplings.len());
}

pub(super) fn apply_flutter_style_summary(
    summary: &mut ReportSummary,
    findings: &[JsonFlutterStyleFinding],
) {
    summary.raw_flutter_style_values = findings
        .iter()
        .filter(|finding| finding.kind == "raw-flutter-style-value")
        .count();
    summary.near_duplicate_theme_tokens = findings
        .iter()
        .filter(|finding| finding.kind == "near-duplicate-theme-token")
        .count();
    summary.unused_theme_extension_tokens = findings
        .iter()
        .filter(|finding| finding.kind == "unused-theme-extension-token")
        .count();
}

pub(super) fn apply_health_inventory_summary(
    summary: &mut ReportSummary,
    clone_groups: &[JsonCloneGroup],
    file_scores: &[JsonFileHealthScore],
    hotspots: &[JsonHealthHotspot],
    targets: &[JsonRefactoringTarget],
) {
    summary.code_duplications = clone_groups.len();
    summary.file_scores = file_scores.len();
    summary.hotspots = hotspots.len();
    summary.refactoring_targets = targets.len();
}

pub(super) fn apply_aux_inventory_summary(
    summary: &mut ReportSummary,
    flags: &[JsonFeatureFlag],
    security: &[JsonSecurityCandidate],
    blind_spots: &[JsonSecurityBlindSpot],
    attack_surface: &[JsonAttackSurfaceEntry],
) {
    summary.feature_flags = flags.len();
    summary.feature_flag_occurrences = flags.iter().map(|flag| flag.occurrences.len()).sum();
    summary.security_candidates = security.len();
    summary.security_candidate_occurrences = security
        .iter()
        .map(|candidate| candidate.occurrences.len())
        .sum();
    summary.security_blind_spots = blind_spots.len();
    summary.attack_surface = attack_surface.len();
}

pub(super) fn json_runtime_coverage_for(
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

pub(super) fn json_threshold_overrides_for(
    results: &AnalysisResults,
) -> Vec<JsonThresholdOverride> {
    results
        .health
        .as_ref()
        .map_or_else(Vec::new, json_threshold_overrides)
}
