use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use tree_sitter::Node;

use crate::generated::is_generated_dart_path;
use crate::graph::normalize_against;
use crate::{Location, ScannedProject};

mod coverage;
mod large_units;
mod ownership;
mod runtime_coverage;
mod runtime_intelligence;
mod scores;
mod threshold_types;
mod thresholds;
mod types;
use coverage::{CoverageMap, coverage_gap_findings, crap_findings, load_lcov};
use large_units::large_functions;
use runtime_coverage::load_runtime_coverage;
pub use runtime_intelligence::{
    RuntimeBlastRadius, RuntimeBlastRisk, RuntimeCoverageAction, RuntimeCoverageIntelligence,
    RuntimeCoverageIntelligenceKind, RuntimeImportance,
};
use scores::{file_health_scores, health_hotspots, project_quality_score, refactoring_targets};
use threshold_types::AppliedThresholds;
pub use threshold_types::{
    EffectiveThresholds, HealthThresholdOverride, HealthThresholdOverrideReport,
    HealthThresholdOverrideStatus, ThresholdSource,
};
use thresholds::ThresholdContext;
pub use types::{
    ComplexityContribution, ComplexityFinding, ComplexityFunctionKind, ComplexityRule,
    CoverageGapFinding, CoverageGapReason, CrapFinding, FileCoverageStatus, FileHealthScore,
    HealthError, HealthHotspot, HealthOptions, HealthReport, HealthToggle, LargeFunction,
    LowTrafficThreshold, RefactoringTarget, RuntimeCoverageConfidence, RuntimeCoverageFinding,
    RuntimeCoverageFindingKind, RuntimeCoverageFormat, RuntimeCoverageReport, RuntimeHotPath,
    SourceMapConfidence,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct FunctionMetrics {
    pub(super) path: PathBuf,
    pub(super) symbol: String,
    pub(super) kind: ComplexityFunctionKind,
    pub(super) location: Location,
    pub(super) end_line: usize,
    pub(super) cyclomatic: usize,
    pub(super) cognitive: usize,
    pub(super) contributions: Vec<ComplexityContribution>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ComplexityState {
    cyclomatic: usize,
    cognitive: usize,
    contributions: Vec<ComplexityContribution>,
}

/// Analyze Dart function complexity for a scanned project.
///
/// # Errors
///
/// Returns [`HealthError`] when a source file cannot be read or parsed.
pub fn analyze_health(
    project: &ScannedProject,
    options: &HealthOptions,
) -> Result<HealthReport, HealthError> {
    let (functions, analyzed_files) = collect_project_functions(project)?;
    let mut threshold_context = ThresholdContext::new(&project.root, options);
    let coverage = load_coverage(project, options)?;
    let runtime_coverage = load_runtime_coverage(project, options, &functions)?;
    let coverage_files = coverage.as_ref().map_or(0, CoverageMap::file_count);
    let complexity = complexity_findings(&functions, options, &mut threshold_context);
    let large_functions = large_functions(&functions, &mut threshold_context);
    let flutter_style = options
        .flutter_style
        .is_enabled()
        .then(|| crate::analyze_flutter_style(project))
        .transpose()?;
    let coverage_gaps = coverage
        .as_ref()
        .filter(|_| options.coverage_gaps.is_enabled())
        .map_or_else(Vec::new, |coverage| {
            coverage_gap_findings(&functions, coverage)
        });
    let mut crap = coverage.as_ref().map_or_else(Vec::new, |coverage| {
        crap_findings(&functions, coverage, &mut threshold_context)
    });
    sort_crap_findings(&mut crap);
    let threshold_overrides = threshold_context.reports();
    let include_hotspots = options.hotspots.is_enabled()
        || options.targets.is_enabled()
        || options.ownership.is_enabled();
    let all_file_scores =
        file_health_scores(&functions, &complexity, &crap, coverage.as_ref(), options);
    let quality_score = project_quality_score(&all_file_scores);
    let mut file_scores =
        if options.file_scores.is_enabled() || include_hotspots || options.ownership.is_enabled() {
            all_file_scores.clone()
        } else {
            Vec::new()
        };
    let mut hotspots = if include_hotspots {
        health_hotspots(&file_scores, options.min_score)
    } else {
        Vec::new()
    };
    let mut refactoring_targets = if options.targets.is_enabled() {
        refactoring_targets(&file_scores)
    } else {
        Vec::new()
    };
    if options.ownership.is_enabled() {
        ownership::apply_ownership(
            &project.root,
            &mut file_scores,
            &mut hotspots,
            &mut refactoring_targets,
        );
    }

    Ok(HealthReport {
        options: options.clone(),
        quality_score,
        analyzed_files,
        functions: functions.len(),
        max_cyclomatic_complexity: max_cyclomatic_complexity(&functions),
        max_cognitive_complexity: max_cognitive_complexity(&functions),
        coverage_files,
        max_crap_score: max_crap_score(&crap),
        complexity,
        large_functions,
        flutter_style,
        coverage_gaps,
        crap,
        threshold_overrides,
        runtime_coverage,
        file_scores,
        hotspots,
        refactoring_targets,
    })
}

fn collect_project_functions(
    project: &ScannedProject,
) -> Result<(Vec<FunctionMetrics>, usize), HealthError> {
    let flutter_classes =
        crate::widgets::flutter_framework_classes(project).map_err(widget_health_error)?;
    let mut functions = Vec::new();
    let mut analyzed_files = 0;
    for file in &project.files {
        let path = normalize_against(&project.root, &file.path);
        if !path.starts_with(&project.root) || is_ignored_path(&path) {
            continue;
        }
        analyzed_files += 1;
        let source = fs::read_to_string(&path).map_err(|source| HealthError::ReadFile {
            path: path.clone(),
            source,
        })?;
        let parsed = crate::dart_parser::parse_dart_source_lossy(&path, &source)
            .map_err(health_parse_error)?;
        collect_functions(
            parsed.tree().root_node(),
            parsed.source(),
            &path,
            &flutter_classes,
            &mut functions,
        );
    }

    Ok((functions, analyzed_files))
}

fn widget_health_error(error: crate::WidgetAnalysisError) -> HealthError {
    match error {
        crate::WidgetAnalysisError::ReadFile { path, source } => {
            HealthError::ReadFile { path, source }
        }
        crate::WidgetAnalysisError::Language(source) => HealthError::Language(source),
        crate::WidgetAnalysisError::ParseCancelled { path }
        | crate::WidgetAnalysisError::Syntax { path } => HealthError::ParseCancelled { path },
    }
}

fn health_parse_error(error: crate::dart_parser::DartParseError) -> HealthError {
    match error {
        crate::dart_parser::DartParseError::Language(source) => HealthError::Language(source),
        crate::dart_parser::DartParseError::ParseCancelled { path } => {
            HealthError::ParseCancelled { path }
        }
        crate::dart_parser::DartParseError::Syntax { path } => HealthError::ParseCancelled { path },
    }
}

fn max_cyclomatic_complexity(functions: &[FunctionMetrics]) -> usize {
    functions
        .iter()
        .map(|function| function.cyclomatic)
        .max()
        .unwrap_or(0)
}

fn max_cognitive_complexity(functions: &[FunctionMetrics]) -> usize {
    functions
        .iter()
        .map(|function| function.cognitive)
        .max()
        .unwrap_or(0)
}

fn complexity_findings(
    functions: &[FunctionMetrics],
    options: &HealthOptions,
    threshold_context: &mut ThresholdContext,
) -> Vec<ComplexityFinding> {
    let mut complexity = functions
        .iter()
        .cloned()
        .filter_map(|function| threshold_finding(function, options, threshold_context))
        .collect::<Vec<_>>();

    sort_complexity_findings(&mut complexity);
    if let Some(top) = options.top {
        complexity.truncate(top);
    }
    if !options.complexity_breakdown.is_enabled() {
        for finding in &mut complexity {
            finding.contributions.clear();
        }
    }
    complexity
}

fn sort_complexity_findings(findings: &mut [ComplexityFinding]) {
    findings.sort_by(|left, right| {
        (
            std::cmp::Reverse(left.cyclomatic_complexity),
            std::cmp::Reverse(left.cognitive_complexity),
            &left.path,
            left.location.line,
            left.location.column,
            &left.symbol,
        )
            .cmp(&(
                std::cmp::Reverse(right.cyclomatic_complexity),
                std::cmp::Reverse(right.cognitive_complexity),
                &right.path,
                right.location.line,
                right.location.column,
                &right.symbol,
            ))
    });
}

fn sort_crap_findings(findings: &mut [CrapFinding]) {
    findings.sort_by(|left, right| {
        (
            std::cmp::Reverse(left.crap_score),
            std::cmp::Reverse(left.cyclomatic_complexity),
            &left.path,
            left.location.line,
            left.location.column,
            &left.symbol,
        )
            .cmp(&(
                std::cmp::Reverse(right.crap_score),
                std::cmp::Reverse(right.cyclomatic_complexity),
                &right.path,
                right.location.line,
                right.location.column,
                &right.symbol,
            ))
    });
}

fn max_crap_score(findings: &[CrapFinding]) -> usize {
    findings
        .iter()
        .map(|finding| finding.crap_score)
        .max()
        .unwrap_or(0)
}

fn collect_functions(
    node: Node<'_>,
    source: &str,
    path: &Path,
    flutter_classes: &BTreeSet<(PathBuf, String)>,
    functions: &mut Vec<FunctionMetrics>,
) {
    if let Some(function) = function_from_node(node, source, path, flutter_classes) {
        functions.push(function);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_functions(child, source, path, flutter_classes, functions);
    }
}

fn function_from_node(
    node: Node<'_>,
    source: &str,
    path: &Path,
    flutter_classes: &BTreeSet<(PathBuf, String)>,
) -> Option<FunctionMetrics> {
    let (symbol, mut kind, body) = match node.kind() {
        "function_declaration" => named_function(node, source, ComplexityFunctionKind::Function)?,
        "getter_declaration" => named_function(node, source, ComplexityFunctionKind::Getter)?,
        "setter_declaration" => named_function(node, source, ComplexityFunctionKind::Setter)?,
        "method_declaration" => method_function(node, source)?,
        "function_expression" => (
            "<closure>".to_owned(),
            ComplexityFunctionKind::Closure,
            find_first_named_descendant(node, "function_expression_body")?,
        ),
        _ => return None,
    };
    if symbol == "build"
        && kind == ComplexityFunctionKind::Method
        && is_flutter_build_method(node, source, path, flutter_classes)
    {
        kind = ComplexityFunctionKind::FlutterBuildMethod;
    }

    let state = score_body(body);
    Some(FunctionMetrics {
        path: path.to_path_buf(),
        symbol,
        kind,
        location: node.start_position().into(),
        end_line: node.end_position().row + 1,
        cyclomatic: state.cyclomatic,
        cognitive: state.cognitive,
        contributions: state.contributions,
    })
}

fn is_flutter_build_method(
    node: Node<'_>,
    source: &str,
    path: &Path,
    flutter_classes: &BTreeSet<(PathBuf, String)>,
) -> bool {
    let mut ancestor = node.parent();
    while let Some(candidate) = ancestor {
        if candidate.kind() == "class_declaration" {
            let name = field_text(candidate, "name", source);
            return name.is_some_and(|name| flutter_classes.contains(&(path.to_path_buf(), name)));
        }
        ancestor = candidate.parent();
    }
    false
}

fn named_function<'tree>(
    node: Node<'tree>,
    source: &str,
    kind: ComplexityFunctionKind,
) -> Option<(String, ComplexityFunctionKind, Node<'tree>)> {
    let signature = node.child_by_field_name("signature")?;
    let symbol = field_text(signature, "name", source)?;
    let body = node.child_by_field_name("body")?;
    Some((symbol, kind, body))
}

fn method_function<'tree>(
    node: Node<'tree>,
    source: &str,
) -> Option<(String, ComplexityFunctionKind, Node<'tree>)> {
    let signature = node.child_by_field_name("signature")?;
    let body = node.child_by_field_name("body")?;
    let Some(inner) = first_named_child(signature) else {
        return Some(("<method>".to_owned(), ComplexityFunctionKind::Method, body));
    };
    let (symbol, kind) = match inner.kind() {
        "function_signature" | "getter_signature" | "setter_signature" => (
            field_text(inner, "name", source).unwrap_or_else(|| "<method>".to_owned()),
            ComplexityFunctionKind::Method,
        ),
        "operator_signature" => (
            operator_name(inner, source).unwrap_or_else(|| "operator".to_owned()),
            ComplexityFunctionKind::Operator,
        ),
        "constructor_signature"
        | "constant_constructor_signature"
        | "factory_constructor_signature"
        | "redirecting_factory_constructor_signature" => (
            constructor_name(inner, source).unwrap_or_else(|| "<constructor>".to_owned()),
            ComplexityFunctionKind::Constructor,
        ),
        _ => ("<method>".to_owned(), ComplexityFunctionKind::Method),
    };

    Some((symbol, kind, body))
}

