use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::{Arg, ArgMatches};

use crate::baseline::baseline_from_report;
use crate::changed_scope::{RefSuggestions, changed_file_scope_from_changed, changed_files};
use crate::config::apply_rules_to_report;
use crate::graph::normalize_path;
use crate::output::{
    AnalysisResults, JsonReport, ReportCommand, apply_audit_risk, build_json_report,
    filter_report_findings,
};
use crate::scan::{ScannedProject, scan_project_with_options};
use crate::{BoundaryCallRule, BoundaryRule, HealthOptions};

use super::{CliError, CommandRequest, analyze::analyze_project};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum AuditGate {
    NewOnly,
    #[default]
    All,
}

pub(super) fn gate_arg() -> Arg {
    Arg::new("gate")
        .long("gate")
        .value_name("MODE")
        .value_parser(["new-only", "all"])
        .default_value("all")
        .help("Audit gate mode: all visible findings or only findings introduced by changed files")
}

pub(super) fn gate(command: ReportCommand, subcommand: &ArgMatches) -> AuditGate {
    if command != ReportCommand::Audit {
        return AuditGate::All;
    }
    match subcommand.get_one::<String>("gate").map(String::as_str) {
        Some("new-only") => AuditGate::NewOnly,
        _ => AuditGate::All,
    }
}

#[derive(Debug, Default)]
pub(super) struct AuditContext {
    pub(super) changed_files: Vec<PathBuf>,
    pub(super) base_finding_identities: BTreeSet<String>,
}

pub(super) fn prepare_context(
    project: &ScannedProject,
    request: &CommandRequest,
    results: &mut AnalysisResults,
) -> Result<AuditContext, CliError> {
    if request.command != ReportCommand::Audit {
        return Ok(AuditContext::default());
    }

    let Some(base) = request.audit_base.as_deref() else {
        unreachable!("clap requires --base for audit");
    };
    let changed = changed_files(&project.root, base)?;
    results.file_scope = Some(changed_file_scope_from_changed(project, &changed));
    let base_finding_identities = base_finding_identities(request, base)?;

    Ok(AuditContext {
        changed_files: changed,
        base_finding_identities,
    })
}

pub(super) fn apply_risk(root: &Path, context: &AuditContext, report: &mut JsonReport) {
    apply_audit_risk(
        root,
        &context.changed_files,
        &context.base_finding_identities,
        report,
    );
}

fn base_finding_identities(
    request: &CommandRequest,
    base: &str,
) -> Result<BTreeSet<String>, CliError> {
    let snapshot = GitSnapshot::create(&request.root, base)?;
    let project = scan_project_with_options(snapshot.root(), &request.scan_options)?;
    let base_request = request_for_snapshot(request, snapshot.root());
    let results = analyze_project(&project, &base_request)?;
    let mut report = build_json_report(&project, &results);
    apply_rules_to_report(&mut report, &request.rules)?;
    filter_report_findings(&mut report, &request.issue_filters.kinds);
    Ok(baseline_from_report(&report)
        .findings
        .into_iter()
        .map(|finding| finding.identity)
        .collect())
}

fn request_for_snapshot(request: &CommandRequest, snapshot_root: &Path) -> CommandRequest {
    let original_root = normalize_path(&request.root);
    let mut snapshot_request = request.clone();
    snapshot_request.root = snapshot_root.to_path_buf();
    snapshot_request.entry_points =
        rebase_project_paths(&original_root, snapshot_root, &request.entry_points);
    snapshot_request.boundaries =
        rebase_boundary_rules(&original_root, snapshot_root, &request.boundaries);
    snapshot_request.boundary_calls =
        rebase_boundary_calls(&original_root, snapshot_root, &request.boundary_calls);
    snapshot_request.policy_packs = absolutize_project_paths(&original_root, &request.policy_packs);
    snapshot_request.health_options =
        rebase_health_options(&original_root, &request.health_options);
    snapshot_request
}

fn rebase_project_paths(
    original_root: &Path,
    snapshot_root: &Path,
    paths: &[PathBuf],
) -> Vec<PathBuf> {
    paths
        .iter()
        .map(|path| rebase_project_path(original_root, snapshot_root, path))
        .collect()
}

fn absolutize_project_paths(original_root: &Path, paths: &[PathBuf]) -> Vec<PathBuf> {
    paths
        .iter()
        .map(|path| {
            if path.is_absolute() {
                path.clone()
            } else {
                original_root.join(path)
            }
        })
        .collect()
}

