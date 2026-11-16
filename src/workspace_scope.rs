use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use glob::Pattern;
use thiserror::Error;

use crate::graph::normalize_path;
use crate::{LocalPubPackage, ScannedProject};

/// Errors returned while applying a pub workspace selector.
#[derive(Debug, Error)]
pub enum WorkspaceScopeError {
    /// A workspace selector glob was invalid.
    #[error("invalid workspace pattern {pattern:?}: {source}")]
    Pattern {
        /// Raw selector pattern.
        pattern: String,
        /// Underlying glob parse error.
        source: glob::PatternError,
    },
    /// No local packages matched the requested selector.
    #[error("no local pub packages matched workspace selector {patterns:?}")]
    NoMatches {
        /// Raw selector values.
        patterns: Vec<String>,
    },
}

/// # Errors
///
/// Returns [`WorkspaceScopeError`] for invalid glob syntax or empty selection.
pub fn workspace_file_scope(
    project: &ScannedProject,
    packages: &[LocalPubPackage],
    patterns: &[String],
) -> Result<Vec<PathBuf>, WorkspaceScopeError> {
    let selectors = WorkspaceSelectors::parse(patterns)?;
    let selected_roots = selectors.selected_roots(&project.root, packages)?;
    Ok(file_scope_for_roots(project, packages, &selected_roots))
}

/// Return root-normalized files of the packages that changed paths touch.
#[must_use]
pub fn changed_workspace_file_scope(
    project: &ScannedProject,
    packages: &[LocalPubPackage],
    changed_paths: &[PathBuf],
) -> Vec<PathBuf> {
    let all_roots = packages
        .iter()
        .map(|package| package.root.clone())
        .collect::<Vec<_>>();
    let selected_roots = changed_paths
        .iter()
        .filter_map(|path| owning_root(&all_roots, path))
        .cloned()
        .collect::<BTreeSet<_>>();

    file_scope_for_roots(project, packages, &selected_roots)
}

fn file_scope_for_roots(
    project: &ScannedProject,
    packages: &[LocalPubPackage],
    selected_roots: &BTreeSet<PathBuf>,
) -> Vec<PathBuf> {
    let all_roots = packages
        .iter()
        .map(|package| package.root.clone())
        .collect::<Vec<_>>();
    let mut scope = BTreeSet::new();

    for file in &project.files {
        let Some(owner) = owning_root(&all_roots, &file.path) else {
            continue;
        };
        if selected_roots.contains(owner) {
            scope.insert(file.path.clone());
        }
    }

    for package in packages {
        if selected_roots.contains(&package.root) {
            scope.insert(package.pubspec_path.clone());
        }
    }

    scope.into_iter().collect()
}

struct WorkspaceSelectors {
    include: Vec<WorkspacePattern>,
    exclude: Vec<WorkspacePattern>,
    raw: Vec<String>,
}

impl WorkspaceSelectors {
    fn parse(values: &[String]) -> Result<Self, WorkspaceScopeError> {
        let mut include = Vec::new();
        let mut exclude = Vec::new();
        let mut raw = Vec::new();

        for value in values {
            for item in value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
            {
                let (negated, pattern) = item
                    .strip_prefix('!')
                    .map_or((false, item), |pattern| (true, pattern.trim()));
                let pattern = trim_path_pattern(pattern);
                if pattern.is_empty() {
                    continue;
                }
                raw.push(item.to_owned());
                let selector = WorkspacePattern::new(pattern)?;
                if negated {
                    exclude.push(selector);
                } else {
                    include.push(selector);
                }
            }
        }

        Ok(Self {
            include,
            exclude,
            raw,
        })
    }

    fn selected_roots(
        &self,
        root: &Path,
        packages: &[LocalPubPackage],
    ) -> Result<BTreeSet<PathBuf>, WorkspaceScopeError> {
        let mut selected = packages
            .iter()
            .filter(|package| {
                self.include.is_empty()
                    || self
                        .include
                        .iter()
                        .any(|pattern| pattern.matches(root, package))
            })
            .filter(|package| {
                !self
                    .exclude
                    .iter()
                    .any(|pattern| pattern.matches(root, package))
            })
            .map(|package| package.root.clone())
            .collect::<BTreeSet<_>>();

        if selected.is_empty() {
            return Err(WorkspaceScopeError::NoMatches {
                patterns: self.raw.clone(),
            });
        }

        selected = selected
            .into_iter()
            .map(|path| normalize_path(&path))
            .collect();
        Ok(selected)
    }
}

struct WorkspacePattern {
    glob: Pattern,
}

impl WorkspacePattern {
    fn new(pattern: &str) -> Result<Self, WorkspaceScopeError> {
        Ok(Self {
            glob: Pattern::new(pattern).map_err(|source| WorkspaceScopeError::Pattern {
                pattern: pattern.to_owned(),
                source,
            })?,
        })
    }

