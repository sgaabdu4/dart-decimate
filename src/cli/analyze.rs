use std::path::Path;

use crate::config::{filter_ignored_dependencies, filter_ignored_dependency_overrides};
use crate::generated::is_generated_dart_path;
use crate::output::{AnalysisResults, ReportCommand};
use crate::scan::ScannedProject;
use crate::{
    SymbolAnalysisOptions, analyze_dependency_hygiene, analyze_health, analyze_security,
    analyze_widgets, check_architecture_boundaries, detect_boundary_call_violations,
    detect_boundary_coverage, detect_cycles, detect_duplicates, detect_feature_flags,
    detect_policy_violations, detect_re_export_cycles, detect_route_collisions, find_dead_code,
    load_policy_pack, missing_suppression_reasons_enabled,
};

use super::entry_points::{entry_points_for_check, entry_points_for_dead_code};
use super::{CliError, CommandRequest};

pub(super) fn analyze_project(
    project: &ScannedProject,
    request: &CommandRequest,
) -> Result<AnalysisResults, CliError> {
    let dead_code = analyze_project_dead_code(project, request)?;

    Ok(AnalysisResults {
        command: request.command,
        symbols: analyze_project_symbols(project, request, dead_code.as_ref()),
        cycles: analyze_project_cycles(project, request.command),
        re_export_cycles: analyze_project_re_export_cycles(project, request.command),
        boundary_violations: analyze_project_boundaries(project, request),
        boundary_coverage: analyze_project_boundary_coverage(project, request),
        boundary_call_violations: analyze_project_boundary_calls(project, request)?,
        policy_violations: analyze_project_policy(project, request)?,
        dependency_hygiene: analyze_project_dependencies(project, request)?,
        duplicates: analyze_project_duplicates(project, request)?,
        health: analyze_project_health(project, request)?,
        feature_flags: analyze_project_flags(project, request)?,
        security: analyze_project_security(project, request, dead_code.as_ref())?,
        routes: analyze_project_routes(project, request.command),
        widgets: analyze_project_widgets(project, request.command, dead_code.as_ref())?,
        file_scope: None,
        require_suppression_reasons: missing_suppression_reasons_enabled(&request.rules),
        dead_code,
    })
}

fn analyze_project_dead_code(
    project: &ScannedProject,
    request: &CommandRequest,
) -> Result<Option<crate::DeadCodeReport>, CliError> {
    match request.command {
        ReportCommand::Check | ReportCommand::Audit => {
            let entries =
                entry_points_for_check(project, &request.entry_points, request.entry_point_mode());
            Ok((!entries.is_empty()).then(|| find_project_dead_code(project, entries, request)))
        }
        ReportCommand::DeadCode => {
            let entries = entry_points_for_dead_code(
                project,
                &request.entry_points,
                request.entry_point_mode(),
            )?;
            Ok(Some(find_project_dead_code(project, entries, request)))
        }
        ReportCommand::Cycles
        | ReportCommand::Dupes
        | ReportCommand::Health
        | ReportCommand::Flags
        | ReportCommand::Security => Ok(None),
        ReportCommand::TraceFile
        | ReportCommand::TraceSymbol
        | ReportCommand::TraceDependency
        | ReportCommand::TraceClone
        | ReportCommand::Inspect => unreachable!("trace commands do not use AnalysisResults"),
    }
}

fn analyze_project_symbols(
    project: &ScannedProject,
    request: &CommandRequest,
    dead_code: Option<&crate::DeadCodeReport>,
) -> Option<crate::SymbolReport> {
    match request.command {
        ReportCommand::Check | ReportCommand::Audit | ReportCommand::DeadCode => {
            Some(crate::analyze_symbols_with_options(
                project,
                dead_code,
                SymbolAnalysisOptions {
                    include_entry_exports: request.symbol_options.include_entry_exports,
                    private_type_leaks: request.symbol_options.private_type_leaks,
                },
            ))
        }
        ReportCommand::Cycles
        | ReportCommand::Dupes
        | ReportCommand::Health
        | ReportCommand::Flags
        | ReportCommand::Security => None,
        ReportCommand::TraceFile
        | ReportCommand::TraceSymbol
        | ReportCommand::TraceDependency
        | ReportCommand::TraceClone
        | ReportCommand::Inspect => unreachable!("trace commands do not use AnalysisResults"),
    }
}

