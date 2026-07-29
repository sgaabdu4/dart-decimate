use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::graph::normalize_against;
use crate::{
    DeadCodeReport, FileTraceReport, JsonReport, ScannedProject, SymbolTraceReport, trace_file,
    trace_symbol,
};

/// Stable inspect schema version for composed evidence bundles.
pub const INSPECT_SCHEMA_VERSION: &str = "dart-decimate.inspect.v1";

/// A targeted evidence bundle for one file or top-level symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectReport {
    /// Schema identifier.
    pub schema_version: String,
    /// Typed JSON envelope discriminator.
    pub kind: String,
    /// Tool name.
    pub tool: String,
    /// Command that produced this report.
    pub command: String,
    /// Requested inspect target.
    pub target: InspectTarget,
    /// File-level graph trace for the target file.
    pub file_trace: FileTraceReport,
    /// Symbol-level trace when inspecting a symbol target.
    pub symbol_trace: Option<SymbolTraceReport>,
    /// Normal report output scoped to the target file.
    pub scoped_report: JsonReport,
    /// Conservative caveats an agent should preserve before editing.
    pub warnings: Vec<String>,
}

/// Inspect target metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InspectTarget {
    /// `file` or `symbol`.
    pub kind: String,
    /// Root-relative file path where possible.
    pub path: String,
    /// Symbol name for symbol targets.
    pub symbol: Option<String>,
}

/// Build an inspect bundle for one file.
#[must_use]
pub fn inspect_file(
    project: &ScannedProject,
    dead_code: &DeadCodeReport,
    scoped_report: JsonReport,
    path: impl AsRef<Path>,
) -> InspectReport {
    let path = normalize_against(&project.root, path.as_ref());
    let file_trace = trace_file(project, dead_code, &path);

    InspectReport {
        schema_version: INSPECT_SCHEMA_VERSION.to_owned(),
        kind: "inspect".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: "inspect".to_owned(),
        target: InspectTarget {
            kind: "file".to_owned(),
            path: display_path(&project.root, &path),
            symbol: None,
        },
        file_trace,
        symbol_trace: None,
        scoped_report,
        warnings: inspect_warnings(),
    }
}

/// Build an inspect bundle for one top-level symbol.
#[must_use]
pub fn inspect_symbol(
    project: &ScannedProject,
    dead_code: &DeadCodeReport,
    scoped_report: JsonReport,
    path: impl AsRef<Path>,
    symbol: &str,
) -> InspectReport {
    let path = normalize_against(&project.root, path.as_ref());
    let file_trace = trace_file(project, dead_code, &path);
    let symbol_trace = trace_symbol(project, dead_code, &path, symbol);

    InspectReport {
        schema_version: INSPECT_SCHEMA_VERSION.to_owned(),
        kind: "inspect".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: "inspect".to_owned(),
        target: InspectTarget {
            kind: "symbol".to_owned(),
            path: display_path(&project.root, &path),
            symbol: Some(symbol.to_owned()),
        },
        file_trace,
        symbol_trace: Some(symbol_trace),
        scoped_report,
        warnings: inspect_warnings(),
    }
}

/// Render a concise human inspect summary.
#[must_use]
pub fn render_inspect_report(report: &InspectReport) -> String {
    let mut rendered = String::new();
    let _ = writeln!(
        rendered,
        "inspect {}{}: findings={} reachable={}",
        report.target.path,
        report
            .target
            .symbol
            .as_ref()
            .map_or_else(String::new, |symbol| format!(":{symbol}")),
        report.scoped_report.summary.findings,
        report.file_trace.reachable
    );
    let _ = writeln!(rendered, "{}", report.file_trace.reason);
    if let Some(symbol_trace) = &report.symbol_trace {
        let _ = writeln!(
            rendered,
            "semantic={} completeness={} impact_paths={} suggested_tests={}",
            symbol_trace.semantic_decision.as_str(),
            symbol_trace.completeness.status.as_str(),
            symbol_trace.impact_paths.len(),
            symbol_trace.suggested_tests.len()
        );
        let _ = writeln!(rendered, "{}", symbol_trace.reason);
    }
    rendered
}

fn inspect_warnings() -> Vec<String> {
    vec![
        "Inspect is graph-based and syntactic; verify dynamic framework entry points before deleting code."
            .to_owned(),
    ]
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
