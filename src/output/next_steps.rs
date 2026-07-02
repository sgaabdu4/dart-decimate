use std::path::Path;

use super::format::display_path;
use super::{AnalysisResults, NextStep};
use crate::DeclarationKind;

pub(super) fn next_steps(root: &Path, results: &AnalysisResults) -> Vec<NextStep> {
    let mut steps = Vec::new();

    if let Some(unused) = results.symbols.as_ref().and_then(|report| {
        report
            .unused_exports
            .iter()
            .find(|unused| unused.kind != DeclarationKind::TypeAlias)
    }) {
        steps.push(NextStep {
            id: "trace-unused-export".to_owned(),
            command: format!(
                "dart-decimate trace-symbol --format json --symbol {}:{}",
                display_path(root, &unused.path),
                unused.name
            ),
            reason: "Trace the first unused export before deleting or suppressing it".to_owned(),
        });
    }

    if let Some(unused) = results.symbols.as_ref().and_then(|report| {
        report
            .unused_exports
            .iter()
            .find(|unused| unused.kind == DeclarationKind::TypeAlias)
    }) {
        steps.push(NextStep {
            id: "trace-unused-type".to_owned(),
            command: format!(
                "dart-decimate trace-symbol --format json --symbol {}:{}",
                display_path(root, &unused.path),
                unused.name
            ),
            reason: "Trace the first unused type alias before deleting or suppressing it"
                .to_owned(),
        });
    }

    if let Some(unused) = results
        .dependency_hygiene
        .as_ref()
        .and_then(|report| report.unused_dependencies.first())
    {
        steps.push(NextStep {
            id: "trace-unused-dependency".to_owned(),
            command: format!(
                "dart-decimate trace-dependency --format json --dependency {}",
                unused.dependency
            ),
            reason: "Trace the first unused dependency before editing pubspec.yaml".to_owned(),
        });
    }

    if let Some(clone) = results
        .duplicates
        .as_ref()
        .and_then(|report| report.clone_groups.first())
    {
        steps.push(NextStep {
            id: "trace-code-duplication".to_owned(),
            command: format!(
                "dart-decimate trace-clone --format json --fingerprint {}",
                clone.fingerprint
            ),
            reason: "Trace the first duplicate code group before extracting shared code".to_owned(),
        });
    }

    if let Some(health) = &results.health {
        if let Some(complexity) = health.complexity.first() {
            steps.push(NextStep {
                id: "complexity-breakdown".to_owned(),
                command: format!(
                    "dart-decimate health --format json --complexity-breakdown --top 1 --max-cyclomatic {} --max-cognitive {}",
                    health.options.max_cyclomatic, health.options.max_cognitive
                ),
                reason: format!(
                    "Explain the decision points driving complexity in {}",
                    display_path(root, &complexity.path)
                ),
            });
        }
    }

    if results.security.as_ref().is_some_and(|report| {
        report.total_occurrences > report.candidates.len() && !report.candidates.is_empty()
    }) {
        steps.push(NextStep {
            id: "review-security-surface".to_owned(),
            command: "dart-decimate security . --format json --surface".to_owned(),
            reason: "List all security candidate occurrences behind representative findings"
                .to_owned(),
        });
    }

    steps
}
