use std::fmt::Write as _;

use crate::decision_surface::{
    DecisionSurfaceCategory, DecisionSurfaceDecision, DecisionSurfaceReport,
};

use super::{
    human_details::{
        best_text, fallback_best, kind_label, omitted_findings_message, summary_groups, why_text,
    },
    types::{Finding, FindingEdge, FindingKind, JsonReport, Severity, Verdict},
};

const MAX_RELATED_FILES: usize = 12;

/// Render a browser-ready static report for humans.
#[must_use]
pub fn render_html_report(report: &JsonReport) -> String {
    let mut html = String::new();
    render_document_start(&mut html, report);
    render_summary(&mut html, report);
    render_findings(&mut html, report);
    render_next_steps(&mut html, report);
    render_document_end(&mut html);
    html
}

/// Render a browser-ready static decision-surface report for humans.
#[must_use]
pub fn render_decision_surface_html_report(report: &DecisionSurfaceReport) -> String {
    let mut html = String::new();
    render_decision_surface_document_start(&mut html, report);
    render_decision_surface_summary(&mut html, report);
    render_decision_surface_decisions(&mut html, report);
    render_document_end(&mut html);
    html
}

fn render_document_start(html: &mut String, report: &JsonReport) {
    let _ = writeln!(html, "<!doctype html>");
    let _ = writeln!(html, "<html lang=\"en\">");
    let _ = writeln!(html, "<head>");
    let _ = writeln!(html, "<meta charset=\"utf-8\">");
    let _ = writeln!(
        html,
        "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">"
    );
    let _ = writeln!(
        html,
        "<title>Dart Decimate {} report</title>",
        escape(report.command.as_str())
    );
    render_style(html);
    let _ = writeln!(html, "</head>");
    let _ = writeln!(html, "<body>");
}

fn render_decision_surface_document_start(html: &mut String, report: &DecisionSurfaceReport) {
    let _ = writeln!(html, "<!doctype html>");
    let _ = writeln!(html, "<html lang=\"en\">");
    let _ = writeln!(html, "<head>");
    let _ = writeln!(html, "<meta charset=\"utf-8\">");
    let _ = writeln!(
        html,
        "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">"
    );
    let _ = writeln!(
        html,
        "<title>Dart Decimate {} decision surface</title>",
        escape(report.command.as_str())
    );
    render_style(html);
    let _ = writeln!(html, "</head>");
    let _ = writeln!(html, "<body>");
}

fn render_style(html: &mut String) {
    let _ = writeln!(
        html,
        "<style>
:root {{
  color-scheme: dark;
  --bg: #070707;
  --panel: #111111;
  --line: #2b2b2b;
  --text: #f1f1f1;
  --muted: #a6a6a6;
  --accent: #8fdcff;
  --error: #ff5c5c;
  --warn: #ffd166;
  --pass: #5ee091;
}}
* {{ box-sizing: border-box; }}
body {{
  margin: 0;
  background: var(--bg);
  color: var(--text);
  font: 15px/1.5 ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, \"Segoe UI\", sans-serif;
}}
main {{ max-width: 1180px; margin: 0 auto; padding: 40px 24px 56px; }}
h1, h2, h3, p {{ margin: 0; }}
h1 {{ font-size: clamp(32px, 5vw, 60px); letter-spacing: 0; line-height: 1; }}
h2 {{ margin-top: 36px; font-size: 22px; }}
.topline {{ color: var(--muted); margin-bottom: 10px; text-transform: uppercase; letter-spacing: .08em; font-size: 12px; }}
.hero {{ border-bottom: 1px solid var(--line); padding-bottom: 28px; }}
.verdict {{ display: inline-flex; align-items: center; gap: 8px; margin-top: 18px; padding: 7px 10px; border: 1px solid var(--line); border-radius: 6px; }}
.pass {{ color: var(--pass); }}
.fail, .error {{ color: var(--error); }}
.warning {{ color: var(--warn); }}
.metrics, .groups {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(190px, 1fr)); gap: 12px; margin-top: 24px; }}
.metric, .group, .finding, .step {{ background: var(--panel); border: 1px solid var(--line); border-radius: 8px; padding: 16px; }}
.metric span, .group span, .label {{ color: var(--muted); display: block; font-size: 12px; text-transform: uppercase; letter-spacing: .06em; }}
.metric strong, .group strong {{ display: block; margin-top: 4px; font-size: 24px; }}
.group p {{ margin-top: 8px; color: var(--text); }}
.findings {{ display: grid; gap: 14px; margin-top: 14px; }}
.finding header {{ display: flex; flex-wrap: wrap; align-items: baseline; justify-content: space-between; gap: 10px; margin-bottom: 14px; }}
.finding h3 {{ font-size: 18px; }}
.rule, .location, .mono {{ font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; }}
.rule {{ color: var(--muted); font-size: 13px; }}
.section {{ margin-top: 12px; }}
.section p {{ margin-top: 3px; }}
.evidence {{ margin: 8px 0 0; padding-left: 18px; color: var(--text); }}
.evidence li {{ margin: 3px 0; }}
.command {{ overflow-wrap: anywhere; color: var(--accent); }}
a {{ color: var(--accent); text-decoration: none; }}
a:hover {{ text-decoration: underline; }}
</style>"
    );
}

