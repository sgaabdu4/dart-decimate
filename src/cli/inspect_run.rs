use std::io::Write;
use std::path::Path;

use super::analyze::analyze_project;
use super::entry_points::entry_points_for_check;
use super::{CliError, CommandRequest, ReportOutputFormat};
use crate::config::apply_rules_to_report;
use crate::graph::normalize_against;
use crate::inspect::{inspect_file, inspect_symbol, render_inspect_report};
use crate::output::{ReportCommand, build_json_report};
use crate::{ScannedProject, find_dead_code};

pub(super) fn run_inspect_request<W: Write>(
    request: &CommandRequest,
    project: &ScannedProject,
    mut writer: W,
) -> Result<i32, CliError> {
    let dead_code = find_dead_code(
        &project.graph,
        entry_points_for_check(project, &request.entry_points, request.entry_point_mode()),
    );
    let report = if let Some(spec) = request.trace_symbol.as_ref() {
        let scoped_report = scoped_check_report(project, request, &spec.file)?;
        inspect_symbol(project, &dead_code, scoped_report, &spec.file, &spec.symbol)
    } else if let Some(file) = request.trace_file.as_ref() {
        let scoped_report = scoped_check_report(project, request, file)?;
        inspect_file(project, &dead_code, scoped_report, file)
    } else {
        return Err(CliError::MissingInspectTarget);
    };

    match request.format {
        ReportOutputFormat::Human => writer.write_all(render_inspect_report(&report).as_bytes())?,
        ReportOutputFormat::Json => {
            serde_json::to_writer_pretty(&mut writer, &report)?;
            writeln!(writer)?;
        }
        ReportOutputFormat::Html | ReportOutputFormat::HtmlOpen => {
            unreachable!("HTML is rejected before inspect rendering")
        }
        ReportOutputFormat::Sarif => unreachable!("SARIF is rejected before inspect rendering"),
    }

    Ok(0)
}

fn scoped_check_report(
    project: &ScannedProject,
    request: &CommandRequest,
    path: &Path,
) -> Result<crate::JsonReport, CliError> {
    let mut scoped_request = request.clone();
    scoped_request.command = ReportCommand::Check;
    scoped_request.audit_base = None;
    scoped_request.file_paths = Vec::new();
    scoped_request.workspace_patterns = Vec::new();
    scoped_request.changed_workspaces = None;
    scoped_request.baseline_paths = Vec::new();
    scoped_request.save_baseline = None;
    scoped_request.regression_baseline = None;
    scoped_request.save_regression_baseline = None;
    scoped_request.fail_on_regression = false;

    let mut results = analyze_project(project, &scoped_request)?;
    results.file_scope = Some(vec![normalize_against(&project.root, path)]);
    let mut report = build_json_report(project, &results);
    apply_rules_to_report(&mut report, &scoped_request.rules)?;
    Ok(report)
}