fn rebase_project_path(original_root: &Path, snapshot_root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.strip_prefix(original_root).map_or_else(
            |_| path.to_path_buf(),
            |relative| snapshot_root.join(relative),
        );
    }
    path.to_path_buf()
}

fn rebase_boundary_rules(
    original_root: &Path,
    snapshot_root: &Path,
    rules: &[BoundaryRule],
) -> Vec<BoundaryRule> {
    rules
        .iter()
        .map(|rule| BoundaryRule {
            from: rebase_project_path(original_root, snapshot_root, &rule.from),
            disallow: rebase_project_path(original_root, snapshot_root, &rule.disallow),
        })
        .collect()
}

fn rebase_boundary_calls(
    original_root: &Path,
    snapshot_root: &Path,
    rules: &[BoundaryCallRule],
) -> Vec<BoundaryCallRule> {
    rules
        .iter()
        .map(|rule| BoundaryCallRule {
            from: rebase_project_path(original_root, snapshot_root, &rule.from),
            forbidden: rule.forbidden.clone(),
        })
        .collect()
}

fn rebase_health_options(original_root: &Path, options: &HealthOptions) -> HealthOptions {
    let mut options = options.clone();
    options.coverage_path = options
        .coverage_path
        .map(|path| absolutize_project_path(original_root, path));
    options.runtime_coverage_path = options
        .runtime_coverage_path
        .map(|path| absolutize_project_path(original_root, path));
    options
}

fn absolutize_project_path(original_root: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        original_root.join(path)
    }
}

struct GitSnapshot {
    root: PathBuf,
}

impl GitSnapshot {
    fn create(project_root: &Path, base: &str) -> Result<Self, CliError> {
        let snapshot = Self {
            root: create_temp_dir()?,
        };
        snapshot.materialize(project_root, base)?;
        Ok(snapshot)
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn materialize(&self, project_root: &Path, base: &str) -> Result<(), CliError> {
        for path in git_tree_paths(project_root, base)? {
            if should_materialize(&path) {
                self.write_git_file(project_root, base, &path)?;
            }
        }
        Ok(())
    }

    fn write_git_file(&self, project_root: &Path, base: &str, path: &str) -> Result<(), CliError> {
        let Some(target) = snapshot_target(&self.root, path) else {
            return Ok(());
        };
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let spec = format!("{base}:{path}");
        let bytes = git_output(project_root, base, ["show", spec.as_str()])?;
        fs::write(target, bytes)?;
        Ok(())
    }
}

impl Drop for GitSnapshot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn snapshot_target(root: &Path, path: &str) -> Option<PathBuf> {
    let relative = Path::new(path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return None;
    }
    Some(root.join(relative))
}

fn git_tree_paths(project_root: &Path, base: &str) -> Result<Vec<String>, CliError> {
    let output = git_output(
        project_root,
        base,
        ["ls-tree", "-r", "-z", "--name-only", base, "--"],
    )?;
    Ok(output
        .split(|byte| *byte == b'\0')
        .filter_map(|path| std::str::from_utf8(path).ok())
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect())
}

fn git_output<'a, I>(project_root: &Path, base: &str, args: I) -> Result<Vec<u8>, CliError>
where
    I: IntoIterator<Item = &'a str>,
{
    let output = Command::new("git")
        .args(args)
        .current_dir(project_root)
        .output()
        .map_err(|source| crate::changed_scope::ChangedScopeError::Git { source })?;
    if output.status.success() {
        return Ok(output.stdout);
    }
    Err(crate::changed_scope::ChangedScopeError::GitDiff {
        base: base.to_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        suggestions: RefSuggestions::default(),
    }
    .into())
}

fn should_materialize(path: &str) -> bool {
    let path = Path::new(path);
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("dart"))
        || path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| {
                matches!(
                    name,
                    "pubspec.yaml" | "pubspec_overrides.yaml" | "pubspec.lock"
                )
            })
        || path.ends_with(Path::new(".dart_tool/package_config.json"))
}

fn create_temp_dir() -> Result<PathBuf, std::io::Error> {
    let base = std::env::temp_dir();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    for attempt in 0..100u8 {
        let path = base.join(format!(
            "dart-decimate-audit-base-{}-{nanos}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not create unique Dart Decimate audit temp directory",
    ))
}
