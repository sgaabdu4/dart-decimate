use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

mod agent_settings;
mod templates;

use agent_settings::{claude_settings_status, install_claude_settings, uninstall_claude_settings};
use templates::{agent_hook_script, agents_block};

/// Stable schema version for hook management output.
pub const HOOKS_SCHEMA_VERSION: &str = "dart-decimate.hooks.v1";

pub(super) const HOOK_MARKER: &str = "dart-decimate-managed-hook";
const AGENT_SCRIPT_PATH: &str = ".claude/hooks/dart-decimate-gate.sh";
const CLAUDE_SETTINGS_PATH: &str = ".claude/settings.json";
const AGENTS_PATH: &str = "AGENTS.md";
pub(super) const AGENT_COMMAND: &str =
    "\"$CLAUDE_PROJECT_DIR\"/.claude/hooks/dart-decimate-gate.sh";
pub(super) const AGENTS_BLOCK_START: &str = "<!-- dart-decimate-managed-hook:start -->";
pub(super) const AGENTS_BLOCK_END: &str = "<!-- dart-decimate-managed-hook:end -->";

/// Hook installation target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookTarget {
    /// Git pre-commit hook under `.git/hooks/pre-commit`.
    Git,
    /// Claude Code and repository agent guidance hook surfaces.
    Agent,
}

impl HookTarget {
    /// CLI target string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Agent => "agent",
        }
    }
}

/// Hook management options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookOptions {
    /// Hook target.
    pub target: HookTarget,
    /// Git base branch used by generated hooks.
    pub branch: String,
    /// Overwrite or remove existing unmanaged hook files.
    pub force: bool,
}

/// Machine-readable hook report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HooksReport {
    /// Schema identifier.
    pub schema_version: String,
    /// Typed JSON envelope discriminator.
    pub kind: String,
    /// Tool name.
    pub tool: String,
    /// Command that produced this report.
    pub command: String,
    /// Project root.
    pub root: PathBuf,
    /// Hook target.
    pub target: HookTarget,
    /// Git base branch used by generated hooks.
    pub branch: String,
    /// Target hook files.
    pub files: Vec<HookFile>,
}

/// Hook file status or mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookFile {
    /// Path relative to root.
    pub path: String,
    /// Whether the hook file exists.
    pub installed: bool,
    /// Whether Dart Decimate owns the hook file content.
    pub managed: bool,
    /// Action performed.
    pub action: HookAction,
}

/// Hook action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookAction {
    /// Status only; no changes.
    Unchanged,
    /// Hook file was created.
    Created,
    /// Managed hook file was overwritten.
    Overwritten,
    /// Managed hook file was removed.
    Removed,
    /// Hook file did not exist.
    Missing,
}

