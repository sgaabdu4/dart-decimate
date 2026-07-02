use super::*;
use crate::output::{FindingAction, FindingEdge, NextStep, ReportCommand, ReportSummary};

#[test]
fn renders_browser_ready_report_with_escaped_evidence() {
    let report = JsonReport {
        schema_version: "dart-decimate.report.v1".to_owned(),
        kind: "combined".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: ReportCommand::Check,
        verdict: Verdict::Fail,
        summary: ReportSummary {
            files: 3,
            edges: 2,
            findings: 1,
            cycles: 1,
            ..ReportSummary::default()
        },
        findings: vec![Finding {
            rule_id: "dart-decimate/circular-dependency".to_owned(),
            fingerprint: Some("cycle:<unsafe>".to_owned()),
            kind: FindingKind::CircularDependency,
            severity: Severity::Error,
            message: "Circular dependency spans 3 Dart files".to_owned(),
            path: "lib/a.dart".to_owned(),
            line: 1,
            column: 0,
            safe_to_delete: false,
            files: vec![
                "lib/a.dart".to_owned(),
                "lib/b.dart".to_owned(),
                "lib/c.dart".to_owned(),
            ],
            edge: None,
            actions: vec![
                FindingAction::new(
                    "break-cycle",
                    "Move shared dependencies inward or invert one import edge",
                    false,
                )
                .with_dart_decimate_args([
                    "inspect",
                    "--format",
                    "json",
                    "--file",
                    "lib/a.dart",
                ]),
            ],
        }],
        clone_groups: Vec::new(),
        complexity: Vec::new(),
        file_scores: Vec::new(),
        hotspots: Vec::new(),
        refactoring_targets: Vec::new(),
        threshold_overrides: Vec::new(),
        feature_flags: Vec::new(),
        security_candidates: Vec::new(),
        attack_surface: Vec::new(),
        runtime_coverage: None,
        next_steps: Vec::new(),
    };

    let rendered = render_html_report(&report);

    assert!(rendered.starts_with("<!doctype html>"));
    assert!(rendered.contains("<h1>check report</h1>"));
    assert!(rendered.contains("Architecture"));
    assert!(rendered.contains("1 circular dependency"));
    assert!(rendered.contains("Why"));
    assert!(rendered.contains("cycle:&lt;unsafe&gt;"));
    assert!(rendered.contains("lib/a.dart -&gt; lib/b.dart -&gt; lib/c.dart -&gt; lib/a.dart"));
}

#[test]
fn groups_findings_by_type_with_search_and_type_controls() {
    let report = JsonReport {
        schema_version: "dart-decimate.report.v1".to_owned(),
        kind: "combined".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: ReportCommand::Check,
        verdict: Verdict::Fail,
        summary: ReportSummary {
            files: 4,
            edges: 3,
            findings: 3,
            unresolved_dependencies: 2,
            unused_widget_params: 1,
            ..ReportSummary::default()
        },
        findings: vec![
            finding(
                FindingKind::UnresolvedDependency,
                "dart-decimate/unresolved-dependency",
                "Missing local import lib/missing_a.dart",
                "lib/a.dart",
            ),
            finding(
                FindingKind::UnresolvedDependency,
                "dart-decimate/unresolved-dependency",
                "Missing local import lib/missing_b.dart",
                "lib/b.dart",
            ),
            finding(
                FindingKind::UnusedWidgetParam,
                "dart-decimate/unused-widget-param",
                "Widget constructor parameter is never read",
                "lib/widget.dart",
            ),
        ],
        clone_groups: Vec::new(),
        complexity: Vec::new(),
        file_scores: Vec::new(),
        hotspots: Vec::new(),
        refactoring_targets: Vec::new(),
        threshold_overrides: Vec::new(),
        feature_flags: Vec::new(),
        security_candidates: Vec::new(),
        attack_surface: Vec::new(),
        runtime_coverage: None,
        next_steps: Vec::new(),
    };

    let rendered = render_html_report(&report);

    assert!(rendered.contains("data-finding-search"));
    assert!(!rendered.contains("data-finding-type"));
    assert!(rendered.contains(
        "<button class=\"type-filter\" type=\"button\" data-finding-filter data-kind=\"Unused widget parameter\" aria-pressed=\"false\">2. Unused widget parameter (1)</button>"
    ));
    assert!(!rendered.contains("if (query || selectedType) group.open = true;"));
    assert!(rendered.contains("<details class=\"finding-group\" data-finding-group data-kind=\"Unresolved dependency\" open>"));
    assert!(rendered.contains(
        "<details class=\"finding-group\" data-finding-group data-kind=\"Unused widget parameter\">"
    ));
    assert!(rendered.contains("<span class=\"group-number\">2.</span>"));
    assert!(rendered.contains("<span class=\"chevron\" aria-hidden=\"true\"></span>"));
    assert!(rendered.contains("2 findings | 2 errors"));
    assert!(rendered.contains("1 finding | 1 error"));
}