fn analyze_project_cycles(
    project: &ScannedProject,
    command: ReportCommand,
) -> Vec<crate::DependencyCycle> {
    match command {
        ReportCommand::Check | ReportCommand::Audit | ReportCommand::Cycles => {
            detect_project_cycles(project)
        }
        ReportCommand::DeadCode
        | ReportCommand::Dupes
        | ReportCommand::Health
        | ReportCommand::Flags
        | ReportCommand::Security => Vec::new(),
        ReportCommand::TraceFile
        | ReportCommand::TraceSymbol
        | ReportCommand::TraceDependency
        | ReportCommand::TraceClone
        | ReportCommand::Inspect => unreachable!("trace commands do not use AnalysisResults"),
    }
}

fn analyze_project_re_export_cycles(
    project: &ScannedProject,
    command: ReportCommand,
) -> Vec<crate::ReExportCycle> {
    match command {
        ReportCommand::Check | ReportCommand::Audit | ReportCommand::Cycles => {
            detect_project_re_export_cycles(project)
        }
        ReportCommand::DeadCode
        | ReportCommand::Dupes
        | ReportCommand::Health
        | ReportCommand::Flags
        | ReportCommand::Security => Vec::new(),
        ReportCommand::TraceFile
        | ReportCommand::TraceSymbol
        | ReportCommand::TraceDependency
        | ReportCommand::TraceClone
        | ReportCommand::Inspect => unreachable!("trace commands do not use AnalysisResults"),
    }
}

fn analyze_project_boundaries(
    project: &ScannedProject,
    request: &CommandRequest,
) -> Vec<crate::BoundaryViolation> {
    match request.command {
        ReportCommand::Check | ReportCommand::Audit => {
            check_architecture_boundaries(&project.graph, &request.boundaries)
        }
        ReportCommand::DeadCode
        | ReportCommand::Cycles
        | ReportCommand::Dupes
        | ReportCommand::Health
        | ReportCommand::Flags
        | ReportCommand::Security => Vec::new(),
        ReportCommand::TraceFile
        | ReportCommand::TraceSymbol
        | ReportCommand::TraceDependency
        | ReportCommand::TraceClone
        | ReportCommand::Inspect => unreachable!("trace commands do not use AnalysisResults"),
    }
}

fn analyze_project_boundary_coverage(
    project: &ScannedProject,
    request: &CommandRequest,
) -> Vec<crate::BoundaryCoverageGap> {
    match request.command {
        ReportCommand::Check | ReportCommand::Audit if request.boundary_coverage => {
            detect_boundary_coverage(
                project,
                &request.boundaries,
                &request.boundary_allow_unmatched,
            )
        }
        ReportCommand::Check
        | ReportCommand::Audit
        | ReportCommand::DeadCode
        | ReportCommand::Cycles
        | ReportCommand::Dupes
        | ReportCommand::Health
        | ReportCommand::Flags
        | ReportCommand::Security => Vec::new(),
        ReportCommand::TraceFile
        | ReportCommand::TraceSymbol
        | ReportCommand::TraceDependency
        | ReportCommand::TraceClone
        | ReportCommand::Inspect => unreachable!("trace commands do not use AnalysisResults"),
    }
}

fn analyze_project_boundary_calls(
    project: &ScannedProject,
    request: &CommandRequest,
) -> Result<Vec<crate::BoundaryCallViolation>, CliError> {
    match request.command {
        ReportCommand::Check | ReportCommand::Audit => Ok(detect_boundary_call_violations(
            project,
            &request.boundary_calls,
        )?),
        ReportCommand::DeadCode
        | ReportCommand::Cycles
        | ReportCommand::Dupes
        | ReportCommand::Health
        | ReportCommand::Flags
        | ReportCommand::Security => Ok(Vec::new()),
        ReportCommand::TraceFile
        | ReportCommand::TraceSymbol
        | ReportCommand::TraceDependency
        | ReportCommand::TraceClone
        | ReportCommand::Inspect => unreachable!("trace commands do not use AnalysisResults"),
    }
}