fn render_summary(html: &mut String, report: &JsonReport) {
    let verdict_class = match report.verdict {
        Verdict::Pass => "pass",
        Verdict::Fail => "fail",
    };
    let verdict = match report.verdict {
        Verdict::Pass => "PASS",
        Verdict::Fail => "FAIL",
    };

    let _ = writeln!(html, "<main>");
    let _ = writeln!(html, "<section class=\"hero\">");
    let _ = writeln!(html, "<p class=\"topline\">Dart Decimate</p>");
    let _ = writeln!(html, "<h1>{} report</h1>", escape(report.command.as_str()));
    let _ = writeln!(
        html,
        "<p class=\"verdict {verdict_class}\">Verdict: <strong>{verdict}</strong></p>"
    );
    let _ = writeln!(html, "<div class=\"metrics\">");
    metric(html, "Files", report.summary.files);
    metric(html, "Edges", report.summary.edges);
    metric(html, "Findings", report.summary.findings);
    if report.summary.functions > 0 || report.summary.quality_score > 0 {
        metric(html, "Quality", report.summary.quality_score);
        metric(html, "Functions", report.summary.functions);
        metric(
            html,
            "Max cyclomatic",
            report.summary.max_cyclomatic_complexity,
        );
    }
    let _ = writeln!(html, "</div>");
    let _ = writeln!(html, "</section>");

    let groups = summary_groups(&report.summary);
    if groups.iter().any(|(_, items)| !items.is_empty()) {
        let _ = writeln!(html, "<section>");
        let _ = writeln!(html, "<h2>Issue Summary</h2>");
        let _ = writeln!(html, "<div class=\"groups\">");
        for (name, items) in groups.iter().filter(|(_, items)| !items.is_empty()) {
            let _ = writeln!(
                html,
                "<article class=\"group\"><span>{}</span><p>{}</p></article>",
                escape(name),
                escape(&items.join(", "))
            );
        }
        let _ = writeln!(html, "</div>");
        let _ = writeln!(html, "</section>");
    }
}

fn render_decision_surface_summary(html: &mut String, report: &DecisionSurfaceReport) {
    let _ = writeln!(html, "<main>");
    let _ = writeln!(html, "<section class=\"hero\">");
    let _ = writeln!(html, "<p class=\"topline\">Dart Decimate</p>");
    let _ = writeln!(
        html,
        "<h1>{} decision surface</h1>",
        escape(report.command.as_str())
    );
    let _ = writeln!(html, "<div class=\"metrics\">");
    metric_text(html, "Base", &report.base);
    metric(html, "Changed files", report.summary.changed_files);
    metric(html, "Decisions", report.summary.decisions);
    let _ = writeln!(html, "</div>");
    let _ = writeln!(html, "</section>");
}

fn render_decision_surface_decisions(html: &mut String, report: &DecisionSurfaceReport) {
    let _ = writeln!(html, "<section>");
    let _ = writeln!(html, "<h2>Decisions</h2>");
    if report.decisions.is_empty() {
        let _ = writeln!(
            html,
            "<article class=\"finding\"><p>No structural decisions surfaced for the selected changes.</p></article>"
        );
        let _ = writeln!(html, "</section>");
        return;
    }

    let _ = writeln!(html, "<div class=\"findings\">");
    for (index, decision) in report.decisions.iter().enumerate() {
        render_decision_surface_decision(html, index + 1, decision);
    }
    let _ = writeln!(html, "</div>");
    let _ = writeln!(html, "</section>");
}

