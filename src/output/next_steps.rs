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

    steps.extend(read_outside_types_steps(root, results));

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

    if let Some(health) = &results.health
        && let Some(complexity) = health.complexity.first()
    {
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

fn read_outside_types_steps(root: &Path, results: &AnalysisResults) -> Vec<NextStep> {
    let Some(report) = results
        .symbols
        .as_ref()
        .filter(|report| report.outside_types_unread > 0)
    else {
        return Vec::new();
    };
    let mut steps = missing_import_steps(&report.outside_missing_imports);
    steps.extend(
        report
        .outside_unread_folders
        .iter()
        .map(|folder| {
            let flutter = std::fs::read_to_string(root.join(folder).join("pubspec.yaml"))
                .is_ok_and(|pubspec| pubspec.contains("sdk: flutter"));
            let tool = if flutter { "flutter pub get" } else { "dart pub get" };
            NextStep {
                id: "read-outside-types".to_owned(),
                command: if folder.is_empty() {
                    tool.to_owned()
                } else {
                    format!("cd {} && {tool}", shell_quote(folder))
                },
                reason: "Write the package config so SDK and package supertypes can be read, then class members that depend on them are checked"
                    .to_owned(),
            }
        }),
    );
    steps
}

fn is_generated_file(file: &str) -> bool {
    [
        ".g.dart",
        ".freezed.dart",
        ".gr.dart",
        ".gen.dart",
        ".mocks.dart",
    ]
    .iter()
    .any(|suffix| file.ends_with(suffix))
}

fn missing_import_steps(missing: &[crate::MissingFile]) -> Vec<NextStep> {
    let mut groups = std::collections::BTreeMap::<(&str, bool), Vec<&crate::MissingFile>>::new();
    for file in missing {
        groups
            .entry((file.folder.as_str(), is_generated_file(&file.file)))
            .or_default()
            .push(file);
    }
    groups
        .into_iter()
        .map(|((folder, generated), files)| {
            let reasons = files
                .iter()
                .map(|file| {
                    let location = if folder.is_empty() {
                        file.file.clone()
                    } else {
                        format!("{folder}/{}", file.file)
                    };
                    let role = if file.part { "a part" } else { "imported" };
                    format!("{location} is {role} but missing")
                })
                .collect::<Vec<_>>()
                .join("; ");
            if generated {
                NextStep {
                    id: "generate-missing-import".to_owned(),
                    command: if folder.is_empty() {
                        "dart run build_runner build".to_owned()
                    } else {
                        format!("cd {} && dart run build_runner build", shell_quote(folder))
                    },
                    reason: format!(
                        "{reasons}; generate them so the types they declare can be read"
                    ),
                }
            } else {
                let names = files
                    .iter()
                    .map(|file| file.file.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                NextStep {
                    id: "fix-missing-import".to_owned(),
                    command: format!("create or fix the import of {names}"),
                    reason: format!(
                        "{reasons}, so supertypes cannot be read and class members stay unchecked"
                    ),
                }
            }
        })
        .collect()
}

fn shell_quote(folder: &str) -> String {
    if folder
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "/._-".contains(character))
    {
        folder.to_owned()
    } else {
        format!("'{}'", folder.replace('\'', "'\\''"))
    }
}
