use std::io::Write;
use std::path::{Path, PathBuf};

use clap::{Arg, ArgAction, ArgMatches, Command, parser::ValueSource, value_parser};

use crate::coverage::{
    coverage_analysis_report, coverage_inventory_upload_report, coverage_setup_report,
    coverage_source_maps_upload_report, render_coverage_analysis_report,
    render_coverage_setup_report, render_coverage_upload_report,
};
use crate::output::json_runtime_coverage;
use crate::scan::{ScanOptions, scan_project_with_options};
use crate::{HealthOptions, LowTrafficThreshold};

use super::common_args::{config_arg, format_arg, root_arg, root_flag_arg, root_path};
use super::health_args::runtime_coverage_args;
use super::{CliError, OutputFormat, load_config, output_format};

pub(super) fn coverage_command() -> Command {
    Command::new("coverage")
        .about("Runtime coverage setup and analysis")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(coverage_setup_command())
        .subcommand(coverage_analyze_command())
        .subcommand(coverage_upload_inventory_command())
        .subcommand(coverage_upload_source_maps_command())
}

pub(super) fn run_coverage<W: Write>(subcommand: &ArgMatches, writer: W) -> Result<i32, CliError> {
    match subcommand.subcommand() {
        Some(("setup", setup)) => run_coverage_setup(setup, writer),
        Some(("analyze", analyze)) => run_coverage_analyze(analyze, writer),
        Some(("upload-inventory", upload)) => run_coverage_upload_inventory(upload, writer),
        Some(("upload-source-maps", upload)) => run_coverage_upload_source_maps(upload, writer),
        _ => unreachable!("clap requires a coverage subcommand"),
    }
}

fn coverage_setup_command() -> Command {
    Command::new("setup")
        .about("Plan or write local runtime coverage defaults")
        .arg(root_arg())
        .arg(root_flag_arg())
        .arg(format_arg())
        .arg(config_arg())
        .arg(
            Arg::new("yes")
                .long("yes")
                .help("Write safe local coverage defaults without prompting")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("non-interactive")
                .long("non-interactive")
                .help("Do not prompt; emit a deterministic setup plan")
                .action(ArgAction::SetTrue),
        )
}

fn coverage_analyze_command() -> Command {
    runtime_coverage_args(
        Command::new("analyze")
            .about("Analyze local V8 or Istanbul runtime coverage")
            .arg(root_arg())
            .arg(root_flag_arg())
            .arg(format_arg())
            .arg(config_arg())
            .arg(
                Arg::new("cloud")
                    .long("cloud")
                    .help("Cloud runtime coverage is not supported yet")
                    .action(ArgAction::SetTrue),
            ),
        false,
    )
    .arg(repo_arg(false))
    .arg(
        Arg::new("top")
            .long("top")
            .value_name("N")
            .help("Show only the N highest runtime hot paths and findings")
            .value_parser(value_parser!(usize)),
    )
}

fn coverage_upload_inventory_command() -> Command {
    Command::new("upload-inventory")
        .about("Build a local source inventory upload dry-run packet")
        .arg(root_arg())
        .arg(root_flag_arg())
        .arg(format_arg())
        .arg(config_arg())
        .arg(repo_arg(false))
        .arg(dry_run_arg())
}

fn coverage_upload_source_maps_command() -> Command {
    Command::new("upload-source-maps")
        .about("Build a source-map upload dry-run packet")
        .arg(root_arg())
        .arg(root_flag_arg())
        .arg(format_arg())
        .arg(config_arg())
        .arg(
            Arg::new("dir")
                .long("dir")
                .value_name("PATH")
                .help("Directory containing source-map files")
                .required(true)
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("git-sha")
                .long("git-sha")
                .value_name("SHA")
                .help("Git commit SHA for the uploaded source maps")
                .required(true)
                .value_parser(value_parser!(String)),
        )
        .arg(repo_arg(true))
        .arg(
            Arg::new("strip-path")
                .long("strip-path")
                .value_name("BOOL")
                .help("Strip project-root prefixes from uploaded source-map paths")
                .default_value("true")
                .default_missing_value("true")
                .num_args(0..=1)
                .value_parser(value_parser!(bool)),
        )
        .arg(dry_run_arg())
}

