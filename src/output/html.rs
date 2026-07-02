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

mod assets;

use assets::{render_interaction_script, render_style};

const MAX_RELATED_FILES: usize = 12;
const MAX_GROUP_RULE_IDS: usize = 3;

/// Render a browser-ready static report with grouped finding navigation.
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

    let groups = grouped_findings(&report.findings);
    render_finding_controls(html, &groups);
    let _ = writeln!(
        html,
        "<p class=\"empty-results\" data-empty-results hidden>No findings match the current filters.</p>"
    );
    let _ = writeln!(html, "<div class=\"finding-groups\">");
    for (group_index, group) in groups.iter().enumerate() {
        render_finding_group(html, group_index, group);
    }
    let _ = writeln!(html, "</div>");
    let _ = writeln!(html, "</section>");
}

struct FindingGroup<'a> {
    kind: FindingKind,
    rule_ids: Vec<&'a str>,
    entries: Vec<(usize, &'a Finding)>,
    error_count: usize,
    warning_count: usize,
}

fn grouped_findings(findings: &[Finding]) -> Vec<FindingGroup<'_>> {
    let mut groups: Vec<FindingGroup<'_>> = Vec::new();
    for (index, finding) in findings.iter().enumerate() {
        if let Some(group) = groups.iter_mut().find(|group| group.kind == finding.kind) {
            group.push(index + 1, finding);
        } else {
            groups.push(FindingGroup::new(index + 1, finding));
        }
    }
    groups
}

impl<'a> FindingGroup<'a> {
    fn new(index: usize, finding: &'a Finding) -> Self {
        let mut group = Self {
            kind: finding.kind,
            rule_ids: Vec::new(),
            entries: Vec::new(),
            error_count: 0,
            warning_count: 0,
        };
        group.push(index, finding);
        group
    }

    fn push(&mut self, index: usize, finding: &'a Finding) {
        match finding.severity {
            Severity::Error => self.error_count += 1,
            Severity::Warning => self.warning_count += 1,
        }
        let rule_id = finding.rule_id.as_str();
        if !self.rule_ids.contains(&rule_id) {
            self.rule_ids.push(rule_id);
        }
        self.entries.push((index, finding));
    }
}

fn render_finding_controls(html: &mut String, groups: &[FindingGroup<'_>]) {
    let total = groups
        .iter()
        .map(|group| group.entries.len())
        .sum::<usize>();
    let _ = writeln!(html, "<div class=\"finding-tools\" data-finding-controls>");
    let _ = writeln!(
        html,
        "<label>Search<input type=\"search\" placeholder=\"Path, message, rule, evidence\" data-finding-search></label>"
    );
    let _ = writeln!(
        html,
        "<div class=\"filter-set\" role=\"group\" aria-label=\"Filter findings by type\">"
    );
    let _ = writeln!(html, "<span class=\"filter-label\">Type</span>");
    let _ = writeln!(html, "<div class=\"filter-buttons\">");
    let _ = writeln!(
        html,
        "<button class=\"type-filter\" type=\"button\" data-finding-filter data-kind=\"\" aria-pressed=\"true\">All types</button>"
    );
    for (index, group) in groups.iter().enumerate() {
        let label = kind_label(group.kind);
        let _ = writeln!(
            html,
            "<button class=\"type-filter\" type=\"button\" data-finding-filter data-kind=\"{}\" aria-pressed=\"false\">{}. {} ({})</button>",
            escape(label),
            index + 1,
            escape(label),
            group.entries.len()
        );
    }
    let _ = writeln!(html, "</div></div>");
    let _ = writeln!(
        html,
        "<p class=\"finding-status\" data-finding-status data-total=\"{total}\" aria-live=\"polite\">Showing {total} of {total} findings in {} {}.</p>",
        groups.len(),
        plural(groups.len(), "type", "types")
    );
    let _ = writeln!(html, "</div>");
}

fn render_finding_group(html: &mut String, group_index: usize, group: &FindingGroup<'_>) {
    let label = kind_label(group.kind);
    let open = if group_index == 0 { " open" } else { "" };
    let _ = writeln!(
        html,
        "<details class=\"finding-group\" data-finding-group data-kind=\"{}\"{}>",
        escape(label),
        open
    );
    let _ = writeln!(html, "<summary>");
    let rule_summary = group_rule_summary(group);
    let rule_html = rule_summary.map_or_else(String::new, |rule_ids| {
        format!("<span class=\"rule\">{}</span>", escape(&rule_ids))
    });
    let _ = writeln!(
        html,
        "<span class=\"summary-title\"><span class=\"group-number\">{}.</span><span class=\"summary-text\"><strong>{}</strong>{}</span></span>",
        group_index + 1,
        escape(label),
        rule_html
    );
    let _ = writeln!(
        html,
        "<span class=\"summary-right\"><span class=\"summary-meta\">{}</span><span class=\"chevron\" aria-hidden=\"true\"></span></span>",
        escape(&group_summary(group))
    );
    let _ = writeln!(html, "</summary>");
    let _ = writeln!(html, "<div class=\"findings\">");
    for (index, finding) in &group.entries {
        render_finding(html, *index, finding);
    }
    let _ = writeln!(html, "</div>");
    let _ = writeln!(html, "</details>");
}

fn group_rule_summary(group: &FindingGroup<'_>) -> Option<String> {
    if group.rule_ids.is_empty() || group.rule_ids.len() > MAX_GROUP_RULE_IDS {
        return None;
    }
    Some(group.rule_ids.join(", "))
}

fn group_summary(group: &FindingGroup<'_>) -> String {
    let total = group.entries.len();
    let mut parts = vec![format!("{total} {}", plural(total, "finding", "findings"))];
    if group.error_count > 0 {
        parts.push(format!(
            "{} {}",
            group.error_count,
            plural(group.error_count, "error", "errors")
        ));
    }
    if group.warning_count > 0 {
        parts.push(format!(
            "{} {}",
            group.warning_count,
            plural(group.warning_count, "warning", "warnings")
        ));
    }
    parts.join(" | ")
}

fn render_finding(html: &mut String, index: usize, finding: &Finding) {
    let severity_class = severity_class(finding.severity);
    let _ = writeln!(html, "<article class=\"finding\" data-finding>");
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
    render_interaction_script(html);
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

const fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
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
mod tests;
