use std::io::Write;

use super::{CliError, CommandRequest, ReportOutputFormat};
use crate::output::ReportCommand;
use crate::scan::ScannedProject;
use crate::trace::{
    render_dependency_trace, render_file_trace, render_symbol_trace, trace_dependency, trace_file,
    trace_symbol,
};
use crate::{find_dead_code, render_clone_trace, trace_clone};

use super::entry_points::entry_points_for_check;

pub(super) fn run_trace_request<W: Write>(
    request: &CommandRequest,
    project: &ScannedProject,
    mut writer: W,
) -> Result<i32, CliError> {
    match request.command {
        ReportCommand::TraceFile => run_trace_file(request, project, &mut writer)?,
        ReportCommand::TraceSymbol => run_trace_symbol(request, project, &mut writer)?,
        ReportCommand::TraceDependency => run_trace_dependency(request, project, &mut writer)?,
        ReportCommand::TraceClone => run_trace_clone(request, project, &mut writer)?,
        ReportCommand::Check
        | ReportCommand::Audit
        | ReportCommand::DeadCode
        | ReportCommand::Cycles
        | ReportCommand::Dupes
        | ReportCommand::Health
        | ReportCommand::Flags
        | ReportCommand::Security
        | ReportCommand::Inspect => {
            unreachable!("run_trace_request is only called for trace commands")
        }
    }

    Ok(0)
}

fn run_trace_file<W: Write>(
    request: &CommandRequest,
    project: &ScannedProject,
    writer: &mut W,
) -> Result<(), CliError> {
    let dead_code = find_dead_code(
        &project.graph,
        entry_points_for_check(project, &request.entry_points, request.entry_point_mode()),
    );
    let Some(file) = request.trace_file.as_ref() else {
        return Err(CliError::MissingTraceFile);
    };
    let report = trace_file(project, &dead_code, file);
    match request.format {
        ReportOutputFormat::Human => writer.write_all(render_file_trace(&report).as_bytes())?,
        ReportOutputFormat::Json => {
            serde_json::to_writer_pretty(&mut *writer, &report)?;
            writeln!(writer)?;
        }
        ReportOutputFormat::Html | ReportOutputFormat::HtmlOpen => {
            unreachable!("HTML is rejected before trace rendering")
        }
        ReportOutputFormat::Sarif => unreachable!("SARIF is rejected before trace rendering"),
    }
    Ok(())
}

fn run_trace_symbol<W: Write>(
    request: &CommandRequest,
    project: &ScannedProject,
    writer: &mut W,
) -> Result<(), CliError> {
    let dead_code = find_dead_code(
        &project.graph,
        entry_points_for_check(project, &request.entry_points, request.entry_point_mode()),
    );
    let Some(spec) = request.trace_symbol.as_ref() else {
        return Err(CliError::TraceSymbol {
            value: String::new(),
        });
    };
    let report = trace_symbol(project, &dead_code, &spec.file, &spec.symbol);
    match request.format {
        ReportOutputFormat::Human => writer.write_all(render_symbol_trace(&report).as_bytes())?,
        ReportOutputFormat::Json => {
            serde_json::to_writer_pretty(&mut *writer, &report)?;
            writeln!(writer)?;
        }
        ReportOutputFormat::Html | ReportOutputFormat::HtmlOpen => {
            unreachable!("HTML is rejected before trace rendering")
        }
        ReportOutputFormat::Sarif => unreachable!("SARIF is rejected before trace rendering"),
    }
    Ok(())
}

fn run_trace_dependency<W: Write>(
    request: &CommandRequest,
    project: &ScannedProject,
    writer: &mut W,
) -> Result<(), CliError> {
    let Some(dependency) = request.trace_dependency.as_ref() else {
        return Err(CliError::MissingTraceDependency);
    };
    let report = trace_dependency(project, dependency)?;
    match request.format {
        ReportOutputFormat::Human => {
            writer.write_all(render_dependency_trace(&report).as_bytes())?;
        }
        ReportOutputFormat::Json => {
            serde_json::to_writer_pretty(&mut *writer, &report)?;
            writeln!(writer)?;
        }
        ReportOutputFormat::Html | ReportOutputFormat::HtmlOpen => {
            unreachable!("HTML is rejected before trace rendering")
        }
        ReportOutputFormat::Sarif => unreachable!("SARIF is rejected before trace rendering"),
    }
    Ok(())
}

fn run_trace_clone<W: Write>(
    request: &CommandRequest,
    project: &ScannedProject,
    writer: &mut W,
) -> Result<(), CliError> {
    let Some(trace) = request.trace_clone.as_ref() else {
        unreachable!("clap requires --fingerprint for trace-clone");
    };
    let duplicates = crate::detect_duplicates(project, &request.duplicate_options)?;
    let report = trace_clone(project, &duplicates, trace);
    match request.format {
        ReportOutputFormat::Human => writer.write_all(render_clone_trace(&report).as_bytes())?,
        ReportOutputFormat::Json => {
            serde_json::to_writer_pretty(&mut *writer, &report)?;
            writeln!(writer)?;
        }
        ReportOutputFormat::Html | ReportOutputFormat::HtmlOpen => {
            unreachable!("HTML is rejected before trace rendering")
        }
        ReportOutputFormat::Sarif => unreachable!("SARIF is rejected before trace rendering"),
    }
    Ok(())
}