fn render_decision_surface_decision(
    html: &mut String,
    index: usize,
    decision: &DecisionSurfaceDecision,
) {
    let _ = writeln!(html, "<article class=\"finding\">");
    let _ = writeln!(html, "<header>");
    let _ = writeln!(
        html,
        "<div><h3>{}. {}</h3><p class=\"rule\">{}</p></div>",
        index,
        escape(&decision.question),
        decision_category_value(decision.category)
    );
    let _ = writeln!(html, "<p class=\"location\">{}</p>", escape(&decision.path));
    let _ = writeln!(html, "</header>");
    section(html, "Expert", &decision.recommended_expert, "");
    render_string_list(html, "Evidence", &decision.evidence);
    render_string_list(html, "Files", &decision.files);
    for command in &decision.suggested_commands {
        section(html, "Command", command, "command mono");
    }
    let _ = writeln!(html, "</article>");
}

fn render_string_list(html: &mut String, label: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    let _ = writeln!(
        html,
        "<div class=\"section\"><span class=\"label\">{}</span><ul class=\"evidence\">",
        escape(label)
    );
    for value in values {
        let _ = writeln!(html, "<li>{}</li>", escape(value));
    }
    let _ = writeln!(html, "</ul></div>");
}

const fn decision_category_value(category: DecisionSurfaceCategory) -> &'static str {
    match category {
        DecisionSurfaceCategory::CouplingBoundary => "coupling-boundary",
        DecisionSurfaceCategory::PublicApiContract => "public-api-contract",
        DecisionSurfaceCategory::Dependency => "dependency",
    }
}

fn render_findings(html: &mut String, report: &JsonReport) {
    let _ = writeln!(html, "<section>");
    let _ = writeln!(html, "<h2>Findings</h2>");
    if report.findings.is_empty() {
        if report.summary.findings == 0 && report.verdict == Verdict::Pass {
            let _ = writeln!(
                html,
                "<article class=\"finding\"><p>No findings. The selected Dart graph checks passed.</p></article>"
            );
        } else {
            let message = omitted_findings_message(&report.summary, report.verdict);
            let _ = writeln!(
                html,
                "<article class=\"finding\"><p>{}</p></article>",
                escape(&message)
            );
        }
        let _ = writeln!(html, "</section>");
        return;
    }

    let _ = writeln!(html, "<div class=\"findings\">");
    for (index, finding) in report.findings.iter().enumerate() {
        render_finding(html, index + 1, finding);
    }
    let _ = writeln!(html, "</div>");
    let _ = writeln!(html, "</section>");
}

fn render_finding(html: &mut String, index: usize, finding: &Finding) {
    let severity_class = severity_class(finding.severity);
    let _ = writeln!(html, "<article class=\"finding\">");
    let _ = writeln!(html, "<header>");
    let _ = writeln!(
        html,
        "<div><h3>{}. {}</h3><p class=\"rule\">{}</p></div>",
        index,
        escape(kind_label(finding.kind)),
        escape(&finding.rule_id)
    );
    let _ = writeln!(
        html,
        "<p class=\"{severity_class}\">{}</p>",
        escape(severity_label(finding.severity))
    );
    let _ = writeln!(html, "</header>");
    section(html, "Location", &location(finding), "location");
    section(html, "What", &finding.message, "");
    section(html, "Why", why_text(finding.kind), "");
    render_evidence(html, finding);
    section(html, "Best", &best_for(finding), "");
    if let Some(command) = finding
        .actions
        .iter()
        .find_map(|action| action.command.as_ref())
    {
        section(html, "Inspect", command, "command mono");
    }
    let _ = writeln!(html, "</article>");
}