fn run_coverage_setup<W: Write>(subcommand: &ArgMatches, mut writer: W) -> Result<i32, CliError> {
    let root = root_path(subcommand);
    let config = load_config(&root, subcommand)?;
    let format = output_format(subcommand, &config);
    let project = scan_project_with_options(
        &root,
        &ScanOptions {
            ignore_patterns: config.ignore_patterns.clone(),
            ..ScanOptions::default()
        },
    )?;
    let report = coverage_setup_report(
        &project,
        subcommand.get_flag("yes"),
        subcommand.get_flag("non-interactive"),
        config.health.coverage_path.is_some() || config.health.runtime_coverage.is_some(),
    )?;

    match format {
        OutputFormat::Human => {
            writer.write_all(render_coverage_setup_report(&report).as_bytes())?;
        }
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut writer, &report)?;
            writeln!(writer)?;
        }
    }

    Ok(0)
}

fn run_coverage_analyze<W: Write>(subcommand: &ArgMatches, mut writer: W) -> Result<i32, CliError> {
    if subcommand.get_flag("cloud") {
        return Err(CliError::UnsupportedCoverageCloud);
    }
    if let Some(repo) = subcommand.get_one::<String>("repo") {
        validate_repo(repo)?;
    }
    let root = root_path(subcommand);
    require_runtime_coverage(subcommand)?;
    let config = load_config(&root, subcommand)?;
    let format = output_format(subcommand, &config);
    let health_options = coverage_health_options(subcommand, config.health_options());

    let project = scan_project_with_options(
        &root,
        &ScanOptions {
            ignore_patterns: config.ignore_patterns.clone(),
            ..ScanOptions::default()
        },
    )?;
    let mut health = crate::analyze_health(&project, &health_options)?;
    let Some(runtime) = health.runtime_coverage.as_mut() else {
        return Err(CliError::MissingRuntimeCoverage);
    };
    if let Some(top) = health_options.top {
        runtime.hot_paths.truncate(top);
        runtime.findings.truncate(top);
        runtime.coverage_intelligence.truncate(top);
        runtime.blast_radius.truncate(top);
        runtime.importance.truncate(top);
    }
    let report = coverage_analysis_report(json_runtime_coverage(&project.root, runtime));

    match format {
        OutputFormat::Human => {
            writer.write_all(render_coverage_analysis_report(&report).as_bytes())?;
        }
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut writer, &report)?;
            writeln!(writer)?;
        }
    }

    Ok(0)
}

fn run_coverage_upload_inventory<W: Write>(
    subcommand: &ArgMatches,
    mut writer: W,
) -> Result<i32, CliError> {
    require_dry_run(subcommand, "coverage upload-inventory")?;
    if let Some(repo) = subcommand.get_one::<String>("repo") {
        validate_repo(repo)?;
    }
    let root = root_path(subcommand);
    let config = load_config(&root, subcommand)?;
    let format = output_format(subcommand, &config);
    let project = scan_project_with_options(
        &root,
        &ScanOptions {
            ignore_patterns: config.ignore_patterns.clone(),
            ..ScanOptions::default()
        },
    )?;
    let report = coverage_inventory_upload_report(
        &project,
        subcommand.get_one::<String>("repo").cloned(),
        subcommand.get_flag("dry-run"),
    );
    match format {
        OutputFormat::Human => {
            writer.write_all(render_coverage_upload_report(&report).as_bytes())?;
        }
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut writer, &report)?;
            writeln!(writer)?;
        }
    }
    Ok(0)
}

