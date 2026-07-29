use std::path::PathBuf;

use clap::{Arg, ArgAction, ArgMatches, Command, parser::ValueSource, value_parser};

use crate::{HealthOptions, LowTrafficThreshold};

pub(super) fn health_command(command: Command) -> Command {
    health_command_without_top(command)
        .arg(
            Arg::new("top")
                .long("top")
                .value_name("N")
                .help("Show only the N highest complexity findings")
                .value_parser(value_parser!(usize)),
        )
        .arg(
            Arg::new("flutter-style")
                .long("flutter-style")
                .help("Include advisory Flutter theme/style analysis")
                .action(ArgAction::SetTrue),
        )
}

pub(super) fn health_command_without_top(command: Command) -> Command {
    let command = command
        .arg(
            Arg::new("max-cyclomatic")
                .long("max-cyclomatic")
                .value_name("N")
                .help("Maximum cyclomatic complexity before reporting")
                .default_value("20")
                .value_parser(value_parser!(usize)),
        )
        .arg(
            Arg::new("max-cognitive")
                .long("max-cognitive")
                .value_name("N")
                .help("Maximum cognitive complexity before reporting")
                .default_value("15")
                .value_parser(value_parser!(usize)),
        )
        .arg(
            Arg::new("max-unit-size")
                .long("max-unit-size")
                .value_name("N")
                .help("Maximum inclusive physical lines in one function-like declaration")
                .default_value("60")
                .value_parser(value_parser!(usize)),
        )
        .arg(
            Arg::new("complexity-breakdown")
                .long("complexity-breakdown")
                .help("Include per-decision-point complexity contributions")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("coverage")
                .long("coverage")
                .value_name("PATH")
                .help("LCOV file for coverage-aware health checks")
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("coverage-gaps")
                .long("coverage-gaps")
                .help("Report Dart files with no covered executable lines in LCOV")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("max-crap")
                .long("max-crap")
                .value_name("N")
                .help("Maximum CRAP score before reporting")
                .value_parser(value_parser!(usize)),
        );
    runtime_coverage_args(command, false)
        .arg(
            Arg::new("file-scores")
                .long("file-scores")
                .help("Include per-file health scores")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("hotspots")
                .long("hotspots")
                .help("Report low-scoring file health hotspots")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("targets")
                .long("targets")
                .help("Report prioritized refactoring targets")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("ownership")
                .long("ownership")
                .help("Attach CODEOWNERS ownership metadata to health output")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("min-score")
                .long("min-score")
                .value_name("N")
                .help("Minimum file health score before hotspot reporting")
                .value_parser(value_parser!(usize)),
        )
}

pub(super) fn runtime_coverage_args(command: Command, required: bool) -> Command {
    command
        .arg(
            Arg::new("runtime-coverage")
                .long("runtime-coverage")
                .value_name("PATH")
                .help("V8 coverage JSON/directory or Istanbul coverage-final.json")
                .required(required)
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("min-invocations-hot")
                .long("min-invocations-hot")
                .value_name("N")
                .help("Minimum runtime invocations before a file is a hot path")
                .default_value("100")
                .value_parser(value_parser!(usize)),
        )
        .arg(
            Arg::new("min-observation-volume")
                .long("min-observation-volume")
                .value_name("N")
                .help("Minimum runtime observations for high-confidence signals")
                .default_value("5000")
                .value_parser(value_parser!(usize)),
        )
        .arg(
            Arg::new("low-traffic-threshold")
                .long("low-traffic-threshold")
                .value_name("FRACTION")
                .help("Runtime traffic fraction considered low traffic")
                .default_value("0.001")
                .value_parser(value_parser!(f64)),
        )
}

pub(super) fn health_options_with_defaults(
    matches: &ArgMatches,
    mut options: HealthOptions,
) -> HealthOptions {
    if is_command_line(matches, "max-cyclomatic") {
        if let Some(max_cyclomatic) = matches.get_one::<usize>("max-cyclomatic") {
            options.max_cyclomatic = *max_cyclomatic;
        }
    }
    if is_command_line(matches, "max-cognitive") {
        if let Some(max_cognitive) = matches.get_one::<usize>("max-cognitive") {
            options.max_cognitive = *max_cognitive;
        }
    }
    if is_command_line(matches, "max-unit-size") {
        if let Some(max_unit_size) = matches.get_one::<usize>("max-unit-size") {
            options.max_unit_size = *max_unit_size;
        }
    }
    if is_command_line(matches, "top") {
        options.top = matches.get_one::<usize>("top").copied();
    }
    if is_command_line(matches, "complexity-breakdown") {
        options.complexity_breakdown = matches.get_flag("complexity-breakdown").into();
    }
    if is_command_line(matches, "coverage") {
        options.coverage_path = matches.get_one::<PathBuf>("coverage").cloned();
    }
    if is_command_line(matches, "coverage-gaps") {
        options.coverage_gaps = matches.get_flag("coverage-gaps").into();
    }
    if is_command_line(matches, "max-crap") {
        options.max_crap = matches.get_one::<usize>("max-crap").copied();
    }
    if is_command_line(matches, "runtime-coverage") {
        options.runtime_coverage_path = matches.get_one::<PathBuf>("runtime-coverage").cloned();
    }
    if is_command_line(matches, "min-invocations-hot") {
        if let Some(min_invocations_hot) = matches.get_one::<usize>("min-invocations-hot") {
            options.min_invocations_hot = (*min_invocations_hot).max(1);
        }
    }
    if is_command_line(matches, "min-observation-volume") {
        if let Some(min_observation_volume) = matches.get_one::<usize>("min-observation-volume") {
            options.min_observation_volume = (*min_observation_volume).max(1);
        }
    }
    if is_command_line(matches, "low-traffic-threshold") {
        if let Some(low_traffic_threshold) = matches.get_one::<f64>("low-traffic-threshold") {
            options.low_traffic_threshold = LowTrafficThreshold::from_ratio(*low_traffic_threshold);
        }
    }
    if is_command_line(matches, "file-scores") {
        options.file_scores = matches.get_flag("file-scores").into();
    }
    if is_command_line(matches, "hotspots") {
        options.hotspots = matches.get_flag("hotspots").into();
    }
    if is_command_line(matches, "targets") {
        options.targets = matches.get_flag("targets").into();
    }
    if is_command_line(matches, "ownership") {
        options.ownership = matches.get_flag("ownership").into();
    }
    if matches
        .try_get_one::<bool>("flutter-style")
        .ok()
        .flatten()
        .copied()
        .unwrap_or_default()
    {
        options.flutter_style = true.into();
    }
    if is_command_line(matches, "min-score") {
        if let Some(min_score) = matches.get_one::<usize>("min-score") {
            options.min_score = (*min_score).min(100);
        }
    }
    options
}

fn is_command_line(matches: &ArgMatches, id: &str) -> bool {
    matches.value_source(id) == Some(ValueSource::CommandLine)
}
