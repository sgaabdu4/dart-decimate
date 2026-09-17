use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Stable schema version for CI template output.
pub const CI_TEMPLATE_SCHEMA_VERSION: &str = "dart-decimate.ci-template.v1";
pub const CI_RECONCILE_REVIEW_SCHEMA_VERSION: &str = "dart-decimate.ci-reconcile-review.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CiTemplatePlatform {
    Github,
    Gitlab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CiReviewProvider {
    Github,
    Gitlab,
}

impl CiTemplatePlatform {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::Gitlab => "gitlab",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiTemplateReport {
    pub schema_version: String,
    pub kind: String,
    pub tool: String,
    pub command: String,
    pub platform: CiTemplatePlatform,
    pub vendor: bool,
    pub files: Vec<CiTemplateFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiTemplateFile {
    pub path: String,
    pub executable: bool,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiReconcileReviewReport {
    pub schema_version: String,
    pub kind: String,
    pub tool: String,
    pub command: String,
    pub provider: CiReviewProvider,
    pub dry_run: bool,
    pub envelope: PathBuf,
    pub repo: Option<String>,
    pub project_id: Option<String>,
    pub pr: Option<String>,
    pub mr: Option<String>,
    pub api_url: Option<String>,
    pub summary: CiReconcileReviewSummary,
    pub fingerprints: Vec<String>,
    pub failed_fingerprints: Vec<String>,
    pub unapplied_fingerprints: Vec<String>,
    pub apply_hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiReconcileReviewSummary {
    pub envelope_fingerprints: usize,
    pub stale_candidates: usize,
    pub applied: usize,
    pub unapplied: usize,
    pub apply_errors: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiReconcileReviewOptions {
    pub provider: CiReviewProvider,
    pub envelope: PathBuf,
    pub dry_run: bool,
    pub repo: Option<String>,
    pub project_id: Option<String>,
    pub pr: Option<String>,
    pub mr: Option<String>,
    pub api_url: Option<String>,
}

#[derive(Debug, Error)]
pub enum CiTemplateError {
    #[error("ci reconcile-review is dry-run only in this release; pass --dry-run")]
    ReconcileDryRunRequired,
    #[error("failed to read review envelope {path}: {source}")]
    ReadEnvelope {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse review envelope {path}: {source}")]
    ParseEnvelope {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("refusing to overwrite existing CI template file {path}; pass --force")]
    AlreadyExists { path: PathBuf },
    #[error("failed to create directory {path}: {source}")]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write CI template file {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to mark CI helper executable {path}: {source}")]
    Permissions {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[must_use]
pub fn ci_template_report(platform: CiTemplatePlatform, vendor: bool) -> CiTemplateReport {
    CiTemplateReport {
        schema_version: CI_TEMPLATE_SCHEMA_VERSION.to_owned(),
        kind: "ci-template".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: "ci-template".to_owned(),
        platform,
        vendor,
        files: template_files(platform, vendor),
    }
}

/// Write the selected CI template files under `root`.
///
/// # Errors
///
/// Returns [`CiTemplateError`] when a target file already exists without
/// `force`, or when directories, files, or executable permissions cannot be
/// written.
pub fn vendor_ci_template(
    root: impl AsRef<Path>,
    platform: CiTemplatePlatform,
    force: bool,
) -> Result<CiTemplateReport, CiTemplateError> {
    let report = ci_template_report(platform, true);
    let root = root.as_ref();
    for file in &report.files {
        let path = root.join(&file.path);
        write_template_file(&path, file, force)?;
    }
    Ok(report)
}

/// Build a dry-run CI review reconciliation report.
///
/// # Errors
///
/// Returns [`CiTemplateError`] when the envelope cannot be read or parsed, or
/// when provider mutation is requested.
pub fn ci_reconcile_review_report(
    options: CiReconcileReviewOptions,
) -> Result<CiReconcileReviewReport, CiTemplateError> {
    if !options.dry_run {
        return Err(CiTemplateError::ReconcileDryRunRequired);
    }
    let envelope = &options.envelope;
    let source = fs::read_to_string(envelope).map_err(|source| CiTemplateError::ReadEnvelope {
        path: envelope.clone(),
        source,
    })?;
    let value = serde_json::from_str::<Value>(&source).map_err(|source| {
        CiTemplateError::ParseEnvelope {
            path: envelope.clone(),
            source,
        }
    })?;
    let fingerprints = envelope_fingerprints(&value);
    Ok(CiReconcileReviewReport {
        schema_version: CI_RECONCILE_REVIEW_SCHEMA_VERSION.to_owned(),
        kind: "ci-reconcile-review".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: "ci reconcile-review".to_owned(),
        provider: options.provider,
        dry_run: options.dry_run,
        envelope: envelope.clone(),
        repo: options.repo,
        project_id: options.project_id,
        pr: options.pr,
        mr: options.mr,
        api_url: options.api_url,
        summary: CiReconcileReviewSummary {
            envelope_fingerprints: fingerprints.len(),
            stale_candidates: 0,
            applied: 0,
            unapplied: fingerprints.len(),
            apply_errors: 0,
        },
        failed_fingerprints: Vec::new(),
        unapplied_fingerprints: fingerprints.clone(),
        fingerprints,
        apply_hint:
            "dry-run only: provider comment mutation is not implemented by Dart Decimate yet"
                .to_owned(),
    })
}

#[must_use]
pub fn render_ci_template(report: &CiTemplateReport) -> String {
    if !report.vendor {
        return report
            .files
            .first()
            .map_or_else(String::new, |file| file.content.clone());
    }

    report
        .files
        .iter()
        .map(|file| format!("# {}\n{}", file.path, file.content))
        .collect::<Vec<_>>()
        .join("\n")
}

fn write_template_file(
    path: &Path,
    file: &CiTemplateFile,
    force: bool,
) -> Result<(), CiTemplateError> {
    if path.exists() && !force {
        return Err(CiTemplateError::AlreadyExists {
            path: path.to_path_buf(),
        });
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CiTemplateError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, &file.content).map_err(|source| CiTemplateError::Write {
        path: path.to_path_buf(),
        source,
    })?;
    mark_executable(path, file.executable)
}

fn mark_executable(path: &Path, executable: bool) -> Result<(), CiTemplateError> {
    if !executable {
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|source| CiTemplateError::Permissions {
                path: path.to_path_buf(),
                source,
            })?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).map_err(|source| CiTemplateError::Permissions {
            path: path.to_path_buf(),
            source,
        })
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

fn template_files(platform: CiTemplatePlatform, vendor: bool) -> Vec<CiTemplateFile> {
    match (platform, vendor) {
        (CiTemplatePlatform::Github, _) => vec![template_file(
            ".github/workflows/dart-decimate.yml",
            false,
            GITHUB_WORKFLOW,
        )],
        (CiTemplatePlatform::Gitlab, false) => {
            vec![template_file(".gitlab-ci.yml", false, GITLAB_CI)]
        }
        (CiTemplatePlatform::Gitlab, true) => vec![
            template_file("ci/gitlab-ci.yml", false, GITLAB_VENDORED_CI),
            template_file("ci/scripts/review.sh", true, GITLAB_REVIEW_SCRIPT),
            template_file("ci/scripts/comment.sh", true, GITLAB_COMMENT_SCRIPT),
        ],
    }
}

fn template_file(path: &str, executable: bool, content: &str) -> CiTemplateFile {
    CiTemplateFile {
        path: path.to_owned(),
        executable,
        content: content.replace("__DART_DECIMATE_VERSION__", crate::VERSION),
    }
}

fn envelope_fingerprints(value: &Value) -> Vec<String> {
    let mut fingerprints = BTreeSet::new();
    collect_fingerprints(value, &mut fingerprints);
    fingerprints.into_iter().collect()
}

fn collect_fingerprints(value: &Value, fingerprints: &mut BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            if let Some(fingerprint) = object.get("fingerprint").and_then(Value::as_str) {
                fingerprints.insert(fingerprint.to_owned());
            }
            for value in object.values() {
                collect_fingerprints(value, fingerprints);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_fingerprints(value, fingerprints);
            }
        }
        _ => {}
    }
}

const GITHUB_WORKFLOW: &str = r"name: Dart Decimate

on:
  pull_request:
  push:
    branches: [main]

jobs:
  dart-decimate:
    runs-on: ubuntu-latest
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - run: npx --yes dart-decimate@__DART_DECIMATE_VERSION__ audit --format json --base origin/${{ github.base_ref || 'main' }} --strict
";

const GITLAB_CI: &str = r#"stages:
  - quality

dart-decimate:
  stage: quality
  image: node:24
  script:
    - npx --yes dart-decimate@__DART_DECIMATE_VERSION__ audit --format json --base "origin/${CI_MERGE_REQUEST_TARGET_BRANCH_NAME:-main}" --strict
  rules:
    - if: $CI_MERGE_REQUEST_IID
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
"#;

const GITLAB_VENDORED_CI: &str = r".dart-decimate:
  stage: quality
  image: node:24
  script:
    - ci/scripts/review.sh
  rules:
    - if: $CI_MERGE_REQUEST_IID
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
";

const GITLAB_REVIEW_SCRIPT: &str = r#"#!/usr/bin/env sh
set -eu

BASE="origin/${CI_MERGE_REQUEST_TARGET_BRANCH_NAME:-main}"
npx --yes dart-decimate@__DART_DECIMATE_VERSION__ audit --format json --base "$BASE" --strict
"#;

const GITLAB_COMMENT_SCRIPT: &str = r#"#!/usr/bin/env sh
set -eu

npx --yes dart-decimate@__DART_DECIMATE_VERSION__ review --format json --base "origin/${CI_MERGE_REQUEST_TARGET_BRANCH_NAME:-main}"
"#;
