use std::path::Path;

use serde::{Deserialize, Serialize};

use super::format::display_path;
use super::{Finding, FindingAction, FindingKind, Severity};
use crate::{
    FlutterStyleFinding, FlutterStyleFindingKind, FlutterStyleReport, FlutterStyleValueKind,
    HealthReport, ThemeTokenEvidence,
};

/// Opt-in Flutter theme/style advisory serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonFlutterStyleFinding {
    /// Stable advisory identifier.
    pub rule_id: String,
    /// Advisory category.
    pub kind: String,
    /// Candidate source file.
    pub path: String,
    /// 1-based line.
    pub line: usize,
    /// 0-based byte column.
    pub column: usize,
    /// Theme-able value category.
    pub value_kind: String,
    /// Normalized raw value when supported.
    pub value: Option<String>,
    /// Primary theme token for pair/unused rows.
    pub token: Option<JsonThemeTokenEvidence>,
    /// Deterministically selected nearest token.
    pub nearest_token: Option<JsonThemeTokenEvidence>,
    /// Deterministic ARGB distance formatted to two decimal places.
    pub distance: Option<String>,
}

/// Root-relative theme token evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonThemeTokenEvidence {
    /// Stable theme role or custom extension field name.
    pub name: String,
    /// Token source file.
    pub path: String,
    /// 1-based line.
    pub line: usize,
    /// 0-based byte column.
    pub column: usize,
    /// Theme-able value category.
    pub value_kind: String,
    /// Normalized token value when supported.
    pub value: Option<String>,
    /// Whether this is a custom `ThemeExtension` field.
    pub custom: bool,
}

pub(super) fn add_flutter_style_findings(
    root: &Path,
    report: &FlutterStyleReport,
    findings: &mut Vec<Finding>,
) {
    findings.extend(
        report
            .findings
            .iter()
            .map(|finding| flutter_style_finding(root, finding)),
    );
}

pub(super) fn json_flutter_style(
    root: &Path,
    report: &HealthReport,
) -> Vec<JsonFlutterStyleFinding> {
    report
        .flutter_style
        .as_ref()
        .map_or_else(Vec::new, |style| {
            style
                .findings
                .iter()
                .map(|finding| JsonFlutterStyleFinding {
                    rule_id: finding.kind.rule_id().to_owned(),
                    kind: style_finding_kind(finding.kind).to_owned(),
                    path: display_path(root, &finding.path),
                    line: finding.location.line,
                    column: finding.location.column,
                    value_kind: style_value_kind(finding.value_kind).to_owned(),
                    value: finding.value.clone(),
                    token: finding
                        .token
                        .as_ref()
                        .map(|token| json_theme_token(root, token)),
                    nearest_token: finding
                        .nearest_token
                        .as_ref()
                        .map(|token| json_theme_token(root, token)),
                    distance: finding.distance.clone(),
                })
                .collect()
        })
}

fn flutter_style_finding(root: &Path, finding: &FlutterStyleFinding) -> Finding {
    let path = display_path(root, &finding.path);
    let (message, action) = finding_text(finding.kind);
    Finding {
        rule_id: finding.kind.rule_id().to_owned(),
        fingerprint: Some(format!(
            "{}:{path}:{}:{}",
            style_finding_kind(finding.kind),
            finding.location.line,
            finding.location.column
        )),
        kind: finding_kind(finding.kind),
        severity: Severity::Warning,
        message: message.to_owned(),
        path: path.clone(),
        line: finding.location.line,
        column: finding.location.column,
        safe_to_delete: false,
        files: finding
            .nearest_token
            .iter()
            .map(|token| display_path(root, &token.path))
            .collect(),
        edge: None,
        actions: vec![
            FindingAction::new("review-flutter-style", action, false)
                .with_target_path(path)
                .with_suppression_comment(format!(
                    "// dart-decimate-ignore-next-line {} -- <reason>",
                    style_finding_kind(finding.kind)
                )),
        ],
    }
}

const fn finding_text(kind: FlutterStyleFindingKind) -> (&'static str, &'static str) {
    match kind {
        FlutterStyleFindingKind::RawFlutterStyleValue => (
            "Raw Flutter style value duplicates a resolvable theme token",
            "Replace the raw value with the reviewed theme token",
        ),
        FlutterStyleFindingKind::NearDuplicateThemeToken => (
            "Custom theme color tokens are near duplicates",
            "Review whether the token pair should share one semantic role",
        ),
        FlutterStyleFindingKind::UnusedThemeExtensionToken => (
            "Custom ThemeExtension token has no observed member access",
            "Verify dynamic/generated consumers before removing the token",
        ),
    }
}

const fn finding_kind(kind: FlutterStyleFindingKind) -> FindingKind {
    match kind {
        FlutterStyleFindingKind::RawFlutterStyleValue => FindingKind::RawFlutterStyleValue,
        FlutterStyleFindingKind::NearDuplicateThemeToken => FindingKind::NearDuplicateThemeToken,
        FlutterStyleFindingKind::UnusedThemeExtensionToken => {
            FindingKind::UnusedThemeExtensionToken
        }
    }
}

const fn style_finding_kind(kind: FlutterStyleFindingKind) -> &'static str {
    match kind {
        FlutterStyleFindingKind::RawFlutterStyleValue => "raw-flutter-style-value",
        FlutterStyleFindingKind::NearDuplicateThemeToken => "near-duplicate-theme-token",
        FlutterStyleFindingKind::UnusedThemeExtensionToken => "unused-theme-extension-token",
    }
}

const fn style_value_kind(kind: FlutterStyleValueKind) -> &'static str {
    match kind {
        FlutterStyleValueKind::Color => "color",
        FlutterStyleValueKind::TextStyle => "text-style",
    }
}

fn json_theme_token(root: &Path, token: &ThemeTokenEvidence) -> JsonThemeTokenEvidence {
    JsonThemeTokenEvidence {
        name: token.name.clone(),
        path: display_path(root, &token.path),
        line: token.location.line,
        column: token.location.column,
        value_kind: style_value_kind(token.value_kind).to_owned(),
        value: token.value.clone(),
        custom: token.custom,
    }
}
