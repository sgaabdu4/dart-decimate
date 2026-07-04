use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::format::display_path;
use super::{Finding, FindingAction, FindingKind, Severity};
use crate::{FeatureFlag, FeatureFlagConfidence, FeatureFlagReport, FeatureFlagSource};

/// Feature flag serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonFeatureFlag {
    /// Flag key/name.
    pub name: String,
    /// Detection source category.
    pub source: FeatureFlagSource,
    /// Provider or platform surface.
    pub provider: String,
    /// Detection confidence.
    pub confidence: FeatureFlagConfidence,
    /// Occurrences for this flag.
    pub occurrences: Vec<JsonFeatureFlagOccurrence>,
}

/// One feature flag occurrence serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonFeatureFlagOccurrence {
    /// Dart file path, root-relative where possible.
    pub path: String,
    /// 1-based line.
    pub line: usize,
    /// 0-based byte column.
    pub column: usize,
    /// Matched expression or API surface.
    pub expression: String,
}

pub(super) fn add_feature_flag_findings(
    root: &Path,
    report: &FeatureFlagReport,
    findings: &mut Vec<Finding>,
) {
    findings.extend(
        report
            .flags
            .iter()
            .map(|flag| feature_flag_finding(root, flag)),
    );
}

pub(super) fn json_feature_flags(root: &Path, report: &FeatureFlagReport) -> Vec<JsonFeatureFlag> {
    report
        .flags
        .iter()
        .map(|flag| JsonFeatureFlag {
            name: flag.name.clone(),
            source: flag.source,
            provider: flag.provider.clone(),
            confidence: flag.confidence,
            occurrences: flag
                .occurrences
                .iter()
                .map(|occurrence| JsonFeatureFlagOccurrence {
                    path: display_path(root, &occurrence.path),
                    line: occurrence.location.line,
                    column: occurrence.location.column,
                    expression: occurrence.expression.clone(),
                })
                .collect(),
        })
        .collect()
}

fn feature_flag_finding(root: &Path, flag: &FeatureFlag) -> Finding {
    let first = &flag.occurrences[0];
    let path = display_path(root, &first.path);
    let files = flag
        .occurrences
        .iter()
        .map(|occurrence| display_path(root, &occurrence.path))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    Finding {
        rule_id: "dart-decimate/feature-flag".to_owned(),
        fingerprint: Some(feature_flag_fingerprint(flag)),
        kind: FindingKind::FeatureFlag,
        severity: feature_flag_severity(flag),
        message: format!(
            "Feature flag {} is referenced via {}",
            flag.name, flag.provider
        ),
        path: path.clone(),
        line: first.location.line,
        column: first.location.column,
        safe_to_delete: false,
        files,
        edge: None,
        actions: vec![
            FindingAction::new(
                "review-feature-flag",
                "Verify the flag owner, rollout state, and stale-code cleanup path",
                false,
            )
            .with_target_path(path.clone())
            .with_target_symbol(flag.name.clone())
            .with_dart_decimate_args(["inspect", "--format", "json", "--file", path.as_str()])
            .with_suppression_comment("// dart-decimate-ignore-next-line feature-flag"),
        ],
    }
}

fn feature_flag_severity(flag: &FeatureFlag) -> Severity {
    if flag.source == FeatureFlagSource::CompileTimeEnvironment
        && (is_dev_or_test_compile_time_flag_name(&flag.name)
            || flag.occurrences.iter().all(|occurrence| {
                let path = occurrence.path.as_path();
                is_dev_or_test_path(path)
            }))
    {
        Severity::Warning
    } else {
        Severity::Error
    }
}

fn is_dev_or_test_compile_time_flag_name(name: &str) -> bool {
    name.split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .any(|token| {
            matches!(
                token.to_ascii_lowercase().as_str(),
                "debug" | "dev" | "e2e" | "test" | "testing"
            )
        })
}

fn is_dev_or_test_path(path: &Path) -> bool {
    let file_name = path.file_name().and_then(|name| name.to_str());
    file_name.is_some_and(|name| {
        name.contains("_dev.")
            || name.contains("_debug.")
            || name.starts_with("dev_")
            || name.starts_with("debug_")
    }) || path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some("test" | "integration_test" | "test_driver" | "tool" | "scripts")
        )
    })
}

fn feature_flag_fingerprint(flag: &FeatureFlag) -> String {
    let text = format!("{}:{}:{}", flag.name, flag.provider, flag.source as u8);
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("flag:{:08x}", hash & 0xffff_ffff)
}
