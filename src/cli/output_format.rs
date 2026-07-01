use clap::{ArgMatches, parser::ValueSource};

use crate::config::{ConfigOutputFormat, DartDecimateConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReportOutputFormat {
    Human,
    Html,
    HtmlOpen,
    Json,
    Sarif,
}

pub(super) fn output_format(subcommand: &ArgMatches, config: &DartDecimateConfig) -> OutputFormat {
    if subcommand.value_source("format") == Some(ValueSource::CommandLine) {
        return output_format_value(subcommand);
    }

    match config.output_format {
        Some(ConfigOutputFormat::Json) => OutputFormat::Json,
        Some(ConfigOutputFormat::Human) | None => output_format_value(subcommand),
    }
}

pub(super) fn output_format_value(subcommand: &ArgMatches) -> OutputFormat {
    match subcommand
        .get_one::<String>("format")
        .map_or("human", String::as_str)
    {
        "json" => OutputFormat::Json,
        _ => OutputFormat::Human,
    }
}

pub(super) fn report_output_format(
    subcommand: &ArgMatches,
    config: &DartDecimateConfig,
) -> ReportOutputFormat {
    if subcommand.value_source("format") == Some(ValueSource::CommandLine) {
        return report_output_format_value(subcommand);
    }

    match config.output_format {
        Some(ConfigOutputFormat::Json) => ReportOutputFormat::Json,
        Some(ConfigOutputFormat::Human) | None => report_output_format_value(subcommand),
    }
}

fn report_output_format_value(subcommand: &ArgMatches) -> ReportOutputFormat {
    match subcommand
        .get_one::<String>("format")
        .map_or("human", String::as_str)
    {
        "html" => ReportOutputFormat::Html,
        "json" => ReportOutputFormat::Json,
        "sarif" => ReportOutputFormat::Sarif,
        _ => ReportOutputFormat::Human,
    }
}