fn score_body(body: Node<'_>) -> ComplexityState {
    let mut state = ComplexityState {
        cyclomatic: 1,
        cognitive: 0,
        contributions: Vec::new(),
    };
    walk_complexity(body, 0, &mut state);
    state
}

fn walk_complexity(node: Node<'_>, nesting: usize, state: &mut ComplexityState) {
    if matches!(
        node.kind(),
        "function_declaration" | "method_declaration" | "function_expression"
    ) {
        return;
    }

    let decision = decision_for(node);
    if let Some(decision) = decision {
        let cognitive = decision.cognitive + usize::from(decision.nesting_penalty) * nesting;
        state.cyclomatic += decision.cyclomatic;
        state.cognitive += cognitive;
        state.contributions.push(ComplexityContribution {
            location: node.start_position().into(),
            kind: decision.kind.to_owned(),
            cyclomatic: decision.cyclomatic,
            cognitive,
            nesting,
        });
    }

    let child_nesting = nesting + usize::from(decision.is_some_and(|item| item.nests_children));
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if should_skip_decision_child(node, child) {
            continue;
        }
        walk_complexity(child, child_nesting, state);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Decision {
    kind: &'static str,
    cyclomatic: usize,
    cognitive: usize,
    nesting_penalty: bool,
    nests_children: bool,
}

fn decision_for(node: Node<'_>) -> Option<Decision> {
    let decision = match node.kind() {
        "if_statement" => Decision {
            kind: "if",
            cyclomatic: 1,
            cognitive: 1,
            nesting_penalty: true,
            nests_children: true,
        },
        "for_statement" | "for_in_statement" | "while_statement" | "do_statement" => Decision {
            kind: "loop",
            cyclomatic: 1,
            cognitive: 1,
            nesting_penalty: true,
            nests_children: true,
        },
        "catch_clause" => Decision {
            kind: "catch",
            cyclomatic: 1,
            cognitive: 1,
            nesting_penalty: true,
            nests_children: true,
        },
        "conditional_expression" => Decision {
            kind: "ternary",
            cyclomatic: 1,
            cognitive: 1,
            nesting_penalty: true,
            nests_children: true,
        },
        "switch_statement_case" | "switch_expression_case" => Decision {
            kind: "case",
            cyclomatic: 1,
            cognitive: 1,
            nesting_penalty: false,
            nests_children: true,
        },
        "logical_and_expression" => boolean_decision("&&"),
        "logical_or_expression" => boolean_decision("||"),
        "if_null_expression" => boolean_decision("??"),
        _ => return None,
    };
    Some(decision)
}

fn boolean_decision(kind: &'static str) -> Decision {
    Decision {
        kind,
        cyclomatic: 1,
        cognitive: 1,
        nesting_penalty: false,
        nests_children: false,
    }
}

fn should_skip_decision_child(parent: Node<'_>, child: Node<'_>) -> bool {
    matches!(
        (parent.kind(), child.kind()),
        ("logical_and_expression", "logical_and_expression")
            | ("logical_or_expression", "logical_or_expression")
            | ("if_null_expression", "if_null_expression")
    )
}

fn threshold_finding(
    function: FunctionMetrics,
    options: &HealthOptions,
    threshold_context: &mut ThresholdContext,
) -> Option<ComplexityFinding> {
    let thresholds = threshold_context.static_thresholds(&function);
    let max_cyclomatic = thresholds
        .effective
        .max_cyclomatic
        .unwrap_or(options.max_cyclomatic);
    let max_cognitive = thresholds
        .effective
        .max_cognitive
        .unwrap_or(options.max_cognitive);
    let high_cyclomatic = function.cyclomatic > max_cyclomatic;
    let high_cognitive = function.cognitive > max_cognitive;
    if !high_cyclomatic && !high_cognitive {
        return None;
    }

    let rule = match (high_cyclomatic, high_cognitive) {
        (true, true) => ComplexityRule::HighComplexity,
        (true, false) => ComplexityRule::HighCyclomaticComplexity,
        (false, true) => ComplexityRule::HighCognitiveComplexity,
        (false, false) => unreachable!("guarded above"),
    };

    Some(ComplexityFinding {
        path: function.path,
        symbol: function.symbol,
        kind: function.kind,
        location: function.location,
        cyclomatic_complexity: function.cyclomatic,
        cognitive_complexity: function.cognitive,
        rule,
        effective_thresholds: override_thresholds(&thresholds),
        threshold_source: thresholds.source,
        threshold_reason: thresholds.reason,
        contributions: function.contributions,
    })
}

fn override_thresholds(thresholds: &AppliedThresholds) -> Option<EffectiveThresholds> {
    thresholds.source.map(|_| thresholds.effective.clone())
}

fn load_coverage(
    project: &ScannedProject,
    options: &HealthOptions,
) -> Result<Option<CoverageMap>, HealthError> {
    if let Some(path) = &options.coverage_path {
        return load_lcov(&project.root, &normalize_against(&project.root, path)).map(Some);
    }

    if options.coverage_gaps.is_enabled() || has_crap_threshold(options) {
        let path = project.root.join("coverage/lcov.info");
        if path.is_file() {
            return load_lcov(&project.root, &path).map(Some);
        }
        return Err(HealthError::MissingCoverageData {
            root: project.root.clone(),
        });
    }

    Ok(None)
}

fn has_crap_threshold(options: &HealthOptions) -> bool {
    options.max_crap.is_some()
        || options
            .threshold_overrides
            .iter()
            .any(HealthThresholdOverride::has_crap_threshold)
}

pub(super) fn is_ignored_path(path: &Path) -> bool {
    if is_generated_dart_path(path) {
        return true;
    }

    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some("test" | "integration_test" | "test_driver" | "__tests__" | "__mocks__")
        )
    })
}

fn field_text(node: Node<'_>, field_name: &str, source: &str) -> Option<String> {
    node.child_by_field_name(field_name)
        .and_then(|child| child.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn operator_name(node: Node<'_>, source: &str) -> Option<String> {
    node.child_by_field_name("operator")
        .and_then(|child| child.utf8_text(source.as_bytes()).ok())
        .map(|operator| format!("operator {operator}"))
}

fn constructor_name(node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    let names = node
        .children_by_field_name("name", &mut cursor)
        .filter(|child| child.kind() == "identifier")
        .filter_map(|child| child.utf8_text(source.as_bytes()).ok())
        .collect::<Vec<_>>();
    if names.is_empty() {
        None
    } else {
        Some(names.join("."))
    }
}

fn first_named_child(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).next()
}

fn find_first_named_descendant<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    if node.kind() == kind {
        return Some(node);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(found) = find_first_named_descendant(child, kind) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests;
