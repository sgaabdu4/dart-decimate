use std::collections::BTreeSet;
use std::path::PathBuf;

use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};

use crate::changed_scope::{changed_file_scope, changed_files};
use crate::graph::normalize_against;
use crate::output::{AnalysisResults, ReportCommand};
use crate::{
    ScannedProject, changed_workspace_file_scope, local_pub_packages, workspace_file_scope,
};

use super::{CliError, CommandRequest};

pub(super) fn report_command(command: Command) -> Command {
    command
        .arg(file_arg())
        .arg(workspace_arg())
        .arg(changed_workspaces_arg())
        .arg(changed_since_arg())
        .arg(compare_arg())
}

pub(super) fn file_arg() -> Arg {
    Arg::new("file")
        .long("file")
        .value_name("PATH")
        .help("Scope findings to a specific file")
        .num_args(1)
        .action(ArgAction::Append)
        .value_parser(value_parser!(PathBuf))
}

pub(super) fn workspace_arg() -> Arg {
    Arg::new("workspace")
        .long("workspace")
        .value_name("PATTERN")
        .help("Scope findings to matching local pub package names or roots")
        .num_args(1)
        .action(ArgAction::Append)
        .conflicts_with("changed-workspaces")
}

pub(super) fn changed_workspaces_arg() -> Arg {
    Arg::new("changed-workspaces")
        .long("changed-workspaces")
        .value_name("REF")
        .help("Scope findings to local pub packages changed since a Git ref")
        .conflicts_with("workspace")
}

pub(super) fn changed_since_arg() -> Arg {
    Arg::new("changed-since")
        .long("changed-since")
        .value_name("REF")
        .help("Scope findings to files changed since a Git ref")
        .conflicts_with("compare")
}

pub(super) fn compare_arg() -> Arg {
    Arg::new("compare")
        .long("compare")
        .value_name("REF")
        .help("Alias for --changed-since; scope findings to files changed since a Git ref")
        .conflicts_with("changed-since")
}

pub(super) fn workspace_patterns(command: ReportCommand, subcommand: &ArgMatches) -> Vec<String> {
    if !supports_report_scope(command) {
        return Vec::new();
    }

    subcommand
        .get_many::<String>("workspace")
        .map(|values| values.cloned().collect())
        .unwrap_or_default()
}

pub(super) fn file_paths(command: ReportCommand, subcommand: &ArgMatches) -> Vec<PathBuf> {
    if !supports_report_scope(command) {
        return Vec::new();
    }

    subcommand
        .get_many::<PathBuf>("file")
        .map(|values| values.cloned().collect())
        .unwrap_or_default()
}

pub(super) fn changed_workspaces(
    command: ReportCommand,
    subcommand: &ArgMatches,
) -> Option<String> {
    if supports_report_scope(command) {
        subcommand.get_one::<String>("changed-workspaces").cloned()
    } else {
        None
    }
}

pub(super) fn changed_since(command: ReportCommand, subcommand: &ArgMatches) -> Option<String> {
    if supports_report_scope(command) {
        subcommand
            .get_one::<String>("changed-since")
            .or_else(|| subcommand.get_one::<String>("compare"))
            .cloned()
    } else {
        None
    }
}

pub(super) fn apply_report_scope(
    project: &ScannedProject,
    results: &mut AnalysisResults,
    request: &CommandRequest,
) -> Result<(), CliError> {
    if request.file_paths.is_empty()
        && request.workspace_patterns.is_empty()
        && request.changed_workspaces.is_none()
        && request.changed_since.is_none()
    {
        return Ok(());
    }

    if !request.file_paths.is_empty() {
        let file_scope = request
            .file_paths
            .iter()
            .map(|path| normalize_against(&project.root, path))
            .collect::<Vec<_>>();
        results.file_scope = Some(intersect_scope(results.file_scope.take(), file_scope));
    }

    if let Some(base) = &request.changed_since {
        let file_scope = changed_file_scope_for_command(request.command, project, base)?;
        results.file_scope = Some(intersect_scope(results.file_scope.take(), file_scope));
    }

    if !request.workspace_patterns.is_empty() {
        let packages = local_pub_packages(&project.root)?;
        let workspace_scope =
            workspace_file_scope(project, &packages, &request.workspace_patterns)?;
        results.file_scope = Some(intersect_scope(results.file_scope.take(), workspace_scope));
    }

    if let Some(base) = &request.changed_workspaces {
        let packages = local_pub_packages(&project.root)?;
        let changed = changed_files(&project.root, base)?;
        let workspace_scope = changed_workspace_file_scope(project, &packages, &changed);
        results.file_scope = Some(intersect_scope(results.file_scope.take(), workspace_scope));
    }

    Ok(())
}

fn changed_file_scope_for_command(
    command: ReportCommand,
    project: &ScannedProject,
    base: &str,
) -> Result<Vec<PathBuf>, CliError> {
    if command == ReportCommand::Audit {
        return Ok(changed_file_scope(project, base)?);
    }
    Ok(changed_files(&project.root, base)?)
}

fn intersect_scope(existing: Option<Vec<PathBuf>>, next: Vec<PathBuf>) -> Vec<PathBuf> {
    let Some(existing) = existing else {
        return next;
    };
    let next = next.into_iter().collect::<BTreeSet<_>>();
    existing
        .into_iter()
        .filter(|path| next.contains(path))
        .collect()
}

fn supports_report_scope(command: ReportCommand) -> bool {
    !matches!(
        command,
        ReportCommand::TraceFile
            | ReportCommand::TraceSymbol
            | ReportCommand::TraceDependency
            | ReportCommand::TraceClone
            | ReportCommand::Inspect
    )
}
