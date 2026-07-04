use std::io::Write;

use clap::{Arg, ArgAction, ArgMatches, Command};

use super::common_args::{
    conditional_environment, config_arg, dart_platform_arg, entry_arg, format_arg, root_arg,
    root_flag_arg, root_path,
};
use super::entry_points::entry_points_for_check;
use super::{CliError, OutputFormat, entry_points, load_config, output_format};
use crate::{
    ProjectListOptions, ProjectListSection, local_pub_packages, project_list_report,
    scan::{ScannedProject, scan_project_with_options},
};

pub(super) fn list_command() -> Command {
    Command::new("list")
        .about("List Dart Decimate project structure")
        .arg(root_arg())
        .arg(root_flag_arg())
        .arg(format_arg())
        .arg(config_arg())
        .arg(entry_arg())
        .arg(dart_platform_arg())
        .arg(super::mode_args::production_arg())
        .arg(super::mode_args::no_production_arg())
        .arg(section_arg("files", "Include parsed Dart files"))
        .arg(section_arg(
            "entry-points",
            "Include effective Dart entry points",
        ))
        .arg(section_arg(
            "workspaces",
            "Include discovered local pub packages",
        ))
        .arg(section_arg(
            "plugins",
            "Include active Dart Decimate adapters",
        ))
        .arg(section_arg(
            "boundaries",
            "Include configured architecture boundaries",
        ))
        .arg(super::scope_args::file_arg())
        .arg(super::scope_args::workspace_arg())
        .arg(super::scope_args::changed_workspaces_arg())
}

pub(super) fn workspaces_command() -> Command {
    Command::new("workspaces")
        .about("List discovered local pub packages")
        .arg(root_arg())
        .arg(root_flag_arg())
        .arg(format_arg())
        .arg(config_arg())
        .arg(entry_arg())
        .arg(dart_platform_arg())
        .arg(super::mode_args::production_arg())
        .arg(super::mode_args::no_production_arg())
        .arg(super::scope_args::file_arg())
        .arg(super::scope_args::workspace_arg())
        .arg(super::scope_args::changed_workspaces_arg())
}

pub(super) fn run_list<W: Write>(subcommand: &ArgMatches, mut writer: W) -> Result<i32, CliError> {
    run_list_with_options(subcommand, &mut writer, "list", &list_options(subcommand))
}

pub(super) fn run_workspaces<W: Write>(
    subcommand: &ArgMatches,
    mut writer: W,
) -> Result<i32, CliError> {
    run_list_with_options(
        subcommand,
        &mut writer,
        "workspaces",
        &ProjectListOptions::from_sections([ProjectListSection::Workspaces]),
    )
}

fn run_list_with_options<W: Write>(
    subcommand: &ArgMatches,
    writer: &mut W,
    command_name: &str,
    options: &ProjectListOptions,
) -> Result<i32, CliError> {
    let root = root_path(subcommand);
    let config = load_config(&root, subcommand)?;
    let format = output_format(subcommand, &config);
    let explicit_entries = entry_points(subcommand, &config);
    let production = super::mode_args::production(subcommand, &config);
    let entry_source = entry_source(subcommand, explicit_entries.is_empty(), production);
    let scan_options = crate::ScanOptions {
        ignore_patterns: config.ignore_patterns.clone(),
        conditional_environment: conditional_environment(subcommand),
    };
    let project = scan_project_with_options(&root, &scan_options)?;
    let mut packages = local_pub_packages(&project.root)?;
    let project = scoped_project(subcommand, project, &mut packages, production)?;
    let entries = entry_points_for_check(
        &project,
        &explicit_entries,
        super::mode_args::production_mode(production),
    );
    let mut report = project_list_report(
        &project,
        &packages,
        &entries,
        entry_source,
        crate::ProjectListBoundaryConfig {
            rules: &config.boundaries,
            presets: &config.boundary_presets,
            allow_unmatched: &config.boundary_allow_unmatched,
        },
        options,
    );
    command_name.clone_into(&mut report.command);

    match format {
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut *writer, &report)?;
            writeln!(writer)?;
        }
        OutputFormat::Human => writer.write_all(render_list(&report).as_bytes())?,
    }

    Ok(0)
}