fn analyze_project_policy(
    project: &ScannedProject,
    request: &CommandRequest,
) -> Result<Vec<crate::PolicyViolation>, CliError> {
    match request.command {
        ReportCommand::Check | ReportCommand::Audit => {
            let packs = request
                .policy_packs
                .iter()
                .map(|path| load_policy_pack(&project.root, path))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(detect_policy_violations(project, &packs)?)
        }
        ReportCommand::DeadCode
        | ReportCommand::Cycles
        | ReportCommand::Dupes
        | ReportCommand::Health
        | ReportCommand::Flags
        | ReportCommand::Security => Ok(Vec::new()),
        ReportCommand::TraceFile
        | ReportCommand::TraceSymbol
        | ReportCommand::TraceDependency
        | ReportCommand::TraceClone
        | ReportCommand::Inspect => unreachable!("trace commands do not use AnalysisResults"),
    }
}

fn analyze_project_dependencies(
    project: &ScannedProject,
    request: &CommandRequest,
) -> Result<Option<crate::DependencyHygieneReport>, CliError> {
    match request.command {
        ReportCommand::Check | ReportCommand::Audit | ReportCommand::DeadCode => {
            let mut report = analyze_dependency_hygiene(project)?;
            filter_ignored_dependencies(&mut report, &request.ignore_dependencies);
            filter_ignored_dependency_overrides(&mut report, &request.ignore_dependency_overrides);
            Ok(Some(report))
        }
        ReportCommand::Cycles
        | ReportCommand::Dupes
        | ReportCommand::Health
        | ReportCommand::Flags
        | ReportCommand::Security => Ok(None),
        ReportCommand::TraceFile
        | ReportCommand::TraceSymbol
        | ReportCommand::TraceDependency
        | ReportCommand::TraceClone
        | ReportCommand::Inspect => unreachable!("trace commands do not use AnalysisResults"),
    }
}

fn analyze_project_duplicates(
    project: &ScannedProject,
    request: &CommandRequest,
) -> Result<Option<crate::DuplicateCodeReport>, CliError> {
    match request.command {
        ReportCommand::Check | ReportCommand::Audit | ReportCommand::Dupes => Ok(Some(
            detect_duplicates(project, &request.duplicate_options)?,
        )),
        ReportCommand::DeadCode
        | ReportCommand::Cycles
        | ReportCommand::Health
        | ReportCommand::Flags
        | ReportCommand::Security => Ok(None),
        ReportCommand::TraceFile
        | ReportCommand::TraceSymbol
        | ReportCommand::TraceDependency
        | ReportCommand::TraceClone
        | ReportCommand::Inspect => unreachable!("trace commands do not use AnalysisResults"),
    }
}

fn analyze_project_health(
    project: &ScannedProject,
    request: &CommandRequest,
) -> Result<Option<crate::HealthReport>, CliError> {
    match request.command {
        ReportCommand::Check | ReportCommand::Audit | ReportCommand::Health => {
            Ok(Some(analyze_health(project, &request.health_options)?))
        }
        ReportCommand::DeadCode
        | ReportCommand::Cycles
        | ReportCommand::Dupes
        | ReportCommand::Flags
        | ReportCommand::Security => Ok(None),
        ReportCommand::TraceFile
        | ReportCommand::TraceSymbol
        | ReportCommand::TraceDependency
        | ReportCommand::TraceClone
        | ReportCommand::Inspect => unreachable!("trace commands do not use AnalysisResults"),
    }
}

fn analyze_project_flags(
    project: &ScannedProject,
    request: &CommandRequest,
) -> Result<Option<crate::FeatureFlagReport>, CliError> {
    match request.command {
        ReportCommand::Check | ReportCommand::Audit | ReportCommand::Flags => Ok(Some(
            detect_feature_flags(project, &request.feature_flag_options)?,
        )),
        ReportCommand::DeadCode
        | ReportCommand::Cycles
        | ReportCommand::Dupes
        | ReportCommand::Health
        | ReportCommand::Security => Ok(None),
        ReportCommand::TraceFile
        | ReportCommand::TraceSymbol
        | ReportCommand::TraceDependency
        | ReportCommand::TraceClone
        | ReportCommand::Inspect => unreachable!("trace commands do not use AnalysisResults"),
    }
}