fn run_coverage_upload_source_maps<W: Write>(
    subcommand: &ArgMatches,
    mut writer: W,
) -> Result<i32, CliError> {
    require_dry_run(subcommand, "coverage upload-source-maps")?;
    let root = root_path(subcommand);
    let config = load_config(&root, subcommand)?;
    let format = output_format(subcommand, &config);
    let repo = subcommand
        .get_one::<String>("repo")
        .cloned()
        .unwrap_or_default();
    validate_repo(&repo)?;
    let git_sha = subcommand
        .get_one::<String>("git-sha")
        .cloned()
        .unwrap_or_default();
    validate_git_sha(&git_sha)?;
    let Some(dir_arg) = subcommand.get_one::<PathBuf>("dir") else {
        return Err(CliError::CoverageUploadDir {
            path: PathBuf::from("<missing>"),
        });
    };
    let dir = resolve_path(&root, dir_arg);
    if !dir.is_dir() {
        return Err(CliError::CoverageUploadDir { path: dir });
    }
    let strip_path = subcommand
        .get_one::<bool>("strip-path")
        .copied()
        .unwrap_or(true);
    let report = coverage_source_maps_upload_report(
        &root,
        &dir,
        repo,
        git_sha,
        strip_path,
        subcommand.get_flag("dry-run"),
    )?;
    match format {
        OutputFormat::Human => {
            writer.write_all(render_coverage_upload_report(&report).as_bytes())?;
        }
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut writer, &report)?;
            writeln!(writer)?;
        }
    }
    Ok(0)
}

fn require_runtime_coverage(subcommand: &ArgMatches) -> Result<(), CliError> {
    if subcommand.get_one::<PathBuf>("runtime-coverage").is_some() {
        Ok(())
    } else {
        Err(CliError::MissingRuntimeCoverage)
    }
}

fn coverage_health_options(subcommand: &ArgMatches, mut options: HealthOptions) -> HealthOptions {
    options.coverage_path = None;
    options.coverage_gaps = false.into();
    options.max_crap = None;
    options.threshold_overrides.clear();
    options.file_scores = false.into();
    options.hotspots = false.into();
    options.targets = false.into();
    options.ownership = false.into();

    options.runtime_coverage_path = subcommand.get_one::<PathBuf>("runtime-coverage").cloned();
    if subcommand.value_source("min-invocations-hot") == Some(ValueSource::CommandLine)
        && let Some(value) = subcommand.get_one::<usize>("min-invocations-hot")
    {
        options.min_invocations_hot = (*value).max(1);
    }
    if subcommand.value_source("min-observation-volume") == Some(ValueSource::CommandLine)
        && let Some(value) = subcommand.get_one::<usize>("min-observation-volume")
    {
        options.min_observation_volume = (*value).max(1);
    }
    if subcommand.value_source("low-traffic-threshold") == Some(ValueSource::CommandLine)
        && let Some(value) = subcommand.get_one::<f64>("low-traffic-threshold")
    {
        options.low_traffic_threshold = LowTrafficThreshold::from_ratio(*value);
    }
    if let Some(top) = subcommand.get_one::<usize>("top") {
        options.top = Some(*top);
    }
    options
}

fn resolve_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn dry_run_arg() -> Arg {
    Arg::new("dry-run")
        .long("dry-run")
        .help("Build the upload packet without network writes")
        .action(ArgAction::SetTrue)
}

fn repo_arg(required: bool) -> Arg {
    Arg::new("repo")
        .long("repo")
        .value_name("OWNER/REPO")
        .help("Repository slug for cloud runtime coverage metadata")
        .required(required)
        .value_parser(value_parser!(String))
}

fn require_dry_run(subcommand: &ArgMatches, command: &'static str) -> Result<(), CliError> {
    if subcommand.get_flag("dry-run") {
        Ok(())
    } else {
        Err(CliError::CoverageUploadDryRunRequired { command })
    }
}

fn validate_repo(repo: &str) -> Result<(), CliError> {
    let mut parts = repo.split('/');
    let valid = matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some(owner), Some(name), None) if !owner.is_empty() && !name.is_empty()
    );
    if valid {
        Ok(())
    } else {
        Err(CliError::CoverageUploadRepo {
            value: repo.to_owned(),
        })
    }
}

fn validate_git_sha(value: &str) -> Result<(), CliError> {
    if (7..=40).contains(&value.len()) && value.chars().all(|char| char.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(CliError::CoverageUploadGitSha {
            value: value.to_owned(),
        })
    }
}
