use std::path::PathBuf;

use clap::ArgMatches;

use crate::output::ReportCommand;
use crate::{BoundaryCallRule, BoundaryRule, DartDecimateConfig};

use super::{CliError, TraceSymbolSpec};

pub(super) struct TraceRequestArgs {
    pub(super) file: Option<PathBuf>,
    pub(super) symbol: Option<TraceSymbolSpec>,
    pub(super) dependency: Option<String>,
    pub(super) clone: Option<String>,
}

pub(super) fn boundary_rules(
    command: ReportCommand,
    subcommand: &ArgMatches,
) -> Result<Vec<BoundaryRule>, CliError> {
    if !matches!(command, ReportCommand::Check | ReportCommand::Audit) {
        return Ok(Vec::new());
    }

    subcommand
        .get_many::<String>("boundary")
        .into_iter()
        .flatten()
        .map(|value| parse_boundary_rule(value))
        .collect()
}

pub(super) fn boundary_call_rules(
    command: ReportCommand,
    subcommand: &ArgMatches,
    config: &DartDecimateConfig,
) -> Result<Vec<BoundaryCallRule>, CliError> {
    let mut rules = config.boundary_calls.clone();
    if !matches!(command, ReportCommand::Check | ReportCommand::Audit) {
        return Ok(rules);
    }

    rules.extend(cli_boundary_call_rules(subcommand)?);
    Ok(rules)
}

pub(super) fn policy_pack_paths(
    command: ReportCommand,
    subcommand: &ArgMatches,
    config: &DartDecimateConfig,
) -> Vec<PathBuf> {
    let mut paths = config.policy_packs.clone();
    if matches!(command, ReportCommand::Check | ReportCommand::Audit)
        && let Some(values) = subcommand.get_many::<PathBuf>("policy-pack")
    {
        paths.extend(values.cloned());
    }
    paths
}

fn cli_boundary_call_rules(subcommand: &ArgMatches) -> Result<Vec<BoundaryCallRule>, CliError> {
    subcommand
        .get_many::<String>("boundary-call")
        .into_iter()
        .flatten()
        .map(|value| parse_boundary_call_rule(value))
        .collect()
}

pub(super) fn trace_request_args(
    command: ReportCommand,
    subcommand: &ArgMatches,
) -> Result<TraceRequestArgs, CliError> {
    let inspect_symbol =
        command == ReportCommand::Inspect && subcommand.get_one::<String>("symbol").is_some();
    let trace_file = if command == ReportCommand::TraceFile
        || (command == ReportCommand::Inspect && !inspect_symbol)
    {
        subcommand.get_one::<PathBuf>("file").cloned()
    } else {
        None
    };
    let trace_symbol = if matches!(command, ReportCommand::TraceSymbol | ReportCommand::Inspect)
        && (command == ReportCommand::TraceSymbol || inspect_symbol)
    {
        let file = subcommand
            .try_get_one::<PathBuf>("file")
            .ok()
            .flatten()
            .cloned();
        subcommand
            .get_one::<String>("symbol")
            .map(|value| parse_trace_symbol(file, value))
            .transpose()?
    } else {
        None
    };
    let trace_dependency = if command == ReportCommand::TraceDependency {
        subcommand.get_one::<String>("dependency").cloned()
    } else {
        None
    };
    let trace_clone = if command == ReportCommand::TraceClone {
        subcommand.get_one::<String>("fingerprint").cloned()
    } else {
        None
    };
    if command == ReportCommand::Inspect && trace_file.is_none() && trace_symbol.is_none() {
        return Err(CliError::MissingInspectTarget);
    }

    Ok(TraceRequestArgs {
        file: trace_file,
        symbol: trace_symbol,
        dependency: trace_dependency,
        clone: trace_clone,
    })
}

fn parse_trace_symbol(file: Option<PathBuf>, value: &str) -> Result<TraceSymbolSpec, CliError> {
    if let Some(file) = file {
        if value.is_empty() || value.contains(':') {
            return Err(CliError::TraceSymbol {
                value: value.to_owned(),
            });
        }
        return Ok(TraceSymbolSpec {
            file,
            symbol: value.to_owned(),
        });
    }

    let Some((file, symbol)) = value.rsplit_once(':') else {
        return Err(CliError::TraceSymbol {
            value: value.to_owned(),
        });
    };
    if file.is_empty() || symbol.is_empty() {
        return Err(CliError::TraceSymbol {
            value: value.to_owned(),
        });
    }
    Ok(TraceSymbolSpec {
        file: PathBuf::from(file),
        symbol: symbol.to_owned(),
    })
}

fn parse_boundary_rule(value: &str) -> Result<BoundaryRule, CliError> {
    let Some((from, disallow)) = value.split_once(':') else {
        return Err(CliError::BoundaryRule {
            value: value.to_owned(),
        });
    };
    if from.is_empty() || disallow.is_empty() {
        return Err(CliError::BoundaryRule {
            value: value.to_owned(),
        });
    }
    Ok(BoundaryRule::new(from, disallow))
}

fn parse_boundary_call_rule(value: &str) -> Result<BoundaryCallRule, CliError> {
    let Some((from, pattern)) = value.split_once(':') else {
        return Err(CliError::BoundaryCallRule {
            value: value.to_owned(),
        });
    };
    if from.is_empty() || pattern.is_empty() {
        return Err(CliError::BoundaryCallRule {
            value: value.to_owned(),
        });
    }
    Ok(BoundaryCallRule::new(from, vec![pattern.to_owned()]))
}
