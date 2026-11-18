use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::output::JsonRuntimeCoverage;
use crate::{DartFile, ScannedProject};

/// Stable schema version for focused runtime coverage analysis.
pub const COVERAGE_ANALYSIS_SCHEMA_VERSION: &str = "dart-decimate.coverage.v1";

/// Focused runtime coverage analysis output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageAnalysisReport {
    /// Schema identifier.
    pub schema_version: String,
    /// Typed envelope discriminator.
    pub kind: String,
    /// Tool name.
    pub tool: String,
    /// Command that produced this report.
    pub command: String,
    /// Runtime coverage intelligence.
    pub runtime_coverage: JsonRuntimeCoverage,
}

/// Runtime coverage setup report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageSetupReport {
    /// Schema identifier.
    pub schema_version: String,
    /// Typed envelope discriminator.
    pub kind: String,
    /// Tool name.
    pub tool: String,
    /// Command that produced this report.
    pub command: String,
    /// Project root.
    pub root: PathBuf,
    /// Whether setup writes were accepted with `--yes`.
    pub applied: bool,
    /// Whether setup ran in non-interactive mode.
    pub non_interactive: bool,
    /// Project coverage setup summary.
    pub summary: CoverageSetupSummary,
    /// Files created or planned by setup.
    pub files: Vec<CoverageSetupFile>,
    /// Runtime coverage capture commands for agents.
    pub capture_commands: Vec<String>,
    /// Suggested follow-up commands.
    pub next_steps: Vec<String>,
    /// Non-fatal setup warnings.
    pub warnings: Vec<String>,
}

/// Runtime coverage setup summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageSetupSummary {
    /// Whether `pubspec.yaml` exists at the root.
    pub pubspec: bool,
    /// Whether the root looks like a Flutter project.
    pub flutter: bool,
    /// Parsed Dart file count.
    pub dart_files: usize,
    /// Coverage config state.
    pub config: CoverageSetupConfigSummary,
}

/// Runtime coverage setup config state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageSetupConfigSummary {
    /// Whether Dart Decimate config already existed.
    pub exists: bool,
    /// Whether loaded config already points at coverage input.
    pub coverage_configured: bool,
}

/// Runtime coverage setup file status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageSetupFile {
    /// Path relative to the project root.
    pub path: String,
    /// File kind.
    pub kind: String,
    /// Action performed or planned.
    pub action: CoverageSetupFileAction,
    /// Agent-readable reason.
    pub reason: String,
}

/// Runtime coverage setup file action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoverageSetupFileAction {
    /// File would be created if `--yes` is passed.
    WouldCreate,
    /// File was created.
    Created,
    /// File already existed or no write was needed.
    Unchanged,
}

/// Offline coverage upload or inventory report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageUploadReport {
    /// Schema identifier.
    pub schema_version: String,
    /// Typed envelope discriminator.
    pub kind: String,
    /// Tool name.
    pub tool: String,
    /// Command that produced this report.
    pub command: String,
    /// Project root.
    pub root: PathBuf,
    /// Dry-run mode. Upload commands are offline-only until cloud support lands.
    pub dry_run: bool,
    /// Target repository, when supplied.
    pub repo: Option<String>,
    /// Git commit SHA, when supplied.
    pub git_sha: Option<String>,
    /// Whether uploaded paths would be stripped to root-relative paths.
    pub strip_path: bool,
    /// Upload inventory summary.
    pub summary: CoverageUploadSummary,
    /// Files included in the dry-run packet.
    pub files: Vec<CoverageUploadFile>,
    /// Non-mutating actions an agent can take next.
    pub actions: Vec<String>,
    /// Non-fatal warnings.
    pub warnings: Vec<String>,
}

/// Upload inventory summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageUploadSummary {
    /// Upload packet mode.
    pub mode: String,
    /// File count.
    pub files: usize,
    /// Total bytes across packet files.
    pub bytes: u64,
}

/// Upload inventory file row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageUploadFile {
    /// Root-relative path when possible.
    pub path: String,
    /// File kind.
    pub kind: String,
    /// File size in bytes.
    pub bytes: u64,
    /// Runtime source-map resolution status for agent confidence.
    pub resolution_status: String,
    /// Runtime source-map mapping quality for agent confidence.
    pub mapping_quality: String,
}

