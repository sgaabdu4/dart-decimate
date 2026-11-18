use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable schema version for project initialization output.
pub const INIT_SCHEMA_VERSION: &str = "dart-decimate.init.v1";

/// Project initialization options.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InitOptions {
    /// Overwrite existing files.
    pub force: bool,
    /// Write agent guidance.
    pub agents: bool,
}

/// Machine-readable initialization report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitReport {
    /// Schema identifier.
    pub schema_version: String,
    /// Typed JSON envelope discriminator.
    pub kind: String,
    /// Tool name.
    pub tool: String,
    /// Command that produced this report.
    pub command: String,
    /// Root that was initialized.
    pub root: PathBuf,
    /// Files written or overwritten.
    pub files: Vec<InitFile>,
    /// Suggested follow-up commands.
    pub next_steps: Vec<String>,
}

/// File produced by `dart-decimate init`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitFile {
    /// Path relative to the initialized root.
    pub path: String,
    /// File purpose.
    pub kind: InitFileKind,
    /// Write action performed.
    pub action: InitFileAction,
}

/// Type of initialized file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InitFileKind {
    /// Dart Decimate configuration.
    Config,
    /// Agent workflow guidance.
    Agents,
}

/// Write action performed for an initialized file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InitFileAction {
    /// File did not exist and was created.
    Created,
    /// File existed and was overwritten because `force` was set.
    Overwritten,
}

/// Errors returned while initializing a project.
#[derive(Debug, Error)]
pub enum InitError {
    /// A target file already exists and `force` was not set.
    #[error("refusing to overwrite existing init file {path}; pass --force")]
    AlreadyExists {
        /// Existing path.
        path: PathBuf,
    },
    /// A directory could not be created.
    #[error("failed to create init directory {path}: {source}")]
    CreateDir {
        /// Directory path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A file could not be written.
    #[error("failed to write init file {path}: {source}")]
    Write {
        /// File path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
}

/// # Errors
///
/// Returns [`InitError`] when an initialized file already exists without `force`, or when directories/files cannot be written.
pub fn init_project(root: impl AsRef<Path>, options: InitOptions) -> Result<InitReport, InitError> {
    let root = root.as_ref();
    let files = init_files(options.agents);
    let mut written = Vec::with_capacity(files.len());

    for file in files {
        let path = root.join(file.path);
        let action = write_init_file(&path, file.content, options.force)?;
        written.push(InitFile {
            path: file.path.to_owned(),
            kind: file.kind,
            action,
        });
    }

    Ok(InitReport {
        schema_version: INIT_SCHEMA_VERSION.to_owned(),
        kind: "init".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: "init".to_owned(),
        root: root.to_path_buf(),
        files: written,
        next_steps: vec![
            "dart-decimate check --format json".to_owned(),
            "dart-decimate audit --format json --base origin/main".to_owned(),
        ],
    })
}

/// Render a human-readable initialization report.
#[must_use]
pub fn render_init_report(report: &InitReport) -> String {
    let mut output = format!("Initialized Dart Decimate in {}\n", report.root.display());
    for file in &report.files {
        let action = match file.action {
            InitFileAction::Created => "created",
            InitFileAction::Overwritten => "overwrote",
        };
        let _ = writeln!(output, "{action} {}", file.path);
    }
    output.push_str("Next:\n");
    for step in &report.next_steps {
        let _ = writeln!(output, "  {step}");
    }
    output
}

struct InitTemplate {
    path: &'static str,
    kind: InitFileKind,
    content: &'static str,
}

fn init_files(agents: bool) -> Vec<InitTemplate> {
    let mut files = vec![InitTemplate {
        path: ".dart-decimaterc",
        kind: InitFileKind::Config,
        content: DART_DECIMATE_RC,
    }];
    if agents {
        files.push(InitTemplate {
            path: "AGENTS.md",
            kind: InitFileKind::Agents,
            content: AGENTS_MD,
        });
    }
    files
}

fn write_init_file(path: &Path, content: &str, force: bool) -> Result<InitFileAction, InitError> {
    let action = if path.exists() {
        if !force {
            return Err(InitError::AlreadyExists {
                path: path.to_path_buf(),
            });
        }
        InitFileAction::Overwritten
    } else {
        InitFileAction::Created
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| InitError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, content).map_err(|source| InitError::Write {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(action)
}

const DART_DECIMATE_RC: &str = r#"{
  "cli": {
    "format": "json",
    "entry": ["lib/main.dart"],
    "production": true
  },
  "ignorePatterns": [
    "**/.dart_tool/**",
    "**/build/**",
    "**/generated/**",
    "**/*.g.dart",
    "**/*.freezed.dart",
    "**/*.gen.dart",
    "**/*.gr.dart",
    "**/*.mocks.dart"
  ],
  "health": {
    "fileScores": true,
    "hotspots": true,
    "targets": true
  },
  "security": {
    "surface": true
  },
  "rules": {
    "dart-decimate/missing-suppression-reason": "warn"
  }
}
"#;

const AGENTS_MD: &str = r"# Dart Decimate Agent Guide

Use Dart Decimate as the first static codebase-intelligence pass for Dart and Flutter changes.

- Run `dart-decimate check --format json` for cleanup, dependency, health, flag, security, and graph findings.
- Run `dart-decimate audit --format json --base origin/main` for changed-code review.
- Inspect before editing with `dart-decimate inspect --format json --file <path>` or `dart-decimate inspect --format json --symbol <file>:<symbol>`.
- Apply only actions with `safe_to_delete: true`, then rerun the same command that produced the finding.

Dart Decimate is graph-first. Treat unresolved dependencies, generated files, build outputs, and runtime-only entry points as evidence to verify before deleting code.
";