fn scoped_project(
    matches: &ArgMatches,
    project: ScannedProject,
    packages: &mut Vec<crate::LocalPubPackage>,
    production: bool,
) -> Result<ScannedProject, CliError> {
    let request = crate::cli::CommandRequest {
        command: crate::ReportCommand::Check,
        root: project.root.clone(),
        format: super::ReportOutputFormat::Json,
        entry_points: Vec::new(),
        production,
        symbol_options: super::SymbolRequestOptions::default(),
        boundaries: Vec::new(),
        boundary_coverage: false,
        boundary_allow_unmatched: Vec::new(),
        boundary_calls: Vec::new(),
        policy_packs: Vec::new(),
        audit_base: None,
        audit_gate: super::audit_run::AuditGate::default(),
        file_paths: matches
            .get_many::<std::path::PathBuf>("file")
            .map(|values| values.cloned().collect::<Vec<_>>())
            .unwrap_or_default(),
        workspace_patterns: matches
            .get_many::<String>("workspace")
            .map(|values| values.cloned().collect::<Vec<_>>())
            .unwrap_or_default(),
        changed_workspaces: matches.get_one::<String>("changed-workspaces").cloned(),
        changed_since: None,
        baseline_paths: Vec::new(),
        security_sarif_file: None,
        security_gate: None,
        security_diff: None,
        security_changed_since: None,
        security_issue_mode: super::security_args::SecurityIssueMode::default(),
        security_summary_mode: super::security_args::SecuritySummaryMode::default(),
        save_baseline: None,
        regression_baseline: None,
        save_regression_baseline: None,
        regression_tolerance: crate::RegressionTolerance::default(),
        fail_on_regression: false,
        trace_file: None,
        trace_symbol: None,
        trace_dependency: None,
        trace_clone: None,
        duplicate_options: crate::DuplicateOptions::default(),
        health_options: crate::HealthOptions::default(),
        feature_flag_options: crate::FeatureFlagOptions::default(),
        security_options: crate::SecurityOptions::default(),
        issue_filters: super::issue_filter_args::IssueFilters::default(),
        scan_options: crate::ScanOptions::default(),
        ignore_dependencies: Vec::new(),
        ignore_dependency_overrides: Vec::new(),
        rules: crate::config::RuleConfig::default(),
    };
    let mut results = crate::output::AnalysisResults {
        command: crate::ReportCommand::Check,
        dead_code: None,
        symbols: None,
        cycles: Vec::new(),
        re_export_cycles: Vec::new(),
        boundary_violations: Vec::new(),
        boundary_coverage: Vec::new(),
        boundary_call_violations: Vec::new(),
        policy_violations: Vec::new(),
        dependency_hygiene: None,
        duplicates: None,
        health: None,
        feature_flags: None,
        security: None,
        routes: None,
        widgets: None,
        file_scope: None,
        require_suppression_reasons: false,
    };
    super::scope_args::apply_report_scope(&project, &mut results, &request)?;
    let Some(scope) = results.file_scope else {
        return Ok(project);
    };

    let scope = scope.into_iter().collect::<std::collections::BTreeSet<_>>();
    let package_roots = packages
        .iter()
        .map(|package| package.root.clone())
        .collect::<Vec<_>>();
    packages.retain(|package| package_in_scope(package, &package_roots, &scope));
    Ok(ScannedProject {
        root: project.root,
        scan_roots: project.scan_roots,
        files: project
            .files
            .into_iter()
            .filter(|file| scope.contains(&file.path))
            .collect(),
        graph: project.graph,
    })
}

fn package_in_scope(
    package: &crate::LocalPubPackage,
    package_roots: &[std::path::PathBuf],
    scope: &std::collections::BTreeSet<std::path::PathBuf>,
) -> bool {
    scope.contains(&package.pubspec_path)
        || scope
            .iter()
            .any(|path| owning_root(package_roots, path) == Some(&package.root))
}

fn owning_root<'root>(
    roots: &'root [std::path::PathBuf],
    path: &std::path::Path,
) -> Option<&'root std::path::PathBuf> {
    roots
        .iter()
        .filter(|root| path.starts_with(root))
        .max_by_key(|root| root.components().count())
}

fn section_arg(id: &'static str, help: &'static str) -> Arg {
    Arg::new(id).long(id).help(help).action(ArgAction::SetTrue)
}

fn list_options(matches: &ArgMatches) -> ProjectListOptions {
    let any = [
        "files",
        "entry-points",
        "workspaces",
        "plugins",
        "boundaries",
    ]
    .into_iter()
    .any(|id| matches.get_flag(id));
    if !any {
        return ProjectListOptions::all();
    }

    ProjectListOptions::from_sections(
        [
            (matches.get_flag("files"), ProjectListSection::Files),
            (
                matches.get_flag("entry-points"),
                ProjectListSection::EntryPoints,
            ),
            (
                matches.get_flag("workspaces"),
                ProjectListSection::Workspaces,
            ),
            (matches.get_flag("plugins"), ProjectListSection::Plugins),
            (
                matches.get_flag("boundaries"),
                ProjectListSection::Boundaries,
            ),
        ]
        .into_iter()
        .filter_map(|(enabled, section)| enabled.then_some(section)),
    )
}

fn entry_source(matches: &ArgMatches, defaulted: bool, production: bool) -> &'static str {
    if defaulted && production {
        "production"
    } else if defaulted {
        "default"
    } else if matches.value_source("entry") == Some(clap::parser::ValueSource::CommandLine) {
        "cli"
    } else {
        "config"
    }
}

fn render_list(report: &crate::ProjectListReport) -> String {
    format!(
        "Files: {}\nEdges: {}\nEntry points: {}\nWorkspaces: {}\nPlugins: {}\nBoundaries: {}\n",
        report.summary.files,
        report.summary.edges,
        report.summary.entry_points,
        report.summary.workspaces,
        report.summary.plugins,
        report.summary.boundary_zones
    )
}