#[test]
fn group_headers_render_unique_rule_ids_only_when_concise() {
    let report = JsonReport {
        schema_version: "dart-decimate.report.v1".to_owned(),
        kind: "combined".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: ReportCommand::Check,
        verdict: Verdict::Fail,
        summary: ReportSummary {
            files: 6,
            findings: 6,
            policy_violations: 2,
            security_candidates: 4,
            ..ReportSummary::default()
        },
        findings: vec![
            finding(
                FindingKind::PolicyViolation,
                "dart-decimate/policy/mobile/no-dart-io",
                "dart:io is not allowed",
                "lib/io.dart",
            ),
            finding(
                FindingKind::PolicyViolation,
                "dart-decimate/policy/mobile/no-process",
                "Process APIs are not allowed",
                "lib/process.dart",
            ),
            finding(
                FindingKind::SecurityCandidate,
                "dart-decimate/security/hardcoded-secret",
                "Hardcoded secret candidate",
                "lib/secret.dart",
            ),
            finding(
                FindingKind::SecurityCandidate,
                "dart-decimate/security/tls-bypass",
                "TLS bypass candidate",
                "lib/tls.dart",
            ),
            finding(
                FindingKind::SecurityCandidate,
                "dart-decimate/security/raw-sql",
                "Raw SQL candidate",
                "lib/sql.dart",
            ),
            finding(
                FindingKind::SecurityCandidate,
                "dart-decimate/security/webview-javascript",
                "WebView JavaScript bridge candidate",
                "lib/webview.dart",
            ),
        ],
        clone_groups: Vec::new(),
        complexity: Vec::new(),
        file_scores: Vec::new(),
        hotspots: Vec::new(),
        refactoring_targets: Vec::new(),
        threshold_overrides: Vec::new(),
        feature_flags: Vec::new(),
        security_candidates: Vec::new(),
        attack_surface: Vec::new(),
        runtime_coverage: None,
        next_steps: Vec::new(),
    };

    let rendered = render_html_report(&report);

    assert!(rendered.contains(
        "<span class=\"summary-text\"><strong>Policy violation</strong><span class=\"rule\">dart-decimate/policy/mobile/no-dart-io, dart-decimate/policy/mobile/no-process</span></span>"
    ));
    assert!(
        rendered
            .contains("<span class=\"summary-text\"><strong>Security candidate</strong></span>")
    );
    assert!(!rendered.contains(
        "<strong>Security candidate</strong><span class=\"rule\">dart-decimate/security/hardcoded-secret</span>"
    ));
}

#[test]
fn grouped_findings_preserves_first_seen_order_for_repeated_types() {
    let findings = vec![
        finding(
            FindingKind::UnresolvedDependency,
            "dart-decimate/unresolved-dependency",
            "Missing local import lib/missing_a.dart",
            "lib/a.dart",
        ),
        finding(
            FindingKind::UnusedWidgetParam,
            "dart-decimate/unused-widget-param",
            "Widget constructor parameter is never read",
            "lib/widget.dart",
        ),
        finding(
            FindingKind::UnresolvedDependency,
            "dart-decimate/unresolved-dependency",
            "Missing local import lib/missing_b.dart",
            "lib/b.dart",
        ),
        finding(
            FindingKind::PolicyViolation,
            "dart-decimate/policy/mobile/no-dart-io",
            "dart:io is not allowed",
            "lib/io.dart",
        ),
        finding(
            FindingKind::UnusedWidgetParam,
            "dart-decimate/unused-widget-param",
            "Second widget constructor parameter is never read",
            "lib/second_widget.dart",
        ),
    ];

    let groups = grouped_findings(&findings);

    assert_eq!(groups.len(), 3);
    assert_eq!(groups[0].kind, FindingKind::UnresolvedDependency);
    assert_eq!(
        groups[0]
            .entries
            .iter()
            .map(|(index, _)| *index)
            .collect::<Vec<_>>(),
        vec![1, 3]
    );
    assert_eq!(groups[1].kind, FindingKind::UnusedWidgetParam);
    assert_eq!(
        groups[1]
            .entries
            .iter()
            .map(|(index, _)| *index)
            .collect::<Vec<_>>(),
        vec![2, 5]
    );
    assert_eq!(groups[2].kind, FindingKind::PolicyViolation);
    assert_eq!(
        groups[2]
            .entries
            .iter()
            .map(|(index, _)| *index)
            .collect::<Vec<_>>(),
        vec![4]
    );
}

