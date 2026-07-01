use std::fmt::Write as _;
use std::io::IsTerminal as _;

use super::{
    human_details::{
        best_text, fallback_best, kind_label, omitted_findings_message, summary_groups, why_text,
    },
    types::{Finding, FindingEdge, FindingKind, JsonReport, ReportSummary, Severity, Verdict},
};

const MAX_RELATED_FILES: usize = 8;

/// Render a concise human report.
#[must_use]
pub fn render_human_report(report: &JsonReport) -> String {
    render_human_report_with_style(report, Style::from_env())
}

#[derive(Clone, Copy)]
struct Style {
    color: bool,
}

impl Style {
    const fn plain() -> Self {
        Self { color: false }
    }

    const fn colored() -> Self {
        Self { color: true }
    }

    fn from_env() -> Self {
        if std::env::var_os("NO_COLOR").is_some() {
            return Self::plain();
        }
        if !std::io::stdout().is_terminal() {
            return Self::plain();
        }
        match std::env::var("TERM") {
            Ok(term) if term.eq_ignore_ascii_case("dumb") => Self::plain(),
            _ => Self::colored(),
        }
    }

    #[must_use]
    fn paint(self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_owned()
        }
    }

    #[must_use]
    fn bold(self, text: &str) -> String {
        self.paint("1", text)
    }

    #[must_use]
    fn dim(self, text: &str) -> String {
        self.paint("2", text)
    }

    #[must_use]
    fn cyan(self, text: &str) -> String {
        self.paint("36", text)
    }

    #[must_use]
    fn green(self, text: &str) -> String {
        self.paint("1;32", text)
    }

    #[must_use]
    fn red(self, text: &str) -> String {
        self.paint("1;31", text)
    }
}

fn render_human_report_with_style(report: &JsonReport, style: Style) -> String {
    let mut rendered = String::new();
    render_header(&mut rendered, report, style);
    render_issue_summary(&mut rendered, &report.summary, style);

    if report.findings.is_empty() {
        if report.summary.findings == 0 && report.verdict == Verdict::Pass {
            let _ = writeln!(
                rendered,
                "\n{}",
                style.green("No findings. The selected Dart graph checks passed.")
            );
        } else {
            let message = omitted_findings_message(&report.summary, report.verdict);
            let message = if report.verdict == Verdict::Fail {
                style.red(&message)
            } else {
                style.cyan(&message)
            };
            let _ = writeln!(rendered, "\n{message}");
        }
        return rendered;
    }

    let _ = writeln!(rendered, "\n{}", style.bold("Findings"));
    for (index, finding) in report.findings.iter().enumerate() {
        render_finding(&mut rendered, index + 1, finding, style);
    }

    if !report.next_steps.is_empty() {
        let _ = writeln!(rendered, "\n{}", style.bold("Next Steps"));
        for (index, step) in report.next_steps.iter().enumerate() {
            let command = terminal_text(&step.command);
            let reason = terminal_text(&step.reason);
            let _ = writeln!(
                rendered,
                "  {}. {}",
                index + 1,
                style.cyan(command.as_str())
            );
            let _ = writeln!(rendered, "     Why: {reason}");
        }
    }

    rendered
}

fn render_header(rendered: &mut String, report: &JsonReport, style: Style) {
    let verdict = match report.verdict {
        Verdict::Pass => style.green("PASS"),
        Verdict::Fail => style.red("FAIL"),
    };
    let _ = writeln!(
        rendered,
        "{} {}: {verdict}",
        style.bold("Dart Decimate"),
        report.command.as_str()
    );
    let _ = writeln!(
        rendered,
        "Files: {} | Edges: {} | Findings: {}",
        style.cyan(&report.summary.files.to_string()),
        style.cyan(&report.summary.edges.to_string()),
        style.cyan(&report.summary.findings.to_string())
    );
    if report.summary.functions > 0 || report.summary.quality_score > 0 {
        let _ = writeln!(
            rendered,
            "Quality: {}/100 | Functions: {} | Max cyclomatic: {} | Max cognitive: {}",
            report.summary.quality_score,
            report.summary.functions,
            report.summary.max_cyclomatic_complexity,
            report.summary.max_cognitive_complexity
        );
    }
}

fn render_issue_summary(rendered: &mut String, summary: &ReportSummary, style: Style) {
    let groups = summary_groups(summary);
    let visible = groups
        .iter()
        .filter(|(_, items)| !items.is_empty())
        .collect::<Vec<_>>();

    if visible.is_empty() {
        return;
    }

    let _ = writeln!(rendered, "\n{}", style.bold("Issue Summary"));
    for (name, items) in visible {
        let _ = writeln!(rendered, "  {}: {}", style.cyan(name), items.join(", "));
    }
}

