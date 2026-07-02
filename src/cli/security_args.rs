use std::path::PathBuf;

use clap::{Arg, ArgAction, ArgMatches, Command, parser::ValueSource, value_parser};

use crate::ReportCommand;
use crate::SecurityOptions;
use crate::security_gate::{SecurityDiffSource, SecurityGateMode};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct SecurityCliOptions {
    pub(super) sarif_file: Option<PathBuf>,
    pub(super) gate: Option<SecurityGateMode>,
    pub(super) diff: Option<SecurityDiffSource>,
    pub(super) changed_since: Option<String>,
    pub(super) ci: bool,
    pub(super) fail_on_issues: bool,
    pub(super) summary: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum SecurityIssueMode {
    #[default]
    VerdictOnly,
    FailOnIssues,
}

impl SecurityIssueMode {
    pub(super) const fn fails_on_issues(self) -> bool {
        matches!(self, Self::FailOnIssues)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum SecuritySummaryMode {
    #[default]
    Full,
    CountsOnly,
}

impl SecuritySummaryMode {
    pub(super) const fn is_counts_only(self) -> bool {
        matches!(self, Self::CountsOnly)
    }
}

impl SecurityCliOptions {
    pub(super) const fn issue_mode(&self) -> SecurityIssueMode {
        if self.fail_on_issues || self.ci {
            SecurityIssueMode::FailOnIssues
        } else {
            SecurityIssueMode::VerdictOnly
        }
    }

    pub(super) const fn summary_mode(&self) -> SecuritySummaryMode {
        if self.summary {
            SecuritySummaryMode::CountsOnly
        } else {
            SecuritySummaryMode::Full
        }
    }
}

pub(super) fn security_command(command: Command) -> Command {
    command
        .arg(
            Arg::new("top")
                .long("top")
                .value_name("N")
                .help("Show only the N most frequent security candidate groups")
                .value_parser(value_parser!(usize)),
        )
        .arg(
            Arg::new("surface")
                .long("surface")
                .help("Include attack-surface inventory entries")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("sarif-file")
                .long("sarif-file")
                .value_name("PATH")
                .help("Write security SARIF output to a file")
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("ci")
                .long("ci")
                .help("Emit SARIF and fail when security candidates are found")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("fail-on-issues")
                .long("fail-on-issues")
                .help("Exit with code 1 when security candidates are found")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("gate")
                .long("gate")
                .value_name("MODE")
                .help("Gate security output to changed candidates")
                .value_parser(["new", "newly-reachable"]),
        )
        .arg(
            Arg::new("diff-file")
                .long("diff-file")
                .value_name("PATCH")
                .help("Unified diff for line-level security scoping; use - for stdin")
                .conflicts_with("diff-stdin")
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("diff-stdin")
                .long("diff-stdin")
                .help("Read a unified diff from stdin for --gate new")
                .conflicts_with("diff-file")
                .action(ArgAction::SetTrue),
        )
}

pub(super) fn security_options_with_defaults(
    matches: &ArgMatches,
    mut options: SecurityOptions,
) -> SecurityOptions {
    if matches.value_source("top") == Some(ValueSource::CommandLine) {
        options.top = matches.get_one::<usize>("top").copied();
    }
    if matches.value_source("surface") == Some(ValueSource::CommandLine) {
        options.surface = matches.get_flag("surface");
    }
    options
}

pub(super) fn security_cli_options(
    command: ReportCommand,
    matches: &ArgMatches,
) -> SecurityCliOptions {
    if command != ReportCommand::Security {
        return SecurityCliOptions::default();
    }
    SecurityCliOptions {
        sarif_file: matches.get_one::<PathBuf>("sarif-file").cloned(),
        gate: security_gate_mode(matches),
        diff: security_diff(matches),
        changed_since: matches
            .get_one::<String>("changed-since")
            .or_else(|| matches.get_one::<String>("compare"))
            .cloned(),
        ci: matches.get_flag("ci"),
        fail_on_issues: matches.get_flag("fail-on-issues"),
        summary: matches.get_flag("summary"),
    }
}

fn security_diff(matches: &ArgMatches) -> Option<SecurityDiffSource> {
    if let Some(path) = matches.get_one::<PathBuf>("diff-file") {
        if path == std::path::Path::new("-") {
            return Some(SecurityDiffSource::Stdin);
        }
        return Some(SecurityDiffSource::File(path.clone()));
    }
    if matches.get_flag("diff-stdin") {
        return Some(SecurityDiffSource::Stdin);
    }
    None
}

fn security_gate_mode(matches: &ArgMatches) -> Option<SecurityGateMode> {
    matches
        .get_one::<String>("gate")
        .map(String::as_str)
        .map(|value| match value {
            "new" => SecurityGateMode::New,
            "newly-reachable" => SecurityGateMode::NewlyReachable,
            _ => unreachable!("clap rejects unsupported security gate modes"),
        })
}
