use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::output::{FindingKind, ReportCommand};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct IssueFilters {
    pub(super) kinds: Vec<FindingKind>,
}

pub(super) fn check_issue_filter_command(command: Command) -> Command {
    let command = shared_issue_filter_command(command);
    command
        .arg(filter_arg(
            "circular-deps",
            "Report only circular dependency findings",
        ))
        .arg(filter_arg(
            "re-export-cycles",
            "Report only re-export cycle findings",
        ))
        .arg(filter_arg(
            "boundary-violations",
            "Report only architecture boundary findings",
        ))
}

pub(super) fn dead_code_issue_filter_command(command: Command) -> Command {
    shared_issue_filter_command(command)
}

fn shared_issue_filter_command(command: Command) -> Command {
    command
        .arg(filter_arg(
            "unused-files",
            "Report only unreachable Dart files",
        ))
        .arg(filter_arg(
            "unused-exports",
            "Report only unused top-level exports",
        ))
        .arg(filter_arg(
            "unused-types",
            "Report only unused public typedefs",
        ))
        .arg(filter_arg(
            "unused-deps",
            "Report only unused pub dependency findings",
        ))
        .arg(filter_arg(
            "unlisted-deps",
            "Report only imports missing from pubspec dependencies",
        ))
        .arg(filter_arg(
            "private-src-imports",
            "Report only cross-package lib/src imports",
        ))
        .arg(filter_arg(
            "duplicate-exports",
            "Report only duplicated public API exports",
        ))
        .arg(filter_arg(
            "unused-enum-members",
            "Report only unused enum constants",
        ))
        .arg(filter_arg(
            "unused-class-members",
            "Report only unused class-like members",
        ))
        .arg(filter_arg(
            "unresolved-imports",
            "Report only unresolved local dependency URIs",
        ))
        .arg(filter_arg(
            "stale-suppressions",
            "Report only stale dart-decimate/fallow suppression comments",
        ))
        .arg(filter_arg(
            "unused-dependency-overrides",
            "Report only unused pub dependency overrides",
        ))
        .arg(filter_arg(
            "misconfigured-dependency-overrides",
            "Report only invalid pub dependency overrides",
        ))
}

pub(super) fn issue_filters(command: ReportCommand, matches: &ArgMatches) -> IssueFilters {
    if !matches!(command, ReportCommand::Check | ReportCommand::DeadCode) {
        return IssueFilters::default();
    }
    let mut kinds = Vec::new();
    push_dead_code_filters(matches, &mut kinds);
    push_dependency_filters(matches, &mut kinds);
    push_graph_filters(matches, &mut kinds);
    push_member_filters(matches, &mut kinds);
    IssueFilters { kinds }
}

fn filter_arg(id: &'static str, help: &'static str) -> Arg {
    Arg::new(id).long(id).help(help).action(ArgAction::SetTrue)
}

fn push_dead_code_filters(matches: &ArgMatches, kinds: &mut Vec<FindingKind>) {
    push_flag(matches, kinds, "unused-files", &[FindingKind::DeadFile]);
    push_flag(
        matches,
        kinds,
        "unused-exports",
        &[FindingKind::UnusedExport],
    );
    push_flag(matches, kinds, "unused-types", &[FindingKind::UnusedType]);
    push_flag(
        matches,
        kinds,
        "private-type-leaks",
        &[FindingKind::PrivateTypeLeak],
    );
    push_flag(
        matches,
        kinds,
        "stale-suppressions",
        &[FindingKind::StaleSuppression],
    );
}

fn push_dependency_filters(matches: &ArgMatches, kinds: &mut Vec<FindingKind>) {
    push_flag(
        matches,
        kinds,
        "unused-deps",
        &[
            FindingKind::UnusedDependency,
            FindingKind::UnusedDevDependency,
            FindingKind::DevDependencyInProduction,
            FindingKind::TestOnlyDependency,
        ],
    );
    push_flag(
        matches,
        kinds,
        "unlisted-deps",
        &[FindingKind::UnlistedDependency],
    );
    push_flag(
        matches,
        kinds,
        "private-src-imports",
        &[FindingKind::PrivateSrcImport],
    );
    push_flag(
        matches,
        kinds,
        "unresolved-imports",
        &[FindingKind::UnresolvedDependency],
    );
    push_flag(
        matches,
        kinds,
        "unused-dependency-overrides",
        &[FindingKind::UnusedDependencyOverride],
    );
    push_flag(
        matches,
        kinds,
        "misconfigured-dependency-overrides",
        &[FindingKind::MisconfiguredDependencyOverride],
    );
}

fn push_graph_filters(matches: &ArgMatches, kinds: &mut Vec<FindingKind>) {
    push_flag(
        matches,
        kinds,
        "duplicate-exports",
        &[FindingKind::DuplicateExport],
    );
    push_flag(
        matches,
        kinds,
        "circular-deps",
        &[FindingKind::CircularDependency],
    );
    push_flag(
        matches,
        kinds,
        "re-export-cycles",
        &[FindingKind::ReExportCycle],
    );
    push_flag(
        matches,
        kinds,
        "boundary-violations",
        &[
            FindingKind::BoundaryViolation,
            FindingKind::BoundaryCoverage,
            FindingKind::BoundaryCallViolation,
        ],
    );
    push_flag(
        matches,
        kinds,
        "policy-violations",
        &[FindingKind::PolicyViolation],
    );
}

fn push_member_filters(matches: &ArgMatches, kinds: &mut Vec<FindingKind>) {
    push_flag(
        matches,
        kinds,
        "unused-enum-members",
        &[FindingKind::UnusedEnumMember],
    );
    push_flag(
        matches,
        kinds,
        "unused-class-members",
        &[FindingKind::UnusedClassMember],
    );
}

fn push_flag(
    matches: &ArgMatches,
    kinds: &mut Vec<FindingKind>,
    id: &str,
    flag_kinds: &[FindingKind],
) {
    if matches
        .try_get_one::<bool>(id)
        .ok()
        .flatten()
        .copied()
        .unwrap_or_default()
    {
        for kind in flag_kinds {
            if !kinds.contains(kind) {
                kinds.push(*kind);
            }
        }
    }
}
