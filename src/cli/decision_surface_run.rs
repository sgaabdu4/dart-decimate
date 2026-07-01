use std::io::Write;

use clap::{Arg, ArgMatches, Command, parser::ValueSource, value_parser};

use crate::changed_scope::changed_files;
use crate::decision_surface::{
    decision_surface_report_for_command, render_decision_surface_report,
};
use crate::output::render_decision_surface_html_report;
use crate::scan::{ScanOptions, scan_project_with_options};

use super::common_args::{config_arg, format_arg, root_arg, root_flag_arg, root_path};
use super::{CliError, ReportOutputFormat, html_open};

pub(super) fn decision_surface_command() -> Command {
    decision_surface_command_named(
        "decision-surface",
        "Surface changed-code structural decisions for review",
    )
}

pub(super) fn review_command() -> Command {
    decision_surface_command_named(
        "review",
        "Review changed-code structural decisions without failing CI",
    )
}

pub(super) fn max_decisions_arg() -> Arg {
    Arg::new("max-decisions")
        .long("max-decisions")
        .value_name("N")
        .help("Maximum decisions to emit")
        .default_value("5")
        .value_parser(value_parser!(usize))
}

fn decision_surface_command_named(name: &'static str, about: &'static str) -> Command {
    Command::new(name)
        .about(about)
        .arg(root_arg())
        .arg(root_flag_arg())
        .arg(format_arg())
        .arg(config_arg())
        .arg(
            Arg::new("base")
                .long("base")
                .value_name("REF")
                .help("Git ref used to scope changed files")
                .required(true),
        )
        .arg(max_decisions_arg())
}

pub(super) fn run_decision_surface<W: Write>(
    subcommand: &ArgMatches,
    mut writer: W,
    command: &'static str,
) -> Result<i32, CliError> {
    run_decision_surface_with_opener(subcommand, &mut writer, command, html_open::open_url)
}

fn run_decision_surface_with_opener<W, F>(
    subcommand: &ArgMatches,
    writer: &mut W,
    command: &'static str,
    opener: F,
) -> Result<i32, CliError>
where
    W: Write,
    F: FnOnce(&str) -> std::io::Result<()>,
{
    let root = root_path(subcommand);
    let config = super::load_config(&root, subcommand)?;
    let project = scan_project_with_options(
        &root,
        &ScanOptions {
            ignore_patterns: config.ignore_patterns.clone(),
            ..ScanOptions::default()
        },
    )?;
    let Some(base) = subcommand.get_one::<String>("base") else {
        unreachable!("clap requires --base for decision-surface");
    };
    let changed = changed_files(&project.root, base)?;
    let max_decisions = subcommand
        .get_one::<usize>("max-decisions")
        .copied()
        .unwrap_or(5);
    let report =
        decision_surface_report_for_command(&project, base, &changed, max_decisions, command);

    match decision_surface_output_format(subcommand, command)? {
        ReportOutputFormat::Human => {
            writer.write_all(render_decision_surface_report(&report).as_bytes())?;
        }
        ReportOutputFormat::HtmlOpen => {
            html_open::write_and_open_html_document_with(
                &report.command,
                &render_decision_surface_html_report(&report),
                writer,
                opener,
            )?;
        }
        ReportOutputFormat::Html => {
            writer.write_all(render_decision_surface_html_report(&report).as_bytes())?;
        }
        ReportOutputFormat::Json => {
            serde_json::to_writer_pretty(&mut *writer, &report)?;
            writeln!(writer)?;
        }
        ReportOutputFormat::Sarif => return Err(CliError::UnsupportedSarifFormat { command }),
    }
    Ok(0)
}

fn decision_surface_output_format(
    subcommand: &ArgMatches,
    command: &'static str,
) -> Result<ReportOutputFormat, CliError> {
    let format = match subcommand
        .get_one::<String>("format")
        .map_or("human", String::as_str)
    {
        "html" => ReportOutputFormat::Html,
        "json" => ReportOutputFormat::Json,
        "sarif" => ReportOutputFormat::Sarif,
        _ => ReportOutputFormat::Human,
    };
    let open_html = subcommand
        .try_get_one::<bool>("open")
        .ok()
        .flatten()
        .copied()
        .unwrap_or_default();
    if !open_html {
        return Ok(format);
    }

    let explicit_format = subcommand.value_source("format") == Some(ValueSource::CommandLine);
    if explicit_format && format != ReportOutputFormat::Html {
        return Err(CliError::HtmlOpenRequiresHtml);
    }
    if format == ReportOutputFormat::Sarif {
        return Err(CliError::UnsupportedSarifFormat { command });
    }
    Ok(ReportOutputFormat::HtmlOpen)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;
    use std::process::Command;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn audit_brief_open_writes_html_and_opens_file_url() -> Result<(), Box<dyn std::error::Error>> {
        let fixture = changed_dependency_fixture()?;
        let matches = super::super::command().try_get_matches_from([
            "dart-decimate",
            "audit",
            fixture.path().to_str().unwrap_or("."),
            "--base",
            "HEAD",
            "--brief",
            "--open",
        ])?;
        let Some(("audit", subcommand)) = matches.subcommand() else {
            panic!("audit subcommand");
        };
        let opened = RefCell::new(String::new());
        let mut output = Vec::new();

        let code =
            run_decision_surface_with_opener(subcommand, &mut output, "audit --brief", |url| {
                opened.replace(url.to_owned());
                Ok(())
            })?;

        let opened = opened.into_inner();
        let path = opened.strip_prefix("file://").unwrap_or(&opened);
        let html = fs::read_to_string(path)?;
        let message = String::from_utf8(output)?;
        assert_eq!(code, 0);
        assert!(opened.starts_with("file:///"));
        assert!(message.contains("Opened HTML report: file:///"));
        assert!(html.contains("<h1>audit --brief decision surface</h1>"));
        assert!(html.contains("pubspec.yaml"));
        Ok(())
    }

    fn changed_dependency_fixture() -> Result<TempDir, Box<dyn std::error::Error>> {
        let fixture = tempfile::tempdir()?;
        git(&fixture, ["init", "-q"])?;
        git(
            &fixture,
            ["config", "user.email", "dart-decimate@example.com"],
        )?;
        git(&fixture, ["config", "user.name", "Dart Decimate Tests"])?;
        write(&fixture, "pubspec.yaml", "name: app\n")?;
        write(&fixture, "lib/main.dart", "void main() {}\n")?;
        git(&fixture, ["add", "."])?;
        git(&fixture, ["commit", "-m", "baseline", "-q"])?;
        write(
            &fixture,
            "pubspec.yaml",
            "name: app\ndependencies:\n  http: ^1.0.0\n",
        )?;
        Ok(fixture)
    }

    fn git<const N: usize>(
        fixture: &TempDir,
        args: [&str; N],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let output = Command::new("git")
            .args(args)
            .current_dir(fixture.path())
            .output()?;
        if output.status.success() {
            return Ok(());
        }
        Err(String::from_utf8_lossy(&output.stderr).to_string().into())
    }

    fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
        let path = fixture.path().join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, source)
    }
}
