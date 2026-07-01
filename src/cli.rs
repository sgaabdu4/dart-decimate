use std::ffi::OsString;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::{Arg, ArgAction, ArgMatches, Command, parser::ValueSource};

use crate::baseline::{
    RegressionTolerance, apply_baseline_to_report, baseline_from_report,
    compare_regression_baseline, load_baseline, load_regression_baseline,
    regression_baseline_from_report, save_baseline as write_baseline,
    save_regression_baseline as write_regression_baseline,
};
use crate::config::{
    ConfigError, DartDecimateConfig, IgnoreDependencyOverrideRule, RuleConfig,
    apply_rules_to_report, load_dart_decimate_config,
};
use crate::output::{
    ReportCommand, Verdict, build_json_report, filter_report_findings, render_html_report,
    render_human_report, render_sarif_report,
};
use crate::scan::{ScanOptions, scan_project_with_options};
use crate::{
    BoundaryCallRule, BoundaryRule, DuplicateOptions, FeatureFlagOptions, HealthOptions,
    SecurityOptions,
};

mod analyze;
mod analyzer_options;
mod audit_run;
mod boundary_args;
mod ci_template_run;
mod command_name;
mod common_args;
mod config_run;
mod coverage_run;
mod decision_surface_run;
mod default_command;
mod dupes_args;
mod entry_points;
mod error;
mod error_output;
mod explain_run;
mod fix_run;
mod flags_args;
mod health_args;
mod hooks_run;
mod html_open;
mod impact_run;
mod init_run;
mod inspect_args;
mod inspect_run;
mod issue_filter_args;
mod list_run;
mod mode_args;
mod output_format;
mod regression_args;
mod request_args;
mod schema_commands;
mod scope_args;
mod security_args;
mod security_gate_run;
mod security_summary_run;
mod summary_args;
mod trace_args;
mod trace_run;
mod unsupported_run;
mod watch_run;
use crate::security_gate::{SecurityDiffSource, SecurityGateMode};
use analyze::analyze_project;
use analyzer_options::{
    duplicate_options_for, feature_flag_options_for, health_options_for, security_options_for,
};
use boundary_args::boundary_command;
use ci_template_run::{ci_command, ci_template_command, run_ci, run_ci_template};
use command_name::report_command_from_name;
use common_args::{
    audit_baseline_arg, baseline_command, report_command, root_path, scan_command,
    symbol_options_command,
};
use config_run::{config_command, run_config};
use coverage_run::{coverage_command, run_coverage};
use decision_surface_run::{
    decision_surface_command, max_decisions_arg, review_command, run_decision_surface,
};
use dupes_args::{dupes_command, trace_clone_command};
pub use error::CliError;
pub use error_output::run_from_env;
use explain_run::{explain_command, run_explain};
use fix_run::{fix_command, run_fix};
use flags_args::flags_command;
use health_args::{health_command, health_command_without_top};
use hooks_run::{hooks_command, run_hooks, run_setup_hooks, setup_hooks_command};
use impact_run::{impact_command, run_impact};
use init_run::{init_command, run_init};
use inspect_args::inspect_command;
use inspect_run::run_inspect_request;
use issue_filter_args::{check_issue_filter_command, dead_code_issue_filter_command};
use list_run::{list_command, run_list, run_workspaces, workspaces_command};
use output_format::{
    OutputFormat, ReportOutputFormat, output_format, output_format_value, report_output_format,
};
use regression_args::{
    regression_baseline_path, regression_request_args, save_regression_baseline_path,
};
use request_args::{boundary_rules, trace_request_args};
use schema_commands::{
    config_schema_command, manifest_command, report_schema_command, rule_pack_schema_command,
    run_config_schema, run_manifest, run_report_schema, run_rule_pack_schema,
};
use security_args::{
    SecurityCliOptions, SecurityIssueMode, SecuritySummaryMode, security_cli_options,
    security_command,
};
use security_gate_run::apply_security_gate;
use trace_args::{
    trace_command, trace_dependency_command, trace_file_command, trace_symbol_command,
};
use trace_run::run_trace_request;
use unsupported_run::{
    license_command, migrate_command, run_license, run_migrate, run_telemetry, telemetry_command,
};
use watch_run::{run_watch, watch_command};

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandRequest {
    command: ReportCommand,
    root: PathBuf,
    format: ReportOutputFormat,
    entry_points: Vec<PathBuf>,
    production: bool,
    symbol_options: SymbolRequestOptions,
    boundaries: Vec<BoundaryRule>,
    boundary_coverage: bool,
    boundary_allow_unmatched: Vec<String>,
    boundary_calls: Vec<BoundaryCallRule>,
    policy_packs: Vec<PathBuf>,
    audit_base: Option<String>,
    audit_gate: audit_run::AuditGate,
    file_paths: Vec<PathBuf>,
    workspace_patterns: Vec<String>,
    changed_workspaces: Option<String>,
    changed_since: Option<String>,
    baseline_paths: Vec<PathBuf>,
    security_sarif_file: Option<PathBuf>,
    security_gate: Option<SecurityGateMode>,
    security_diff: Option<SecurityDiffSource>,
    security_changed_since: Option<String>,
    security_issue_mode: SecurityIssueMode,
    security_summary_mode: SecuritySummaryMode,
    save_baseline: Option<PathBuf>,
    regression_baseline: Option<PathBuf>,
    save_regression_baseline: Option<PathBuf>,
    regression_tolerance: RegressionTolerance,
    fail_on_regression: bool,
    trace_file: Option<PathBuf>,
    trace_symbol: Option<TraceSymbolSpec>,
    trace_dependency: Option<String>,
    trace_clone: Option<String>,
    duplicate_options: DuplicateOptions,
    health_options: HealthOptions,
    feature_flag_options: FeatureFlagOptions,
    security_options: SecurityOptions,
    issue_filters: issue_filter_args::IssueFilters,
    scan_options: ScanOptions,
    ignore_dependencies: Vec<String>,
    ignore_dependency_overrides: Vec<IgnoreDependencyOverrideRule>,
    rules: RuleConfig,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SymbolRequestOptions {
    include_entry_exports: bool,
    private_type_leaks: bool,
}

impl CommandRequest {
    fn entry_point_mode(&self) -> entry_points::EntryPointMode {
        mode_args::production_mode(self.production)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TraceSymbolSpec {
    file: PathBuf,
    symbol: String,
}

/// Run Dart Decimate from explicit arguments.
///
/// # Errors
///
/// Returns [`CliError`] for invalid arguments, scan failures, or output errors.
pub fn run_from<I, T, W>(args: I, writer: W) -> Result<i32, CliError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
    W: Write,
{
    let matches = command().try_get_matches_from(default_command::args_with_default_check(args))?;
    match matches.subcommand() {
        Some(("config-schema", _)) => return run_config_schema(writer),
        Some(("report-schema", _)) => return run_report_schema(writer),
        Some(("rule-pack-schema", _)) => return run_rule_pack_schema(writer),
        Some(("schema", _)) => return run_manifest(writer),
        Some(("config", subcommand)) => return run_config(subcommand, writer),
        Some(("list", subcommand)) => return run_list(subcommand, writer),
        Some(("workspaces", subcommand)) => return run_workspaces(subcommand, writer),
        Some(("explain", subcommand)) => return run_explain(subcommand, writer),
        Some(("fix", subcommand)) => return run_fix(subcommand, writer),
        Some(("migrate", subcommand)) => return run_migrate(subcommand, writer),
        Some(("telemetry", subcommand)) => return run_telemetry(subcommand, writer),
        Some(("license", subcommand)) => return run_license(subcommand, writer),
        Some(("impact", subcommand)) => return run_impact(subcommand, writer),
        Some(("ci", subcommand)) => return run_ci(subcommand, writer),
        Some(("ci-template", subcommand)) => return run_ci_template(subcommand, writer),
        Some(("init", subcommand)) => return run_init(subcommand, writer),
        Some(("hooks", subcommand)) => return run_hooks(subcommand, writer),
        Some(("setup-hooks", subcommand)) => return run_setup_hooks(subcommand, writer),
        Some(("watch", subcommand)) => return run_watch(subcommand, writer),
        Some(("decision-surface", subcommand)) => {
            return run_decision_surface(subcommand, writer, "decision-surface");
        }
        Some(("review", subcommand)) => return run_decision_surface(subcommand, writer, "review"),
        Some(("coverage", subcommand)) => return run_coverage(subcommand, writer),
        Some(("audit", subcommand)) if subcommand.get_flag("brief") => {
            return run_decision_surface(subcommand, writer, "audit --brief");
        }
        _ => {}
    }
    let request = request_from_matches(&matches)?;
    run_request(&request, writer)
}

fn run_request<W: Write>(request: &CommandRequest, mut writer: W) -> Result<i32, CliError> {
    let project = scan_project_with_options(&request.root, &request.scan_options)?;
    if matches!(
        request.command,
        ReportCommand::TraceFile
            | ReportCommand::TraceSymbol
            | ReportCommand::TraceDependency
            | ReportCommand::TraceClone
    ) {
        return run_trace_request(request, &project, writer);
    }
    if request.command == ReportCommand::Inspect {
        return run_inspect_request(request, &project, writer);
    }

    let mut results = analyze_project(&project, request)?;
    let audit_context = audit_run::prepare_context(&project, request, &mut results)?;
    scope_args::apply_report_scope(&project, &mut results, request)?;
    let mut report = build_json_report(&project, &results);
    apply_rules_to_report(&mut report, &request.rules)?;
    filter_report_findings(&mut report, &request.issue_filters.kinds);
    for path in &request.baseline_paths {
        let baseline = load_baseline(resolve_report_path(&project.root, path))?;
        apply_baseline_to_report(&mut report, &baseline);
    }
    let regressed = if let Some(path) = &request.regression_baseline {
        let baseline = load_regression_baseline(resolve_report_path(&project.root, path))?;
        compare_regression_baseline(&report, &baseline, request.regression_tolerance).regressed()
    } else {
        false
    };
    if let Some(path) = &request.save_baseline {
        let baseline = baseline_from_report(&report);
        write_baseline(resolve_report_path(&project.root, path), &baseline)?;
    }
    if let Some(path) = &request.save_regression_baseline {
        let baseline = regression_baseline_from_report(&report);
        write_regression_baseline(resolve_report_path(&project.root, path), &baseline)?;
    }
    apply_security_gate(&project, request, &mut report)?;
    if let Some(path) = &request.security_sarif_file {
        let mut file = File::create(resolve_report_path(&project.root, path))?;
        serde_json::to_writer_pretty(&mut file, &render_sarif_report(&report))?;
        writeln!(file)?;
    }
    security_summary_run::apply_security_summary(request, &mut report);
    audit_run::apply_risk(&project.root, &audit_context, &mut report);
    apply_duplication_threshold_gate(&mut report);
    let code = security_summary_run::exit_code(request, &report, regressed);

    match request.format {
        ReportOutputFormat::Human => writer.write_all(render_human_report(&report).as_bytes())?,
        ReportOutputFormat::HtmlOpen => {
            html_open::write_and_open_html_report(&report, &mut writer)?;
        }
        ReportOutputFormat::Html => writer.write_all(render_html_report(&report).as_bytes())?,
        ReportOutputFormat::Json => {
            serde_json::to_writer_pretty(&mut writer, &report)?;
            writeln!(writer)?;
        }
        ReportOutputFormat::Sarif => {
            serde_json::to_writer_pretty(&mut writer, &render_sarif_report(&report))?;
            writeln!(writer)?;
        }
    }

    Ok(code)
}

fn apply_duplication_threshold_gate(report: &mut crate::output::JsonReport) {
    if report.summary.duplication_threshold_exceeded {
        report.verdict = Verdict::Fail;
    }
}

fn command() -> Command {
    let command = Command::new("dart-decimate")
        .about("Rust-native Dart and Flutter module-graph intelligence")
        .subcommand_required(false)
        .arg_required_else_help(false);
    schema_subcommands(support_subcommands(shortcut_subcommands(
        report_subcommands(command),
    )))
}

fn shortcut_subcommands(command: Command) -> Command {
    command
        .subcommand(output_shortcut_command(
            "human",
            "Shortcut for check with readable terminal output",
        ))
        .subcommand(output_shortcut_command(
            "json",
            "Shortcut for check with JSON output",
        ))
        .subcommand(
            output_shortcut_command("html", "Shortcut for check with a browser HTML report").arg(
                Arg::new("stdout")
                    .long("stdout")
                    .help("Print the HTML report instead of opening it in the browser")
                    .action(ArgAction::SetTrue),
            ),
        )
}

fn output_shortcut_command(name: &'static str, about: &'static str) -> Command {
    Command::new(name)
        .about(about)
        .after_help("All check flags can also be passed to this shortcut.")
        .arg(common_args::root_arg())
        .arg(
            Arg::new("format")
                .long("format")
                .value_name("FORMAT")
                .value_parser(common_args::REPORT_FORMAT_VALUES)
                .hide(true),
        )
}

fn report_subcommands(command: Command) -> Command {
    command
        .subcommand(check_issue_filter_command(symbol_options_command(
            dupes_command(health_command_without_top(boundary_command(
                baseline_command(Command::new("check").about("Run all enabled graph checks")),
            ))),
        )))
        .subcommand(symbol_options_command(dupes_command(
            health_command_without_top(boundary_command(report_command(audit_command()))),
        )))
        .subcommand(dead_code_issue_filter_command(symbol_options_command(
            baseline_command(
                Command::new("dead-code").about("Find Dart files unreachable from entry points"),
            ),
        )))
        .subcommand(baseline_command(
            Command::new("cycles").about("Find circular file dependencies"),
        ))
        .subcommand(dupes_command(baseline_command(
            Command::new("dupes").about("Find duplicated Dart code blocks"),
        )))
        .subcommand(health_command(baseline_command(
            Command::new("health").about("Find complex Dart functions and methods"),
        )))
        .subcommand(flags_command(baseline_command(
            Command::new("flags").about("Find Dart and Flutter feature flag patterns"),
        )))
        .subcommand(security_command(baseline_command(
            Command::new("security").about("Find unverified Dart and Flutter security candidates"),
        )))
        .subcommand(trace_file_command(scan_command(
            Command::new("trace-file").about("Trace one Dart file"),
        )))
        .subcommand(trace_command(
            Command::new("trace").about("Trace one top-level Dart symbol"),
        ))
        .subcommand(trace_symbol_command(scan_command(
            Command::new("trace-symbol").about("Trace one top-level Dart symbol"),
        )))
        .subcommand(trace_dependency_command(scan_command(
            Command::new("trace-dependency").about("Trace one pub dependency"),
        )))
        .subcommand(trace_clone_command(dupes_command(scan_command(
            Command::new("trace-clone").about("Trace one duplicate code group"),
        ))))
        .subcommand(inspect_command(scan_command(
            Command::new("inspect").about("Compose one evidence bundle for a file or symbol"),
        )))
}

fn audit_command() -> Command {
    Command::new("audit")
        .about("Run changed-code graph checks")
        .arg(
            Arg::new("brief")
                .long("brief")
                .help("Emit a Fallow-style advisory review brief and always exit 0")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("base")
                .long("base")
                .value_name("REF")
                .help("Git ref used to scope changed files")
                .required(true),
        )
        .arg(audit_run::gate_arg())
        .arg(audit_baseline_arg(
            "dead-code-baseline",
            "Dead-code baseline file",
        ))
        .arg(audit_baseline_arg(
            "health-baseline",
            "Health baseline file",
        ))
        .arg(audit_baseline_arg(
            "dupes-baseline",
            "Duplicate-code baseline file",
        ))
        .arg(max_decisions_arg())
}

fn support_subcommands(command: Command) -> Command {
    command
        .subcommand(list_command())
        .subcommand(workspaces_command())
        .subcommand(explain_command())
        .subcommand(fix_command())
        .subcommand(migrate_command())
        .subcommand(telemetry_command())
        .subcommand(license_command())
        .subcommand(init_command())
        .subcommand(hooks_command())
        .subcommand(setup_hooks_command())
        .subcommand(watch_command())
        .subcommand(impact_command())
        .subcommand(ci_command())
        .subcommand(ci_template_command())
        .subcommand(review_command())
        .subcommand(decision_surface_command())
        .subcommand(coverage_command())
        .subcommand(config_command())
}

fn schema_subcommands(command: Command) -> Command {
    command
        .subcommand(manifest_command())
        .subcommand(config_schema_command())
        .subcommand(report_schema_command())
        .subcommand(rule_pack_schema_command())
}

fn request_from_matches(matches: &ArgMatches) -> Result<CommandRequest, CliError> {
    let Some((name, subcommand)) = matches.subcommand() else {
        unreachable!("clap requires a subcommand");
    };

    let command = report_command_from_name(name);

    let root = root_path(subcommand);
    let config = load_config(&root, subcommand)?;
    let security_cli = security_cli_options(command, subcommand);
    let format = request_format(subcommand, &config, &security_cli)?;
    let entry_points = entry_points(subcommand, &config);
    let production = mode_args::production(subcommand, &config);
    let symbol_options = SymbolRequestOptions {
        include_entry_exports: mode_args::include_entry_exports(subcommand, &config),
        private_type_leaks: mode_args::private_type_leaks(subcommand, &config),
    };
    let mut boundaries = config.boundaries.clone();
    boundaries.extend(boundary_rules(command, subcommand)?);
    let boundary_calls = request_args::boundary_call_rules(command, subcommand, &config)?;
    let policy_packs = request_args::policy_pack_paths(command, subcommand, &config);
    let boundary_coverage = subcommand
        .try_get_one::<bool>("boundary-coverage")
        .ok()
        .flatten()
        .copied()
        .unwrap_or_default()
        || config.boundary_coverage;
    let audit_base = (command == ReportCommand::Audit)
        .then(|| subcommand.get_one::<String>("base").cloned())
        .flatten();
    let file_paths = scope_args::file_paths(command, subcommand);
    let workspace_patterns = scope_args::workspace_patterns(command, subcommand);
    let changed_workspaces = scope_args::changed_workspaces(command, subcommand);
    let changed_since = scope_args::changed_since(command, subcommand);
    let baseline_paths = baseline_paths(command, subcommand);
    let save_baseline = if supports_global_baseline(command) {
        subcommand.get_one::<PathBuf>("save-baseline").cloned()
    } else {
        None
    };
    let regression = regression_request_args(command, subcommand)?;
    let regression_baseline = regression_baseline_path(command, subcommand);
    let save_regression_baseline = save_regression_baseline_path(command, subcommand);
    let trace = trace_request_args(command, subcommand)?;
    let duplicate_options = duplicate_options_for(command, subcommand, &config)?;
    let health_options = health_options_for(command, subcommand, &config);
    let feature_flag_options = feature_flag_options_for(command, subcommand, &config);
    let security_options = security_options_for(command, subcommand, &config);
    let issue_filters = issue_filter_args::issue_filters(command, subcommand);
    validate_security_cli(&security_cli, &security_options)?;
    Ok(CommandRequest {
        command,
        root,
        format,
        entry_points,
        production,
        symbol_options,
        boundaries,
        boundary_coverage,
        boundary_allow_unmatched: config.boundary_allow_unmatched.clone(),
        boundary_calls,
        policy_packs,
        audit_base,
        audit_gate: audit_run::gate(command, subcommand),
        file_paths,
        workspace_patterns,
        changed_workspaces,
        changed_since,
        baseline_paths,
        security_issue_mode: security_cli.issue_mode(),
        security_summary_mode: security_cli.summary_mode(),
        security_sarif_file: security_cli.sarif_file,
        security_gate: security_cli.gate,
        security_diff: security_cli.diff,
        security_changed_since: security_cli.changed_since,
        save_baseline,
        regression_baseline,
        save_regression_baseline,
        regression_tolerance: regression.tolerance,
        fail_on_regression: regression.fail_on_regression,
        trace_file: trace.file,
        trace_symbol: trace.symbol,
        trace_dependency: trace.dependency,
        trace_clone: trace.clone,
        duplicate_options,
        health_options,
        feature_flag_options,
        security_options,
        issue_filters,
        scan_options: ScanOptions {
            ignore_patterns: config.ignore_patterns.clone(),
            conditional_environment: common_args::conditional_environment(subcommand),
        },
        ignore_dependencies: config.ignore_dependencies.clone(),
        ignore_dependency_overrides: config.ignore_dependency_overrides.clone(),
        rules: config.rules.clone(),
    })
}

fn request_format(
    subcommand: &ArgMatches,
    config: &DartDecimateConfig,
    security_cli: &SecurityCliOptions,
) -> Result<ReportOutputFormat, CliError> {
    let open_html = subcommand
        .try_get_one::<bool>("open")
        .ok()
        .flatten()
        .copied()
        .unwrap_or_default();
    let explicit_format = subcommand.value_source("format") == Some(ValueSource::CommandLine);
    let format = if security_cli.ci {
        ReportOutputFormat::Sarif
    } else {
        report_output_format(subcommand, config)
    };

    if !open_html {
        return Ok(format);
    }
    if security_cli.ci || (explicit_format && format != ReportOutputFormat::Html) {
        return Err(CliError::HtmlOpenRequiresHtml);
    }
    Ok(ReportOutputFormat::HtmlOpen)
}

fn validate_security_cli(
    security_cli: &SecurityCliOptions,
    security_options: &SecurityOptions,
) -> Result<(), CliError> {
    if security_options.top.is_some()
        && (security_cli.gate.is_some()
            || security_cli.diff.is_some()
            || security_cli.changed_since.is_some())
    {
        return Err(CliError::UnsupportedSecurityTopScope);
    }
    Ok(())
}

fn baseline_paths(command: ReportCommand, subcommand: &ArgMatches) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if supports_global_baseline(command) {
        if let Some(path) = subcommand.get_one::<PathBuf>("baseline") {
            paths.push(path.clone());
        }
    }
    if command == ReportCommand::Audit {
        for id in ["dead-code-baseline", "health-baseline", "dupes-baseline"] {
            if let Some(path) = subcommand.get_one::<PathBuf>(id) {
                paths.push(path.clone());
            }
        }
    }
    paths
}

fn supports_global_baseline(command: ReportCommand) -> bool {
    matches!(
        command,
        ReportCommand::Check
            | ReportCommand::DeadCode
            | ReportCommand::Cycles
            | ReportCommand::Dupes
            | ReportCommand::Health
            | ReportCommand::Flags
            | ReportCommand::Security
    )
}

fn resolve_report_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn load_config(root: &Path, subcommand: &ArgMatches) -> Result<DartDecimateConfig, ConfigError> {
    load_dart_decimate_config(
        root,
        subcommand
            .get_one::<PathBuf>("config")
            .map(std::path::PathBuf::as_path),
    )
}

fn entry_points(subcommand: &ArgMatches, config: &DartDecimateConfig) -> Vec<PathBuf> {
    if subcommand.value_source("entry") == Some(ValueSource::CommandLine) {
        return subcommand
            .get_many::<PathBuf>("entry")
            .map(|values| values.cloned().collect())
            .unwrap_or_default();
    }
    config.entry_points.clone()
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod trace_tests;