#[test]
fn escapes_control_characters_from_user_values() {
    let report = JsonReport {
        schema_version: "dart-decimate.report.v1".to_owned(),
        kind: "combined".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: ReportCommand::Check,
        verdict: Verdict::Fail,
        summary: ReportSummary {
            files: 2,
            edges: 1,
            findings: 1,
            cycles: 1,
            ..ReportSummary::default()
        },
        findings: vec![Finding {
            rule_id: "dart-decimate/circular-dependency".to_owned(),
            fingerprint: Some("cycle:\x1bunsafe".to_owned()),
            kind: FindingKind::CircularDependency,
            severity: Severity::Error,
            message: "Circular dependency includes \x07bell".to_owned(),
            path: "lib/\x1b[31mbad.dart".to_owned(),
            line: 1,
            column: 0,
            safe_to_delete: false,
            files: vec![
                "lib/\x1b[31mbad.dart".to_owned(),
                "lib/\x07bell.dart".to_owned(),
            ],
            edge: Some(FindingEdge {
                from: "lib/\x1b[31mbad.dart".to_owned(),
                to: "lib/\x07bell.dart".to_owned(),
                specifier: "package:app/\x1bbad.dart".to_owned(),
                kind: "import".to_owned(),
            }),
            actions: vec![
                FindingAction::new("inspect-control", "Inspect \x07bell", false)
                    .with_dart_decimate_args([
                        "inspect",
                        "--format",
                        "json",
                        "--file",
                        "lib/\x1b[31mbad.dart",
                    ]),
            ],
        }],
        clone_groups: Vec::new(),
        complexity: Vec::new(),
        file_scores: Vec::new(),
        hotspots: Vec::new(),
        refactoring_targets: Vec::new(),
        threshold_overrides: Vec::new(),
        feature_flags: Vec::new(),
        security_candidates: Vec::new(),
        attack_surface: Vec::new(),
        runtime_coverage: None,
        next_steps: vec![NextStep {
            id: "inspect-control".to_owned(),
            command: "dart-decimate inspect --file lib/\x1b[31mbad.dart".to_owned(),
            reason: "Review \x07bell evidence".to_owned(),
        }],
    };

    let rendered = render_html_report(&report);

    assert!(!rendered.contains('\x1b'));
    assert!(!rendered.contains('\x07'));
    assert!(rendered.contains("lib/&#x1B;[31mbad.dart"));
    assert!(rendered.contains("&#x7;bell"));
}

#[test]
fn renders_omitted_details_for_summary_only_failures() {
    let mut report = JsonReport {
        schema_version: "dart-decimate.report.v1".to_owned(),
        kind: "combined".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: ReportCommand::Security,
        verdict: Verdict::Fail,
        summary: ReportSummary {
            files: 1,
            findings: 2,
            security_candidates: 2,
            ..ReportSummary::default()
        },
        findings: Vec::new(),
        clone_groups: Vec::new(),
        complexity: Vec::new(),
        file_scores: Vec::new(),
        hotspots: Vec::new(),
        refactoring_targets: Vec::new(),
        threshold_overrides: Vec::new(),
        feature_flags: Vec::new(),
        security_candidates: Vec::new(),
        attack_surface: Vec::new(),
        runtime_coverage: None,
        next_steps: Vec::new(),
    };
    report.findings.clear();

    let rendered = render_html_report(&report);

    assert!(rendered.contains("<h1>security report</h1>"));
    assert!(rendered.contains("2 findings were omitted from this summary output."));
    assert!(!rendered.contains("No findings. The selected Dart graph checks passed."));
}

fn finding(kind: FindingKind, rule_id: &str, message: &str, path: &str) -> Finding {
    Finding {
        rule_id: rule_id.to_owned(),
        fingerprint: None,
        kind,
        severity: Severity::Error,
        message: message.to_owned(),
        path: path.to_owned(),
        line: 1,
        column: 0,
        safe_to_delete: false,
        files: Vec::new(),
        edge: None,
        actions: Vec::new(),
    }
}