    fn matches(&self, root: &Path, package: &LocalPubPackage) -> bool {
        package_candidates(root, package)
            .iter()
            .any(|candidate| self.glob.matches(candidate))
    }
}

fn package_candidates(root: &Path, package: &LocalPubPackage) -> Vec<String> {
    let relative = display_path(root, &package.root);
    let relative = if relative.is_empty() {
        ".".to_owned()
    } else {
        relative
    };
    let absolute = display_path(Path::new(""), &package.root);
    [package.name.clone(), relative, absolute]
        .into_iter()
        .collect()
}

fn owning_root<'root>(roots: &'root [PathBuf], path: &Path) -> Option<&'root PathBuf> {
    roots
        .iter()
        .filter(|root| path.starts_with(root))
        .max_by_key(|root| root.components().count())
}

fn trim_path_pattern(pattern: &str) -> &str {
    pattern.trim().trim_end_matches('/')
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => value.to_str(),
            std::path::Component::RootDir => Some(""),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan_fixture(files: &[&str]) -> Result<ScannedProject, Box<dyn std::error::Error>> {
        let fixture = tempfile::tempdir()?;
        write(&fixture, "packages/app/pubspec.yaml", "name: app\n")?;
        write(&fixture, "packages/shared/pubspec.yaml", "name: shared\n")?;
        write(
            &fixture,
            "packages/app/example/pubspec.yaml",
            "name: example\n",
        )?;
        for file in files {
            write(&fixture, file, "void value() {}\n")?;
        }
        Ok(crate::scan_project(fixture.path())?)
    }

    fn package(root: &Path, name: &str, path: &str) -> LocalPubPackage {
        LocalPubPackage {
            name: name.to_owned(),
            root: root.join(path),
            pubspec_path: root.join(path).join("pubspec.yaml"),
        }
    }

    fn write(fixture: &tempfile::TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
        let path = fixture.path().join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, source)
    }

    #[test]
    fn matches_name_path_glob_and_excludes() -> Result<(), Box<dyn std::error::Error>> {
        let project = scan_fixture(&[
            "packages/app/lib/main.dart",
            "packages/shared/lib/shared.dart",
        ])?;
        let root = project.root.clone();
        let packages = vec![
            package(&root, "app", "packages/app"),
            package(&root, "shared", "packages/shared"),
        ];

        let scope =
            workspace_file_scope(&project, &packages, &[String::from("packages/*,!shared")])?;

        assert_eq!(
            scope,
            vec![
                root.join("packages/app/lib/main.dart"),
                root.join("packages/app/pubspec.yaml"),
            ]
        );
        Ok(())
    }

    #[test]
    fn uses_longest_package_root_for_nested_packages() -> Result<(), Box<dyn std::error::Error>> {
        let project = scan_fixture(&[
            "packages/app/lib/main.dart",
            "packages/app/example/lib/main.dart",
        ])?;
        let root = project.root.clone();
        let packages = vec![
            package(&root, "app", "packages/app"),
            package(&root, "example", "packages/app/example"),
        ];

        let scope = workspace_file_scope(&project, &packages, &[String::from("app")])?;

        assert!(scope.contains(&root.join("packages/app/lib/main.dart")));
        assert!(!scope.contains(&root.join("packages/app/example/lib/main.dart")));
        Ok(())
    }

    #[test]
    fn errors_when_no_package_matches() -> Result<(), Box<dyn std::error::Error>> {
        let project = scan_fixture(&["packages/app/lib/main.dart"])?;
        let root = project.root.clone();
        let packages = vec![package(&root, "app", "packages/app")];

        let Err(error) = workspace_file_scope(&project, &packages, &[String::from("missing")])
        else {
            panic!("missing selector should error");
        };

        assert!(matches!(error, WorkspaceScopeError::NoMatches { .. }));
        Ok(())
    }

    #[test]
    fn changed_paths_select_owning_workspace() -> Result<(), Box<dyn std::error::Error>> {
        let project = scan_fixture(&[
            "packages/app/lib/main.dart",
            "packages/shared/lib/shared.dart",
        ])?;
        let root = project.root.clone();
        let packages = vec![
            package(&root, "app", "packages/app"),
            package(&root, "shared", "packages/shared"),
        ];

        let scope = changed_workspace_file_scope(
            &project,
            &packages,
            &[root.join("packages/shared/README.md")],
        );

        assert_eq!(
            scope,
            vec![
                root.join("packages/shared/lib/shared.dart"),
                root.join("packages/shared/pubspec.yaml"),
            ]
        );
        Ok(())
    }
}