fn render_evidence(html: &mut String, finding: &Finding) {
    let _ = writeln!(
        html,
        "<div class=\"section\"><span class=\"label\">Evidence</span><ul class=\"evidence\">"
    );
    let _ = writeln!(html, "<li>Location: {}</li>", escape(&location(finding)));
    if let Some(fingerprint) = &finding.fingerprint {
        let _ = writeln!(html, "<li>Fingerprint: {}</li>", escape(fingerprint));
    }
    if let Some(edge) = &finding.edge {
        render_edge(html, edge);
    }
    if !finding.files.is_empty() {
        let label = if matches!(
            finding.kind,
            FindingKind::CircularDependency | FindingKind::ReExportCycle
        ) {
            "Cycle preview"
        } else {
            "Related files"
        };
        let value = if matches!(
            finding.kind,
            FindingKind::CircularDependency | FindingKind::ReExportCycle
        ) {
            cycle_preview(&finding.files)
        } else {
            related_files_preview(&finding.files)
        };
        let _ = writeln!(html, "<li>{label}: {}</li>", escape(&value));
        let _ = writeln!(html, "<li>Files: {} total</li>", finding.files.len());
    }
    let safe = if finding.safe_to_delete {
        "yes, after reviewing generated/dynamic usage"
    } else {
        "no; review or refactor before deleting"
    };
    let _ = writeln!(html, "<li>Safe to delete: {safe}</li>");
    let _ = writeln!(html, "</ul></div>");
}

fn render_edge(html: &mut String, edge: &FindingEdge) {
    let text = format!(
        "{} {} -> {} ({})",
        edge.kind, edge.from, edge.to, edge.specifier
    );
    let _ = writeln!(html, "<li>Edge: {}</li>", escape(&text));
}

fn render_next_steps(html: &mut String, report: &JsonReport) {
    if report.next_steps.is_empty() {
        return;
    }
    let _ = writeln!(html, "<section>");
    let _ = writeln!(html, "<h2>Next Steps</h2>");
    for step in &report.next_steps {
        let _ = writeln!(
            html,
            "<article class=\"step\"><p class=\"command mono\">{}</p><p>{}</p></article>",
            escape(&step.command),
            escape(&step.reason)
        );
    }
    let _ = writeln!(html, "</section>");
}

fn render_document_end(html: &mut String) {
    let _ = writeln!(html, "</main>");
    let _ = writeln!(html, "</body>");
    let _ = writeln!(html, "</html>");
}

fn metric(html: &mut String, label: &str, value: usize) {
    metric_text(html, label, &value.to_string());
}

fn metric_text(html: &mut String, label: &str, value: &str) {
    let _ = writeln!(
        html,
        "<div class=\"metric\"><span>{}</span><strong>{}</strong></div>",
        escape(label),
        escape(value)
    );
}

fn section(html: &mut String, label: &str, value: &str, class_name: &str) {
    let class_attr = if class_name.is_empty() {
        String::new()
    } else {
        format!(" class=\"{class_name}\"")
    };
    let _ = writeln!(
        html,
        "<div class=\"section\"><span class=\"label\">{}</span><p{}>{}</p></div>",
        escape(label),
        class_attr,
        escape(value)
    );
}

fn best_for(finding: &Finding) -> String {
    finding.actions.first().map_or_else(
        || fallback_best(finding.kind),
        |action| best_text(finding, action),
    )
}

fn location(finding: &Finding) -> String {
    let path = if finding.path.is_empty() {
        "<project>"
    } else {
        finding.path.as_str()
    };
    format!("{path}:{}:{}", finding.line, finding.column)
}

const fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

const fn severity_class(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

fn cycle_preview(files: &[String]) -> String {
    let mut visible = files
        .iter()
        .take(MAX_RELATED_FILES)
        .cloned()
        .collect::<Vec<_>>();
    if files.len() > MAX_RELATED_FILES {
        visible.push(format!(
            "... {} more files ...",
            files.len() - MAX_RELATED_FILES
        ));
    }
    if let Some(first) = files.first() {
        visible.push(first.clone());
    }
    visible.join(" -> ")
}

fn related_files_preview(files: &[String]) -> String {
    let mut visible = files
        .iter()
        .take(MAX_RELATED_FILES)
        .cloned()
        .collect::<Vec<_>>();
    if files.len() > MAX_RELATED_FILES {
        visible.push(format!(
            "... {} more files",
            files.len() - MAX_RELATED_FILES
        ));
    }
    visible.join(", ")
}

fn escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            character if character.is_control() => {
                let _ = write!(escaped, "&#x{:X};", character as u32);
            }
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
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
}
