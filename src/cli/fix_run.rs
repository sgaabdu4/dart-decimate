use std::collections::BTreeSet;
use std::io::Write;

use clap::{Arg, ArgAction, ArgGroup, ArgMatches, Command};

use crate::config::apply_rules_to_report;
use crate::fix::{FixMode, fix_findings, render_fix_report};
use crate::output::{ReportCommand, build_json_report};
use crate::scan::{ScanOptions, scan_project_with_options};
use crate::{
    DuplicateOptions, FeatureFlagOptions, HealthOptions, RegressionTolerance, SecurityOptions,
};

use super::analyze::analyze_project;
use super::{CliError, CommandRequest, OutputFormat};

pub(super) fn fix_command() -> Command {
    super::scope_args::report_command(super::scan_command(
        Command::new("fix").about("Plan or apply safe auto-fixes"),
    ))
    .arg(
        Arg::new("action")
            .long("action")
            .value_name("ACTION")
            .help("Only plan or apply this auto-fix action")
            .num_args(1)
            .action(ArgAction::Append),
    )
    .arg(
        Arg::new("dry-run")
            .long("dry-run")
            .help("Preview safe fixes without modifying files")
            .conflicts_with_all(["apply", "yes"])
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new("apply")
            .long("apply")
            .help("Apply planned fixes")
            .requires("apply-confirmation")
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new("yes")
            .long("yes")
            .help("Apply planned fixes without prompting")
            .action(ArgAction::SetTrue),
    )
    .arg(
        Arg::new("confirm")
            .long("confirm")
            .help("Confirm that --apply may modify files")
            .action(ArgAction::SetTrue),
    )
    .group(
        ArgGroup::new("apply-confirmation")
            .args(["confirm", "yes"])
            .multiple(false),
    )
}

pub(super) fn run_fix<W: Write>(subcommand: &ArgMatches, mut writer: W) -> Result<i32, CliError> {
    let root = super::common_args::root_path(subcommand);
    let config = super::load_config(&root, subcommand)?;
    if super::report_output_format(subcommand, &config) == super::ReportOutputFormat::Sarif {
        return Err(CliError::UnsupportedSarifFormat { command: "fix" });
    }
    let format = super::output_format(subcommand, &config);
    let production = super::mode_args::production(subcommand, &config);
    let project = scan_project_with_options(
        &root,
        &ScanOptions {
            ignore_patterns: config.ignore_patterns.clone(),
            conditional_environment: super::common_args::conditional_environment(subcommand),
        },
    )?;
    let command = ReportCommand::Check;
    let request = CommandRequest {
        command,
        root,
        format: match format {
            OutputFormat::Human => super::ReportOutputFormat::Human,
            OutputFormat::Json => super::ReportOutputFormat::Json,
        },
        entry_points: super::entry_points(subcommand, &config),
        production,
        symbol_options: super::SymbolRequestOptions {
            include_entry_exports: super::mode_args::include_entry_exports(subcommand, &config),
            private_type_leaks: false,
        },
        boundaries: config.boundaries.clone(),
        boundary_coverage: false,
        boundary_allow_unmatched: config.boundary_allow_unmatched.clone(),
        boundary_calls: Vec::new(),
        policy_packs: Vec::new(),
        audit_base: None,
        audit_gate: super::audit_run::AuditGate::default(),
        file_paths: super::scope_args::file_paths(command, subcommand),
        workspace_patterns: super::scope_args::workspace_patterns(command, subcommand),
        changed_workspaces: super::scope_args::changed_workspaces(command, subcommand),
        changed_since: super::scope_args::changed_since(command, subcommand),
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
        regression_tolerance: RegressionTolerance::default(),
        fail_on_regression: false,
        trace_file: None,
        trace_symbol: None,
        trace_dependency: None,
        trace_clone: None,
        duplicate_options: DuplicateOptions::default(),
        health_options: HealthOptions::default(),
        feature_flag_options: FeatureFlagOptions::default(),
        security_options: SecurityOptions::default(),
        issue_filters: super::issue_filter_args::IssueFilters::default(),
        scan_options: ScanOptions {
            ignore_patterns: config.ignore_patterns.clone(),
            conditional_environment: super::common_args::conditional_environment(subcommand),
        },
        ignore_dependencies: config.ignore_dependencies.clone(),
        ignore_dependency_overrides: config.ignore_dependency_overrides.clone(),
        rules: config.rules.clone(),
    };
    let mut results = analyze_project(&project, &request)?;
    super::scope_args::apply_report_scope(&project, &mut results, &request)?;
    let mut report = build_json_report(&project, &results);
    apply_rules_to_report(&mut report, &request.rules)?;

    let actions = subcommand
        .get_many::<String>("action")
        .map(|values| values.cloned().collect::<BTreeSet<_>>())
        .unwrap_or_default();
    let mode = if subcommand.get_flag("apply") || subcommand.get_flag("yes") {
        FixMode::Apply
    } else {
        FixMode::DryRun
    };
    let fix_report = fix_findings(&project.root, &report.findings, &actions, mode);
    match format {
        OutputFormat::Human => writer.write_all(render_fix_report(&fix_report).as_bytes())?,
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut writer, &fix_report)?;
            writeln!(writer)?;
        }
    }

    Ok(i32::from(
        mode == FixMode::Apply && fix_report.summary.skipped > 0,
    ))
}