fn analyze_project_security(
    project: &ScannedProject,
    request: &CommandRequest,
    dead_code: Option<&crate::DeadCodeReport>,
) -> Result<Option<crate::SecurityReport>, CliError> {
    match request.command {
        ReportCommand::Check | ReportCommand::Audit => Ok(Some(analyze_security(
            project,
            &request.security_options,
            dead_code,
        )?)),
        ReportCommand::Security => {
            let entries =
                entry_points_for_check(project, &request.entry_points, request.entry_point_mode());
            let reachability =
                (!entries.is_empty()).then(|| find_project_dead_code(project, entries, request));
            Ok(Some(analyze_security(
                project,
                &request.security_options,
                reachability.as_ref(),
            )?))
        }
        ReportCommand::DeadCode
        | ReportCommand::Cycles
        | ReportCommand::Dupes
        | ReportCommand::Health
        | ReportCommand::Flags => Ok(None),
        ReportCommand::TraceFile
        | ReportCommand::TraceSymbol
        | ReportCommand::TraceDependency
        | ReportCommand::TraceClone
        | ReportCommand::Inspect => unreachable!("trace commands do not use AnalysisResults"),
    }
}

fn analyze_project_routes(
    project: &ScannedProject,
    command: ReportCommand,
) -> Option<crate::RouteCollisionReport> {
    match command {
        ReportCommand::Check | ReportCommand::Audit => Some(detect_route_collisions(project)),
        ReportCommand::DeadCode
        | ReportCommand::Cycles
        | ReportCommand::Dupes
        | ReportCommand::Health
        | ReportCommand::Flags
        | ReportCommand::Security => None,
        ReportCommand::TraceFile
        | ReportCommand::TraceSymbol
        | ReportCommand::TraceDependency
        | ReportCommand::TraceClone
        | ReportCommand::Inspect => unreachable!("trace commands do not use AnalysisResults"),
    }
}

fn analyze_project_widgets(
    project: &ScannedProject,
    command: ReportCommand,
    dead_code: Option<&crate::DeadCodeReport>,
) -> Result<Option<crate::WidgetReport>, CliError> {
    match command {
        ReportCommand::Check | ReportCommand::Audit => {
            Ok(Some(analyze_widgets(project, dead_code)?))
        }
        ReportCommand::DeadCode
        | ReportCommand::Cycles
        | ReportCommand::Dupes
        | ReportCommand::Health
        | ReportCommand::Flags
        | ReportCommand::Security => Ok(None),
        ReportCommand::TraceFile
        | ReportCommand::TraceSymbol
        | ReportCommand::TraceDependency
        | ReportCommand::TraceClone
        | ReportCommand::Inspect => unreachable!("trace commands do not use AnalysisResults"),
    }
}

fn find_project_dead_code<I, P>(
    project: &ScannedProject,
    entry_points: I,
    request: &CommandRequest,
) -> crate::DeadCodeReport
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let mut report = find_dead_code(&project.graph, entry_points);
    report.dead_files.retain(|dead_file| {
        is_project_path(project, &dead_file.path) && !is_generated_dart_path(&dead_file.path)
    });
    report
        .reachable_files
        .retain(|path| is_project_path(project, path));
    if request.production {
        for dead_file in &mut report.dead_files {
            dead_file.safe_to_delete = false;
        }
    }
    report
}

fn detect_project_cycles(project: &ScannedProject) -> Vec<crate::DependencyCycle> {
    detect_cycles(&project.graph)
        .into_iter()
        .filter(|cycle| {
            cycle
                .files
                .iter()
                .any(|path| is_project_path(project, path))
                && !cycle.files.iter().all(|path| is_generated_dart_path(path))
        })
        .collect()
}

fn detect_project_re_export_cycles(project: &ScannedProject) -> Vec<crate::ReExportCycle> {
    detect_re_export_cycles(&project.graph)
        .into_iter()
        .filter(|cycle| {
            cycle
                .files
                .iter()
                .any(|path| is_project_path(project, path))
                && !cycle.files.iter().all(|path| is_generated_dart_path(path))
        })
        .collect()
}

fn is_project_path(project: &ScannedProject, path: &Path) -> bool {
    path.starts_with(&project.root)
}
