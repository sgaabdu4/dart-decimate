use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use glob::Pattern;
use ignore::WalkBuilder;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::graph::normalize_path;
use crate::package_map::PackageMap;
use crate::{
    DartFile, ExtractError, GraphError, GraphOptions, ModuleGraph, build_module_graph_with_options,
    extract_dart_file,
};

/// Parsed Dart files plus their resolved module graph.
#[derive(Debug)]
pub struct ScannedProject {
    /// Primary root used for dependency resolution and root-relative output.
    pub root: PathBuf,
    /// Local roots included in discovery, sorted for deterministic entrypoint analysis.
    pub scan_roots: Vec<PathBuf>,
    /// Extracted Dart files, sorted by path for deterministic output.
    pub files: Vec<DartFile>,
    /// Directed graph built from the extracted files.
    pub graph: ModuleGraph,
}

/// Options controlling Dart file discovery.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScanOptions {
    /// Glob patterns excluded from Dart file discovery.
    pub ignore_patterns: Vec<String>,
    /// Dart conditional URI environment values. Empty means include every branch.
    pub conditional_environment: BTreeMap<String, String>,
}

/// Errors returned while scanning a Dart or Flutter project.
#[derive(Debug, Error)]
pub enum ScanError {
    /// The current working directory could not be read.
    #[error("failed to read current directory: {source}")]
    CurrentDir {
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A scan root could not be traversed.
    #[error("failed to walk directory {path}: {source}")]
    Walk {
        /// Directory path.
        path: PathBuf,
        /// Underlying ignore-aware walk error.
        source: ignore::Error,
    },
    /// A pubspec could not be read while expanding local packages.
    #[error("failed to read pubspec {path}: {source}")]
    ReadPubspec {
        /// Pubspec path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A pubspec could not be parsed while expanding local packages.
    #[error("failed to parse pubspec {path}: {source}")]
    ParsePubspec {
        /// Pubspec path.
        path: PathBuf,
        /// Underlying YAML parse error.
        source: serde_yaml_ng::Error,
    },
    /// A pub workspace glob pattern was invalid.
    #[error("invalid pub workspace glob pattern {pattern}: {source}")]
    WorkspacePattern {
        /// Invalid pattern.
        pattern: String,
        /// Underlying glob pattern error.
        source: glob::PatternError,
    },
    /// A configured ignore glob pattern was invalid.
    #[error("invalid ignore pattern {pattern}: {source}")]
    IgnorePattern {
        /// Invalid ignore pattern.
        pattern: String,
        /// Underlying glob pattern error.
        source: glob::PatternError,
    },
    /// A pub workspace glob expansion failed.
    #[error("failed to expand pub workspace glob {pattern}: {source}")]
    WorkspaceGlob {
        /// Pattern being expanded.
        pattern: String,
        /// Underlying glob error.
        source: glob::GlobError,
    },
    /// A Dart file could not be parsed.
    #[error(transparent)]
    Extract(#[from] ExtractError),
    /// A module graph could not be built.
    #[error(transparent)]
    Graph(#[from] GraphError),
}

/// Discover, parse, and graph all `.dart` files under `root` and discovered
/// local package roots.
///
/// Parsing is parallelized with Rayon after a deterministic filesystem walk.
///
/// # Errors
///
/// Returns [`ScanError`] when discovery, parsing, or graph construction fails.
pub fn scan_project(root: impl AsRef<Path>) -> Result<ScannedProject, ScanError> {
    scan_project_with_options(root, &ScanOptions::default())
}

/// Discover, parse, and graph all non-ignored `.dart` files under `root` and
/// discovered local package roots.
///
/// Parsing is parallelized with Rayon after a deterministic filesystem walk.
///
/// # Errors
///
/// Returns [`ScanError`] when discovery, parsing, or graph construction fails.
pub fn scan_project_with_options(
    root: impl AsRef<Path>,
    options: &ScanOptions,
) -> Result<ScannedProject, ScanError> {
    let root = normalize_scan_root(root.as_ref())?;
    let ignore_matcher = IgnoreMatcher::new(&options.ignore_patterns)?;
    let package_map = PackageMap::discover(&root)?;
    let mut scan_roots = BTreeSet::new();
    scan_roots.insert(root.clone());
    scan_roots.extend(package_map.local_roots());

    let mut paths = BTreeSet::new();
    for scan_root in &scan_roots {
        discover_dart_files(&root, scan_root, &ignore_matcher, &mut paths)?;
    }
    let paths = paths.into_iter().collect::<Vec<_>>();

    let mut files = paths
        .par_iter()
        .map(extract_dart_file)
        .collect::<Result<Vec<_>, _>>()?;
    files.sort_by(|left, right| left.path.cmp(&right.path));

    let graph = build_module_graph_with_options(
        &root,
        &files,
        &GraphOptions {
            conditional_environment: options.conditional_environment.clone(),
        },
    )?;

    Ok(ScannedProject {
        root,
        scan_roots: scan_roots.into_iter().collect(),
        files,
        graph,
    })
}

fn normalize_scan_root(root: &Path) -> Result<PathBuf, ScanError> {
    if root.is_absolute() {
        return Ok(normalize_path(root));
    }

    let current_dir = std::env::current_dir().map_err(|source| ScanError::CurrentDir { source })?;
    Ok(normalize_path(&current_dir.join(root)))
}

fn discover_dart_files(
    root: &Path,
    dir: &Path,
    ignore_matcher: &IgnoreMatcher,
    paths: &mut BTreeSet<PathBuf>,
) -> Result<(), ScanError> {
    let filter_root = root.to_path_buf();
    let filter_matcher = ignore_matcher.clone();
    let mut builder = WalkBuilder::new(dir);
    builder
        .standard_filters(false)
        .git_ignore(true)
        .require_git(false)
        .filter_entry(move |entry| {
            entry.depth() == 0
                || !(filter_matcher.matches(&filter_root, entry.path())
                    || entry
                        .file_type()
                        .is_some_and(|file_type| file_type.is_dir())
                        && should_skip_dir(entry.path()))
        });

    for result in builder.build() {
        let entry = result.map_err(|source| ScanError::Walk {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
            && path
                .extension()
                .is_some_and(|extension| extension == "dart")
        {
            paths.insert(normalize_path(path));
        }
    }

    Ok(())
}

fn should_skip_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".dart_tool" | ".git" | ".idea" | ".pub-cache" | "build" | "target")
    )
}

#[derive(Debug, Clone)]
struct IgnoreMatcher {
    patterns: Vec<Pattern>,
}

impl IgnoreMatcher {
    fn new(patterns: &[String]) -> Result<Self, ScanError> {
        let patterns = patterns
            .iter()
            .map(|pattern| {
                Pattern::new(pattern).map_err(|source| ScanError::IgnorePattern {
                    pattern: pattern.clone(),
                    source,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { patterns })
    }

    fn matches(&self, root: &Path, path: &Path) -> bool {
        if self.patterns.is_empty() {
            return false;
        }

        let normalized = normalize_path(path);
        let relative = normalized.strip_prefix(root).unwrap_or(&normalized);
        let relative_display = slash_path(relative);
        let absolute_display = slash_path(&normalized);

        self.patterns.iter().any(|pattern| {
            pattern.matches_path(relative)
                || pattern.matches(&relative_display)
                || pattern.matches(&absolute_display)
        })
    }
}

fn slash_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => value.to_str(),
            std::path::Component::RootDir => Some(""),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests;