/// Hook errors.
#[derive(Debug, Error)]
pub enum HooksError {
    /// Git hook target requires an initialized Git repository.
    #[error("cannot install git hook because {path} does not exist")]
    MissingGitDir {
        /// Missing `.git` directory.
        path: PathBuf,
    },
    /// Existing hook is not Dart Decimate-managed and `force` was not set.
    #[error("refusing to overwrite unmanaged hook {path}; pass --force")]
    UnmanagedHook {
        /// Existing hook path.
        path: PathBuf,
    },
    /// Directory creation failed.
    #[error("failed to create hook directory {path}: {source}")]
    CreateDir {
        /// Directory path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Hook file write failed.
    #[error("failed to write hook {path}: {source}")]
    Write {
        /// Hook path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Hook file read failed.
    #[error("failed to read hook {path}: {source}")]
    Read {
        /// Hook path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Hook permission update failed.
    #[error("failed to mark hook executable {path}: {source}")]
    Permissions {
        /// Hook path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Hook removal failed.
    #[error("failed to remove hook {path}: {source}")]
    Remove {
        /// Hook path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
}

/// # Errors
///
/// Returns [`HooksError`] if an existing hook cannot be read.
pub fn hooks_status(
    root: impl AsRef<Path>,
    options: &HookOptions,
) -> Result<HooksReport, HooksError> {
    let root = root.as_ref();
    let files = match options.target {
        HookTarget::Git => {
            vec![hook_status_file(
                root,
                &git_hook_path(root),
                HookAction::Unchanged,
            )?]
        }
        HookTarget::Agent => agent_status_files(root)?,
    };
    Ok(report("hooks status", root, options, files))
}

/// # Errors
///
/// Returns [`HooksError`] when the target is not available, the hook is unmanaged without `force`, or file writes fail.
pub fn install_hooks(
    root: impl AsRef<Path>,
    options: &HookOptions,
) -> Result<HooksReport, HooksError> {
    let root = root.as_ref();
    let files = match options.target {
        HookTarget::Git => {
            let path = git_hook_path(root);
            ensure_git_dir(root)?;
            let action = write_git_hook(root, &path, options)?;
            vec![hook_status_file(root, &path, action)?]
        }
        HookTarget::Agent => install_agent_hooks(root, options)?,
    };
    Ok(report("hooks install", root, options, files))
}

/// # Errors
///
/// Returns [`HooksError`] when an unmanaged hook exists without `force`, or when file removal fails.
pub fn uninstall_hooks(
    root: impl AsRef<Path>,
    options: &HookOptions,
) -> Result<HooksReport, HooksError> {
    let root = root.as_ref();
    let files = match options.target {
        HookTarget::Git => vec![remove_git_hook(root, &git_hook_path(root), options.force)?],
        HookTarget::Agent => uninstall_agent_hooks(root, options.force)?,
    };
    Ok(report("hooks uninstall", root, options, files))
}

/// Render a human-readable hook report.
#[must_use]
pub fn render_hooks_report(report: &HooksReport) -> String {
    let Some(file) = report.files.first() else {
        return format!(
            "Hooks: {} target {}\n",
            report.command,
            report.target.as_str()
        );
    };
    let state = if file.installed {
        if file.managed {
            "installed"
        } else {
            "unmanaged"
        }
    } else {
        "not installed"
    };
    format!(
        "Hooks: {} target {} ({state})\n{}: {:?}\n",
        report.command,
        report.target.as_str(),
        file.path,
        file.action
    )
}

fn report(command: &str, root: &Path, options: &HookOptions, files: Vec<HookFile>) -> HooksReport {
    HooksReport {
        schema_version: HOOKS_SCHEMA_VERSION.to_owned(),
        kind: "hooks".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: command.to_owned(),
        root: root.to_path_buf(),
        target: options.target,
        branch: options.branch.clone(),
        files,
    }
}

fn ensure_git_dir(root: &Path) -> Result<(), HooksError> {
    let path = root.join(".git");
    if path.is_dir() {
        Ok(())
    } else {
        Err(HooksError::MissingGitDir { path })
    }
}

fn git_hook_path(root: &Path) -> PathBuf {
    root.join(".git/hooks/pre-commit")
}

fn agent_script_path(root: &Path) -> PathBuf {
    root.join(AGENT_SCRIPT_PATH)
}

pub(super) fn claude_settings_path(root: &Path) -> PathBuf {
    root.join(CLAUDE_SETTINGS_PATH)
}

fn agents_path(root: &Path) -> PathBuf {
    root.join(AGENTS_PATH)
}

fn hook_status_file(root: &Path, path: &Path, action: HookAction) -> Result<HookFile, HooksError> {
    let installed = path.is_file();
    let managed = installed && read_hook(path)?.contains(HOOK_MARKER);
    Ok(HookFile {
        path: relative_path(root, path),
        installed,
        managed,
        action,
    })
}

fn status_file_with_marker(
    root: &Path,
    path: &Path,
    action: HookAction,
    marker: &str,
) -> Result<HookFile, HooksError> {
    let installed = path.is_file();
    let managed = installed && read_hook(path)?.contains(marker);
    Ok(HookFile {
        path: relative_path(root, path),
        installed,
        managed,
        action,
    })
}

pub(super) fn missing_file(root: &Path, path: &Path) -> HookFile {
    HookFile {
        path: relative_path(root, path),
        installed: false,
        managed: false,
        action: HookAction::Missing,
    }
}

fn agent_status_files(root: &Path) -> Result<Vec<HookFile>, HooksError> {
    Ok(vec![
        hook_status_file(root, &agent_script_path(root), HookAction::Unchanged)?,
        claude_settings_status(root, HookAction::Unchanged)?,
        status_file_with_marker(
            root,
            &agents_path(root),
            HookAction::Unchanged,
            AGENTS_BLOCK_START,
        )?,
    ])
}

fn install_agent_hooks(root: &Path, options: &HookOptions) -> Result<Vec<HookFile>, HooksError> {
    Ok(vec![
        write_managed_file(
            root,
            &agent_script_path(root),
            &agent_hook_script(&options.branch),
            options.force,
            true,
        )?,
        install_claude_settings(root, options.force)?,
        install_agents_block(root, &options.branch)?,
    ])
}

fn uninstall_agent_hooks(root: &Path, force: bool) -> Result<Vec<HookFile>, HooksError> {
    Ok(vec![
        remove_git_hook(root, &agent_script_path(root), force)?,
        uninstall_claude_settings(root, force)?,
        uninstall_agents_block(root)?,
    ])
}

fn write_managed_file(
    root: &Path,
    path: &Path,
    source: &str,
    force: bool,
    executable: bool,
) -> Result<HookFile, HooksError> {
    let action = if path.exists() {
        if !read_hook(path)?.contains(HOOK_MARKER) && !force {
            return Err(HooksError::UnmanagedHook {
                path: path.to_path_buf(),
            });
        }
        HookAction::Overwritten
    } else {
        HookAction::Created
    };
    write_text(path, source)?;
    if executable {
        mark_executable(path)?;
    }
    hook_status_file(root, path, action)
}

fn write_git_hook(
    root: &Path,
    path: &Path,
    options: &HookOptions,
) -> Result<HookAction, HooksError> {
    let action = if path.exists() {
        if !read_hook(path)?.contains(HOOK_MARKER) && !options.force {
            return Err(HooksError::UnmanagedHook {
                path: path.to_path_buf(),
            });
        }
        HookAction::Overwritten
    } else {
        HookAction::Created
    };
    write_text(path, &git_hook_script(&options.branch))?;
    mark_executable(path)?;
    let _ = root;
    Ok(action)
}

fn install_agents_block(root: &Path, branch: &str) -> Result<HookFile, HooksError> {
    let path = agents_path(root);
    let block = agents_block(branch);
    let (source, action) = if path.exists() {
        let source = read_hook(&path)?;
        if source.contains(AGENTS_BLOCK_START) && source.contains(AGENTS_BLOCK_END) {
            (
                replace_managed_block(&source, &block),
                HookAction::Overwritten,
            )
        } else {
            (
                append_managed_block(&source, &block),
                HookAction::Overwritten,
            )
        }
    } else {
        (block, HookAction::Created)
    };
    write_text(&path, &source)?;
    status_file_with_marker(root, &path, action, AGENTS_BLOCK_START)
}

fn uninstall_agents_block(root: &Path) -> Result<HookFile, HooksError> {
    let path = agents_path(root);
    if !path.exists() {
        return Ok(missing_file(root, &path));
    }
    let source = read_hook(&path)?;
    if !source.contains(AGENTS_BLOCK_START) || !source.contains(AGENTS_BLOCK_END) {
        return status_file_with_marker(root, &path, HookAction::Unchanged, AGENTS_BLOCK_START);
    }
    write_text(&path, &replace_managed_block(&source, ""))?;
    status_file_with_marker(root, &path, HookAction::Removed, AGENTS_BLOCK_START)
}

fn replace_managed_block(source: &str, replacement: &str) -> String {
    let Some(start) = source.find(AGENTS_BLOCK_START) else {
        return source.to_owned();
    };
    let Some(end) = source.find(AGENTS_BLOCK_END) else {
        return source.to_owned();
    };
    let end = end + AGENTS_BLOCK_END.len();
    let mut output = String::new();
    output.push_str(source[..start].trim_end());
    if !replacement.is_empty() {
        if !output.is_empty() {
            output.push_str("\n\n");
        }
        output.push_str(replacement.trim());
    }
    output.push_str(source[end..].trim_start_matches('\n'));
    if !output.ends_with('\n') {
        output.push('\n');
    }
    output
}

fn append_managed_block(source: &str, block: &str) -> String {
    let mut output = source.trim_end().to_owned();
    if !output.is_empty() {
        output.push_str("\n\n");
    }
    output.push_str(block.trim());
    output.push('\n');
    output
}

pub(super) fn write_text(path: &Path, source: &str) -> Result<(), HooksError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| HooksError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, source).map_err(|source| HooksError::Write {
        path: path.to_path_buf(),
        source,
    })
}

fn remove_git_hook(root: &Path, path: &Path, force: bool) -> Result<HookFile, HooksError> {
    if !path.exists() {
        return Ok(HookFile {
            path: relative_path(root, path),
            installed: false,
            managed: false,
            action: HookAction::Missing,
        });
    }
    let managed = read_hook(path)?.contains(HOOK_MARKER);
    if !managed && !force {
        return Err(HooksError::UnmanagedHook {
            path: path.to_path_buf(),
        });
    }
    fs::remove_file(path).map_err(|source| HooksError::Remove {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(HookFile {
        path: relative_path(root, path),
        installed: false,
        managed,
        action: HookAction::Removed,
    })
}

pub(super) fn read_hook(path: &Path) -> Result<String, HooksError> {
    fs::read_to_string(path).map_err(|source| HooksError::Read {
        path: path.to_path_buf(),
        source,
    })
}

fn mark_executable(path: &Path) -> Result<(), HooksError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|source| HooksError::Permissions {
                path: path.to_path_buf(),
                source,
            })?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).map_err(|source| HooksError::Permissions {
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

fn git_hook_script(branch: &str) -> String {
    format!(
        r#"#!/bin/sh
# {HOOK_MARKER}
set -eu
BASE="${{DART_DECIMATE_BASE:-{branch}}}"
if ! command -v dart-decimate >/dev/null 2>&1; then
  echo "dart-decimate hook: dart-decimate binary not found" >&2
  exit 2
fi
dart-decimate audit . --base "$BASE" --format json --summary
"#
    )
}

pub(super) fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
