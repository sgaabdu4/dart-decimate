use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

use crate::graph::normalize_against;
use crate::scan::ScannedProject;

/// Errors returned while computing changed file scope.
#[derive(Debug, Error)]
pub enum ChangedScopeError {
    /// Git could not be executed.
    #[error("failed to run git diff for changed scope: {source}")]
    Git {
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Git returned a non-zero status.
    #[error("git diff failed for base {base:?}: {stderr}{suggestions}")]
    GitDiff {
        /// Base revision passed by the caller.
        base: String,
        /// Stderr from git.
        stderr: String,
        /// Similar refs found in the local repository.
        suggestions: RefSuggestions,
    },
}

/// Similar Git refs for an invalid changed-scope base.
#[derive(Debug, Clone, Default)]
pub struct RefSuggestions(Vec<String>);

impl fmt::Display for RefSuggestions {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.as_slice() {
            [] => Ok(()),
            [one] => write!(formatter, "\nDid you mean {one}?"),
            many => write!(formatter, "\nDid you mean one of: {}?", many.join(", ")),
        }
    }
}

impl RefSuggestions {
    pub(crate) fn for_base(root: &Path, base: &str) -> Self {
        Self(similar_refs(root, base))
    }
}

/// Return root-normalized files changed since `base`.
///
/// # Errors
///
/// Returns [`ChangedScopeError`] if `git diff` cannot be executed or the base
/// revision is invalid for this repository.
pub fn changed_files(root: &Path, base: &str) -> Result<Vec<PathBuf>, ChangedScopeError> {
    let output = Command::new("git")
        .args(["diff", "--name-only", "--diff-filter=ACMRTUXB", base, "--"])
        .current_dir(root)
        .output()
        .map_err(|source| ChangedScopeError::Git { source })?;

    if !output.status.success() {
        return Err(git_diff_error(root, base, &output.stderr));
    }

    let untracked = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(root)
        .output()
        .map_err(|source| ChangedScopeError::Git { source })?;

    if !untracked.status.success() {
        return Err(ChangedScopeError::GitDiff {
            base: base.to_owned(),
            stderr: String::from_utf8_lossy(&untracked.stderr).trim().to_owned(),
            suggestions: RefSuggestions::default(),
        });
    }

    let mut paths = changed_paths(root, &output.stdout);
    paths.extend(changed_paths(root, &untracked.stdout));
    Ok(paths.into_iter().collect())
}

/// Return changed files plus one resolved graph hop in either direction.
///
/// # Errors
///
/// Returns [`ChangedScopeError`] if changed-file discovery fails.
pub fn changed_file_scope(
    project: &ScannedProject,
    base: &str,
) -> Result<Vec<PathBuf>, ChangedScopeError> {
    let changed = changed_files(&project.root, base)?;
    Ok(changed_file_scope_from_changed(project, &changed))
}

/// Expand an already discovered changed-file list with one graph hop.
#[must_use]
pub fn changed_file_scope_from_changed(
    project: &ScannedProject,
    changed: &[PathBuf],
) -> Vec<PathBuf> {
    expand_related_files(project, changed.to_vec())
}

fn expand_related_files(project: &ScannedProject, changed: Vec<PathBuf>) -> Vec<PathBuf> {
    let changed_set = changed.into_iter().collect::<BTreeSet<_>>();
    let mut scope = changed_set.clone();

    for dependency in project.graph.dependencies() {
        if changed_set.contains(&dependency.from_path) {
            scope.insert(dependency.to_path);
        } else if changed_set.contains(&dependency.to_path) {
            scope.insert(dependency.from_path);
        }
    }

    scope.into_iter().collect()
}

fn changed_paths(root: &Path, stdout: &[u8]) -> BTreeSet<PathBuf> {
    stdout
        .split(|byte| *byte == b'\n')
        .filter_map(|line| std::str::from_utf8(line).ok())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| normalize_changed_path(root, line))
        .collect()
}

fn normalize_changed_path(root: &Path, line: &str) -> Option<PathBuf> {
    let path = normalize_against(root, Path::new(line));
    path.starts_with(root).then_some(path)
}

fn git_diff_error(root: &Path, base: &str, stderr: &[u8]) -> ChangedScopeError {
    ChangedScopeError::GitDiff {
        base: base.to_owned(),
        stderr: String::from_utf8_lossy(stderr).trim().to_owned(),
        suggestions: RefSuggestions::for_base(root, base),
    }
}

fn similar_refs(root: &Path, base: &str) -> Vec<String> {
    let Ok(output) = Command::new("git")
        .args([
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads",
            "refs/remotes",
        ])
        .current_dir(root)
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let mut scored = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|ref_name| !ref_name.is_empty())
        .filter(|ref_name| !ref_name.ends_with("/HEAD"))
        .filter(|ref_name| *ref_name != base)
        .filter_map(|ref_name| {
            let short = ref_name.rsplit('/').next().unwrap_or(ref_name);
            let score = levenshtein(base, ref_name).min(levenshtein(base, short));
            let contains_bonus = if ref_name.contains(base) || short.contains(base) {
                0
            } else {
                score
            };
            let score = score.min(contains_bonus);
            (score <= suggestion_threshold(base)).then_some((score, ref_name.to_owned()))
        })
        .collect::<Vec<_>>();
    scored.sort();
    scored
        .into_iter()
        .map(|(_, ref_name)| ref_name)
        .take(3)
        .collect()
}

fn suggestion_threshold(value: &str) -> usize {
    2.max(value.len() / 3)
}

fn levenshtein(left: &str, right: &str) -> usize {
    let right_chars = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right_chars.len()).collect::<Vec<_>>();
    for (left_index, left_char) in left.chars().enumerate() {
        let mut current = Vec::with_capacity(right_chars.len() + 1);
        current.push(left_index + 1);
        for (right_index, right_char) in right_chars.iter().enumerate() {
            let insertion = current[right_index] + 1;
            let deletion = previous[right_index + 1] + 1;
            let substitution = previous[right_index] + usize::from(left_char != *right_char);
            current.push(insertion.min(deletion).min(substitution));
        }
        previous = current;
    }
    previous[right_chars.len()]
}