/// Build a focused runtime coverage report.
#[must_use]
pub fn coverage_analysis_report(runtime_coverage: JsonRuntimeCoverage) -> CoverageAnalysisReport {
    CoverageAnalysisReport {
        schema_version: COVERAGE_ANALYSIS_SCHEMA_VERSION.to_owned(),
        kind: "runtime-coverage".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: "coverage analyze".to_owned(),
        runtime_coverage,
    }
}

/// # Errors
///
/// Returns an IO error if `apply` is true and the setup config cannot be written.
pub fn coverage_setup_report(
    project: &ScannedProject,
    apply: bool,
    non_interactive: bool,
    coverage_configured: bool,
) -> io::Result<CoverageSetupReport> {
    let root = &project.root;
    let config_path = root.join(".dart-decimaterc");
    let config_exists = config_path.exists();
    let action = if config_exists {
        CoverageSetupFileAction::Unchanged
    } else if apply {
        fs::write(&config_path, COVERAGE_CONFIG)?;
        CoverageSetupFileAction::Created
    } else {
        CoverageSetupFileAction::WouldCreate
    };
    let pubspec = fs::read_to_string(root.join("pubspec.yaml")).ok();
    let flutter = pubspec.as_deref().is_some_and(is_flutter_pubspec);
    let mut warnings = Vec::new();
    if pubspec
        .as_ref()
        .is_none_or(|source| source.trim().is_empty())
    {
        warnings.push("pubspec.yaml was not found at the project root".to_owned());
    }
    if !flutter {
        warnings
            .push("Flutter was not detected; Dart VM coverage input is still supported".to_owned());
    }

    Ok(CoverageSetupReport {
        schema_version: COVERAGE_ANALYSIS_SCHEMA_VERSION.to_owned(),
        kind: "coverage-setup".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: "coverage setup".to_owned(),
        root: root.clone(),
        applied: apply,
        non_interactive,
        summary: CoverageSetupSummary {
            pubspec: pubspec.is_some(),
            flutter,
            dart_files: project.files.len(),
            config: CoverageSetupConfigSummary {
                exists: config_exists,
                coverage_configured,
            },
        },
        files: vec![CoverageSetupFile {
            path: ".dart-decimaterc".to_owned(),
            kind: "config".to_owned(),
            action,
            reason: setup_file_reason(action).to_owned(),
        }],
        capture_commands: coverage_capture_commands(flutter),
        next_steps: vec![
            "dart-decimate health . --coverage coverage/lcov.info --coverage-gaps --format json"
                .to_owned(),
            "dart-decimate coverage analyze . --runtime-coverage coverage/coverage-final.json --format json"
                .to_owned(),
        ],
        warnings,
    })
}

/// Build a local source inventory upload dry-run report.
#[must_use]
pub fn coverage_inventory_upload_report(
    project: &ScannedProject,
    repo: Option<String>,
    dry_run: bool,
) -> CoverageUploadReport {
    let files = project
        .files
        .iter()
        .filter(|file| production_source_file(&project.root, file))
        .map(|file| {
            upload_file(
                &project.root,
                &file.path,
                "dart-source",
                "resolved",
                "high",
                true,
            )
        })
        .collect::<Vec<_>>();
    upload_report(UploadReportInput {
        root: &project.root,
        kind: "coverage-upload-inventory",
        command: "coverage upload-inventory",
        mode: "inventory",
        dry_run,
        repo,
        git_sha: None,
        strip_path: true,
        files,
    })
}

/// # Errors
///
/// Returns an IO error when the source-map directory cannot be read.
pub fn coverage_source_maps_upload_report(
    root: &Path,
    dir: &Path,
    repo: String,
    git_sha: String,
    strip_path: bool,
    dry_run: bool,
) -> io::Result<CoverageUploadReport> {
    let mut paths = Vec::new();
    collect_source_maps(dir, &mut paths)?;
    paths.sort();
    let files = paths
        .iter()
        .map(|path| upload_file(root, path, "source-map", "resolved", "high", strip_path))
        .collect::<Vec<_>>();
    Ok(upload_report(UploadReportInput {
        root,
        kind: "coverage-upload-source-maps",
        command: "coverage upload-source-maps",
        mode: "source-maps",
        dry_run,
        repo: Some(repo),
        git_sha: Some(git_sha),
        strip_path,
        files,
    }))
}

/// Render a concise human runtime coverage report.
#[must_use]
pub fn render_coverage_analysis_report(report: &CoverageAnalysisReport) -> String {
    let summary = &report.runtime_coverage.summary;
    format!(
        "Runtime coverage: {} observed files, {} invocations, {} hot paths, {} findings\n",
        summary.observed_files, summary.total_invocations, summary.hot_paths, summary.findings
    )
}

