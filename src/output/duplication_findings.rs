use std::collections::BTreeSet;
use std::path::Path;

use super::format::display_path;
use super::{Finding, FindingAction, FindingKind, Severity};
use crate::DuplicateCodeReport;
use serde::{Deserialize, Serialize};

/// Clone group serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonCloneGroup {
    /// Stable clone fingerprint.
    pub fingerprint: String,
    /// Matching clone instances.
    pub instances: Vec<JsonCloneInstance>,
    /// Normalized lines in the matched duplicated block.
    pub line_count: usize,
    /// Normalized tokens in the matched duplicated block.
    pub token_count: usize,
}

/// Clone instance serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonCloneInstance {
    /// Dart file path, root-relative where possible.
    pub path: String,
    /// 1-based start line.
    pub start_line: usize,
    /// 1-based end line.
    pub end_line: usize,
    /// 0-based byte column.
    pub column: usize,
}

pub(super) fn add_duplication_findings(
    root: &Path,
    report: &DuplicateCodeReport,
    findings: &mut Vec<Finding>,
) {
    findings.extend(
        report
            .clone_groups
            .iter()
            .map(|clone| code_duplication_finding(root, clone)),
    );
}

pub(super) fn json_clone_groups(root: &Path, report: &DuplicateCodeReport) -> Vec<JsonCloneGroup> {
    report
        .clone_groups
        .iter()
        .map(|clone| JsonCloneGroup {
            fingerprint: clone.fingerprint.clone(),
            instances: clone
                .instances
                .iter()
                .map(|instance| JsonCloneInstance {
                    path: display_path(root, &instance.path),
                    start_line: instance.start_line,
                    end_line: instance.end_line,
                    column: instance.column,
                })
                .collect(),
            line_count: clone.line_count,
            token_count: clone.token_count,
        })
        .collect()
}

fn code_duplication_finding(root: &Path, clone: &crate::CodeClone) -> Finding {
    let first = &clone.instances[0];
    let path = display_path(root, &first.path);
    let files = clone
        .instances
        .iter()
        .map(|instance| display_path(root, &instance.path))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    Finding {
        rule_id: "dart-decimate/code-duplication".to_owned(),
        fingerprint: Some(clone.fingerprint.clone()),
        kind: FindingKind::CodeDuplication,
        severity: Severity::Warning,
        message: format!(
            "{} duplicated Dart lines appear in {} places",
            clone.line_count,
            clone.instances.len()
        ),
        path: path.clone(),
        line: first.start_line,
        column: first.column,
        safe_to_delete: false,
        files,
        edge: None,
        actions: vec![
            FindingAction::new(
                "trace-clone",
                "Trace this clone group before extracting shared code",
                false,
            )
            .with_target_path(path.clone())
            .with_dart_decimate_args([
                "trace-clone",
                "--format",
                "json",
                "--fingerprint",
                clone.fingerprint.as_str(),
            ])
            .with_suppression_comment("// dart-decimate-ignore-next-line code-duplication"),
        ],
    }
}