fn render_finding(rendered: &mut String, index: usize, finding: &Finding, style: Style) {
    let severity = severity_label(finding.severity);
    let severity = style.paint(severity_color(finding.severity), severity);
    let _ = writeln!(
        rendered,
        "\n{}. [{}] {} at {}",
        index,
        severity,
        style.bold(kind_label(finding.kind)),
        location(finding)
    );
    let rule_id = terminal_text(&finding.rule_id);
    let _ = writeln!(rendered, "   Rule: {}", style.dim(&rule_id));
    let _ = writeln!(rendered, "   What: {}", terminal_text(&finding.message));
    let _ = writeln!(rendered, "   Why: {}", why_text(finding.kind));
    render_evidence(rendered, finding, style);
    render_best_action(rendered, finding, style);
}

fn render_evidence(rendered: &mut String, finding: &Finding, style: Style) {
    let _ = writeln!(rendered, "   Evidence:");
    let _ = writeln!(rendered, "     Location: {}", location(finding));
    if let Some(fingerprint) = &finding.fingerprint {
        let _ = writeln!(rendered, "     Fingerprint: {}", terminal_text(fingerprint));
    }
    if let Some(edge) = &finding.edge {
        render_edge(rendered, edge);
    }
    if !finding.files.is_empty() {
        render_related_files(rendered, finding, style);
    }
    let safe = if finding.safe_to_delete {
        "yes, after reviewing generated/dynamic usage"
    } else {
        "no; review or refactor before deleting"
    };
    let _ = writeln!(rendered, "     Safe to delete: {safe}");
}

fn render_edge(rendered: &mut String, edge: &FindingEdge) {
    let _ = writeln!(
        rendered,
        "     Edge: {} {} -> {} ({})",
        terminal_text(&edge.kind),
        terminal_text(&edge.from),
        terminal_text(&edge.to),
        terminal_text(&edge.specifier)
    );
}

fn render_related_files(rendered: &mut String, finding: &Finding, style: Style) {
    if matches!(
        finding.kind,
        FindingKind::CircularDependency | FindingKind::ReExportCycle
    ) {
        let _ = writeln!(
            rendered,
            "     Cycle preview: {}",
            style.cyan(&cycle_preview(&finding.files))
        );
        let _ = writeln!(
            rendered,
            "     Files in cycle: {} total",
            finding.files.len()
        );
    } else {
        let _ = writeln!(
            rendered,
            "     Related files: {}",
            related_files_preview(&finding.files)
        );
    }
}

fn render_best_action(rendered: &mut String, finding: &Finding, style: Style) {
    let best = finding.actions.first().map_or_else(
        || fallback_best(finding.kind),
        |action| best_text(finding, action),
    );
    let _ = writeln!(rendered, "   Best: {}", terminal_text(&best));

    if let Some(command) = finding
        .actions
        .iter()
        .find_map(|action| action.command.as_ref())
    {
        let command = terminal_text(command);
        let _ = writeln!(rendered, "   Inspect: {}", style.cyan(&command));
    }
}

#[must_use]
fn location(finding: &Finding) -> String {
    let path = if finding.path.is_empty() {
        "<project>"
    } else {
        finding.path.as_str()
    };
    let path = terminal_text(path);
    format!("{path}:{}:{}", finding.line, finding.column)
}

fn terminal_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .collect()
}

#[must_use]
const fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warn",
    }
}

#[must_use]
const fn severity_color(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "1;31",
        Severity::Warning => "1;33",
    }
}

#[must_use]
fn cycle_preview(files: &[String]) -> String {
    let mut visible = files
        .iter()
        .take(MAX_RELATED_FILES)
        .map(|file| terminal_text(file))
        .collect::<Vec<_>>();
    if files.len() > MAX_RELATED_FILES {
        visible.push(format!(
            "... {} more files ...",
            files.len() - MAX_RELATED_FILES
        ));
    }
    if let Some(first) = files.first() {
        visible.push(terminal_text(first));
    }
    visible.join(" -> ")
}

