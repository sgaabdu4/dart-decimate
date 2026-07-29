use std::fmt::Write as _;

use super::{DependencyTraceReport, FileTraceReport, SymbolTraceReport};

/// Render a concise human file trace.
#[must_use]
pub fn render_file_trace(report: &FileTraceReport) -> String {
    let mut rendered = String::new();
    let _ = writeln!(
        rendered,
        "trace-file {}: found={} reachable={} entry_point={}",
        report.path, report.found, report.reachable, report.entry_point
    );
    let _ = writeln!(rendered, "{}", report.reason);
    rendered
}

/// Render a concise human symbol trace.
#[must_use]
pub fn render_symbol_trace(report: &SymbolTraceReport) -> String {
    let mut rendered = String::new();
    let _ = writeln!(
        rendered,
        "trace-symbol {}:{}: found={} reachable_file={} references={}",
        report.path,
        report.symbol,
        report.found,
        report.reachable_file,
        report.direct_references.len()
    );
    let _ = writeln!(
        rendered,
        "semantic={} completeness={} impact_paths={} suggested_tests={}",
        report.semantic_decision.as_str(),
        report.completeness.status.as_str(),
        report.impact_paths.len(),
        report.suggested_tests.len()
    );
    let _ = writeln!(rendered, "{}", report.reason);
    rendered
}

/// Render a concise human dependency trace.
#[must_use]
pub fn render_dependency_trace(report: &DependencyTraceReport) -> String {
    let mut rendered = String::new();
    let _ = writeln!(
        rendered,
        "trace-dependency {}: found={} declared={} used={} imports={}",
        report.dependency, report.found, report.declared, report.is_used, report.total_import_count
    );
    let _ = writeln!(rendered, "{}", report.reason);
    rendered
}