/// Render a concise human setup report.
#[must_use]
pub fn render_coverage_setup_report(report: &CoverageSetupReport) -> String {
    let mut output = format!(
        "Coverage setup: {} Dart files, flutter={}, applied={}\n",
        report.summary.dart_files, report.summary.flutter, report.applied
    );
    for file in &report.files {
        let _ = writeln!(output, "{}: {:?}", file.path, file.action);
    }
    output
}

/// Render a concise human upload dry-run report.
#[must_use]
pub fn render_coverage_upload_report(report: &CoverageUploadReport) -> String {
    format!(
        "{}: {} files, {} bytes, dry_run={}\n",
        report.command, report.summary.files, report.summary.bytes, report.dry_run
    )
}

struct UploadReportInput<'a> {
    root: &'a Path,
    kind: &'a str,
    command: &'a str,
    mode: &'a str,
    dry_run: bool,
    repo: Option<String>,
    git_sha: Option<String>,
    strip_path: bool,
    files: Vec<CoverageUploadFile>,
}

fn upload_report(input: UploadReportInput<'_>) -> CoverageUploadReport {
    let bytes = input.files.iter().map(|file| file.bytes).sum();
    CoverageUploadReport {
        schema_version: COVERAGE_ANALYSIS_SCHEMA_VERSION.to_owned(),
        kind: input.kind.to_owned(),
        tool: "dart-decimate".to_owned(),
        command: input.command.to_owned(),
        root: input.root.to_path_buf(),
        dry_run: input.dry_run,
        repo: input.repo,
        git_sha: input.git_sha,
        strip_path: input.strip_path,
        summary: CoverageUploadSummary {
            mode: input.mode.to_owned(),
            files: input.files.len(),
            bytes,
        },
        files: input.files,
        actions: vec![
            "review dry-run packet before enabling hosted runtime coverage".to_owned(),
            "run dart-decimate coverage analyze with a local --runtime-coverage file for immediate evidence"
                .to_owned(),
        ],
        warnings: Vec::new(),
    }
}

fn upload_file(
    root: &Path,
    path: &Path,
    kind: &str,
    resolution_status: &str,
    mapping_quality: &str,
    strip_path: bool,
) -> CoverageUploadFile {
    CoverageUploadFile {
        path: if strip_path {
            display_path(root, path)
        } else {
            path.to_string_lossy().replace('\\', "/")
        },
        kind: kind.to_owned(),
        bytes: fs::metadata(path).map_or(0, |metadata| metadata.len()),
        resolution_status: resolution_status.to_owned(),
        mapping_quality: mapping_quality.to_owned(),
    }
}

fn collect_source_maps(dir: &Path, paths: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_source_maps(&path, paths)?;
        } else if file_type.is_file() && path.extension().is_some_and(|ext| ext == "map") {
            paths.push(path);
        }
    }
    Ok(())
}

fn production_source_file(root: &Path, file: &DartFile) -> bool {
    let path = display_path(root, &file.path);
    !path.starts_with("test/") && !path.ends_with("_test.dart")
}

fn coverage_capture_commands(flutter: bool) -> Vec<String> {
    let mut commands = Vec::new();
    if flutter {
        commands.push("flutter test --coverage".to_owned());
    }
    commands.push("dart run coverage:test_with_coverage".to_owned());
    commands.push(
        "dart-decimate coverage analyze . --runtime-coverage coverage/coverage-final.json --format json"
            .to_owned(),
    );
    commands
}

fn setup_file_reason(action: CoverageSetupFileAction) -> &'static str {
    match action {
        CoverageSetupFileAction::WouldCreate => {
            "pass --yes to create coverage defaults without prompting"
        }
        CoverageSetupFileAction::Created => {
            "created coverage defaults for LCOV and runtime coverage inputs"
        }
        CoverageSetupFileAction::Unchanged => {
            "existing config was left unchanged; merge coverage defaults manually if needed"
        }
    }
}

fn is_flutter_pubspec(source: &str) -> bool {
    source.contains("sdk: flutter") || source.contains("\nflutter:")
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

const COVERAGE_CONFIG: &str = r#"{
  "health": {
    "coverage_path": "coverage/lcov.info",
    "coverage_gaps": true,
    "runtime_coverage": "coverage/coverage-final.json"
  }
}
"#;