#[must_use]
fn related_files_preview(files: &[String]) -> String {
    let mut visible = files
        .iter()
        .take(MAX_RELATED_FILES)
        .map(|file| terminal_text(file))
        .collect::<Vec<_>>();
    if files.len() > MAX_RELATED_FILES {
        visible.push(format!(
            "... {} more files",
            files.len() - MAX_RELATED_FILES
        ));
    }
    visible.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{FindingAction, ReportCommand};

    #[test]
    fn explains_large_cycles_with_truncated_evidence() {
        let report = report_with_finding(cycle_finding(12));

        let rendered = render_human_report_with_style(&report, Style::plain());

        assert!(rendered.contains("Dart Decimate check: FAIL"));
        assert!(rendered.contains("Architecture: 1 circular dependency"));
        assert!(rendered.contains("[error] Circular dependency at lib/file_0.dart:1:0"));
        assert!(rendered.contains("What: Circular dependency spans 12 Dart files"));
        assert!(rendered.contains("Why: These files import or export each other in a loop."));
        assert!(rendered.contains("Evidence:"));
        assert!(rendered.contains("Cycle preview: lib/file_0.dart -> lib/file_1.dart"));
        assert!(rendered.contains("... 4 more files ... -> lib/file_0.dart"));
        assert!(rendered.contains("Files in cycle: 12 total"));
        assert!(rendered.contains("Best: Break one import/export edge first."));
        assert!(
            rendered
                .contains("Inspect: dart-decimate inspect --format json --file lib/file_0.dart")
        );
    }

    #[test]
    fn renders_color_when_enabled() {
        let report = report_with_finding(cycle_finding(2));

        let rendered = render_human_report_with_style(&report, Style::colored());

        assert!(rendered.contains("\x1b[1;31mFAIL\x1b[0m"));
        assert!(rendered.contains("\x1b[1;31merror\x1b[0m"));
        assert!(
            rendered
                .contains("\x1b[36mlib/file_0.dart -> lib/file_1.dart -> lib/file_0.dart\x1b[0m")
        );
    }

    #[test]
    fn explains_safe_delete_actions_and_next_steps() {
        let mut report = report_with_finding(Finding {
            rule_id: "dart-decimate/dead-file".to_owned(),
            fingerprint: None,
            kind: FindingKind::DeadFile,
            severity: Severity::Error,
            message: "Dart file is unreachable from the configured entry points: lib/dead.dart"
                .to_owned(),
            path: "lib/dead.dart".to_owned(),
            line: 1,
            column: 0,
            safe_to_delete: true,
            files: Vec::new(),
            edge: None,
            actions: vec![FindingAction::new(
                "delete-file",
                "Delete the unreachable Dart file after confirming no dynamic entry point uses it",
                true,
            )
            .with_dart_decimate_args([
                "inspect",
                "--format",
                "json",
                "--file",
                "lib/dead.dart",
            ])],
        });
        report.next_steps.push(crate::output::NextStep {
            id: "trace-dead-file".to_owned(),
            command: "dart-decimate inspect --format json --file lib/dead.dart".to_owned(),
            reason: "Collect references before deleting the file".to_owned(),
        });

        let rendered = render_human_report_with_style(&report, Style::plain());

        assert!(rendered.contains("Cleanup: 1 dead file"));
        assert!(rendered.contains("Safe to delete: yes, after reviewing generated/dynamic usage"));
        assert!(rendered.contains("Best: Delete the unreachable Dart file"));
        assert!(rendered.contains("Next Steps"));
        assert!(rendered.contains("Why: Collect references before deleting the file"));
    }

    #[test]
    fn strips_control_characters_from_terminal_paths() {
        let mut finding = cycle_finding(2);
        finding.path = "lib/\x1b[31mbad.dart".to_owned();
        finding.message = "Circular dependency includes lib/\x1b[31mbad.dart".to_owned();
        finding.files = vec![
            "lib/\x1b[31mbad.dart".to_owned(),
            "lib/live.dart".to_owned(),
        ];
        let report = report_with_finding(finding);

        let rendered = render_human_report_with_style(&report, Style::plain());

        assert!(
            !rendered
                .chars()
                .any(|character| { character.is_control() && character != '\n' })
        );
        assert!(!rendered.contains('\x1b'));
    }

    #[test]
    fn renders_omitted_details_for_summary_only_failures() {
        let mut report = report_with_finding(cycle_finding(2));
        report.findings.clear();

        let rendered = render_human_report_with_style(&report, Style::plain());

        assert!(rendered.contains("Dart Decimate check: FAIL"));
        assert!(rendered.contains("Findings: 1"));
        assert!(rendered.contains("1 finding was omitted from this summary output."));
        assert!(!rendered.contains("No findings. The selected Dart graph checks passed."));
    }

    fn report_with_finding(finding: Finding) -> JsonReport {
        let mut summary = ReportSummary {
            files: finding.files.len().max(1),
            edges: finding.files.len(),
            findings: 1,
            ..ReportSummary::default()
        };
        match finding.kind {
            FindingKind::CircularDependency => summary.cycles = 1,
            FindingKind::DeadFile => summary.dead_files = 1,
            _ => {}
        }
        JsonReport {
            schema_version: "dart-decimate.report.v1".to_owned(),
            kind: "combined".to_owned(),
            tool: "dart-decimate".to_owned(),
            command: ReportCommand::Check,
            verdict: Verdict::Fail,
            summary,
            findings: vec![finding],
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
        }
    }

    fn cycle_finding(count: usize) -> Finding {
        let files = (0..count)
            .map(|index| format!("lib/file_{index}.dart"))
            .collect::<Vec<_>>();
        let path = files[0].clone();
        Finding {
            rule_id: "dart-decimate/circular-dependency".to_owned(),
            fingerprint: None,
            kind: FindingKind::CircularDependency,
            severity: Severity::Error,
            message: format!("Circular dependency spans {count} Dart files"),
            path: path.clone(),
            line: 1,
            column: 0,
            safe_to_delete: false,
            files,
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
                    path.as_str(),
                ]),
            ],
        }
    }
}
