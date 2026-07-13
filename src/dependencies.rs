use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};
use thiserror::Error;

use crate::graph::normalize_path;
use crate::{DependencyKind, ExtractError, GraphError, Location};

mod analyze;
mod codegen;
pub(crate) use codegen::file_uses_codegen_dependency;
mod discovery;
mod overrides;
mod private_src_imports;
mod pubspec_document;
mod pubspec_entry;
mod usage;
pub use analyze::analyze_dependency_hygiene;
use discovery::discover_packages;
use overrides::misconfigured_dependency_overrides;
pub use overrides::{DependencyOverrideMisconfigReason, MisconfiguredDependencyOverride};
pub use private_src_imports::PrivateSrcImport;
use private_src_imports::imports_private_src;
use pubspec_document::{declared_dependencies_from_source, dependency_location};
use usage::{DependencyUsage, allows_dev_dependency};

/// Dart pub dependency hygiene findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyHygieneReport {
    /// Declared dependencies not imported by Dart files in the declaring package.
    pub unused_dependencies: Vec<UnusedPackageDependency>,
    /// Dependency override entries whose key or value cannot be honored by Pub.
    pub misconfigured_dependency_overrides: Vec<MisconfiguredDependencyOverride>,
    /// Imported packages absent from the importing package's pubspec.
    pub unlisted_dependencies: Vec<UnlistedPackageDependency>,
    /// Imports into another package's private `lib/src` implementation tree.
    pub private_src_imports: Vec<PrivateSrcImport>,
}

/// A declared pub dependency that has no Dart import/export usage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnusedPackageDependency {
    /// Package declaring the dependency.
    pub package: String,
    /// Pubspec path that declares the dependency.
    pub pubspec_path: PathBuf,
    /// Dependency package name.
    pub dependency: String,
    /// Pubspec dependency section.
    pub section: DependencySection,
    /// Dependency hygiene issue class.
    pub issue: DependencyIssue,
    /// Best-effort location of the dependency key in `pubspec.yaml`.
    pub location: Location,
    /// Whether Dart Decimate can suggest removal from current evidence alone.
    pub safe_to_delete: bool,
}

/// A package import/export missing from the declaring package's pubspec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlistedPackageDependency {
    /// Package containing the import/export.
    pub package: String,
    /// Pubspec path for the package containing the import/export.
    pub pubspec_path: PathBuf,
    /// Dart file containing the import/export.
    pub path: PathBuf,
    /// Imported package name.
    pub dependency: String,
    /// Import/export URI.
    pub specifier: String,
    /// Whether the dependency came from an import or export directive.
    pub kind: DependencyKind,
    /// Pubspec section where the dependency is declared, if present in a wrong section.
    pub declared_section: Option<DependencySection>,
    /// Location of the import/export directive.
    pub location: Location,
}

/// A dependency declaration found in a local pubspec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclaredPackageDependency {
    /// Package declaring the dependency.
    pub package: String,
    /// Pubspec path that declares the dependency.
    pub pubspec_path: PathBuf,
    /// Dependency package name.
    pub dependency: String,
    /// Pubspec dependency section.
    pub section: DependencySection,
    /// Best-effort location of the dependency key in `pubspec.yaml`.
    pub location: Location,
}

/// A local pub package discovered under the scan root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalPubPackage {
    /// Pub package name.
    pub name: String,
    /// Package root directory.
    pub root: PathBuf,
    /// Pubspec path.
    pub pubspec_path: PathBuf,
}

/// Pub dependency section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencySection {
    /// Runtime dependencies.
    Dependencies,
    /// Development-only dependencies.
    DevDependencies,
    /// Temporary dependency overrides.
    DependencyOverrides,
}

/// Dependency hygiene issue class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DependencyIssue {
    /// Runtime dependency has no Dart import/export usage.
    UnusedRuntimeDependency,
    /// Development dependency has no Dart import/export usage.
    UnusedDevDependency,
    /// Runtime dependency is imported only from development-only files.
    TestOnlyDependency,
    /// Dependency override is absent from the resolved lockfile package graph.
    UnusedDependencyOverride,
}

impl DependencySection {
    const fn as_pubspec_key(self) -> &'static str {
        match self {
            Self::Dependencies => "dependencies",
            Self::DevDependencies => "dev_dependencies",
            Self::DependencyOverrides => "dependency_overrides",
        }
    }
}

/// Errors returned while analyzing pub dependency hygiene.
#[derive(Debug, Error)]
pub enum DependencyHygieneError {
    /// Local package resolution failed while following a generated runtime file.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// A reachable generated runtime file could not be parsed.
    #[error(transparent)]
    Extract(#[from] ExtractError),
    /// A directory could not be read.
    #[error("failed to read directory {path}: {source}")]
    ReadDir {
        /// Directory path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A directory entry could not be read.
    #[error("failed to read directory entry under {path}: {source}")]
    ReadDirEntry {
        /// Directory being scanned.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A file type could not be read.
    #[error("failed to read file type for {path}: {source}")]
    FileType {
        /// Entry path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A pubspec could not be read.
    #[error("failed to read pubspec {path}: {source}")]
    ReadPubspec {
        /// Pubspec path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A pubspec could not be parsed.
    #[error("failed to parse pubspec {path}: {source}")]
    ParsePubspec {
        /// Pubspec path.
        path: PathBuf,
        /// Underlying YAML parse error.
        source: serde_yaml_ng::Error,
    },
}

/// Find local pubspec declarations for one dependency package.
///
/// # Errors
///
/// Returns [`DependencyHygieneError`] if pubspec discovery or parsing fails.
pub fn declared_package_dependencies(
    root: &Path,
    dependency: &str,
) -> Result<Vec<DeclaredPackageDependency>, DependencyHygieneError> {
    let packages = discover_packages(root)?;
    let mut declarations = Vec::new();
    for package in packages {
        for declared in package
            .dependencies
            .into_iter()
            .filter(|declared| declared.name == dependency)
        {
            declarations.push(DeclaredPackageDependency {
                package: package.name.clone(),
                pubspec_path: declared.source_path.clone(),
                dependency: declared.name,
                section: declared.section,
                location: declared.location,
            });
        }
    }

    declarations.sort_by(|left, right| {
        (
            &left.package,
            &left.pubspec_path,
            left.section.as_pubspec_key(),
            &left.dependency,
        )
            .cmp(&(
                &right.package,
                &right.pubspec_path,
                right.section.as_pubspec_key(),
                &right.dependency,
            ))
    });
    Ok(declarations)
}

/// List local pub packages under a root.
///
/// # Errors
///
/// Returns [`DependencyHygieneError`] if pubspec discovery or parsing fails.
pub fn local_pub_packages(root: &Path) -> Result<Vec<LocalPubPackage>, DependencyHygieneError> {
    discover_packages(root).map(|packages| {
        packages
            .into_iter()
            .map(|package| LocalPubPackage {
                name: package.name,
                root: package.root,
                pubspec_path: package.pubspec_path,
            })
            .collect()
    })
}

pub(crate) fn local_pub_package_from_pubspec(
    path: &Path,
) -> Result<Option<LocalPubPackage>, DependencyHygieneError> {
    let source = read_pubspec_source(path)?;
    let value = parse_pubspec_value(path, &source)?;
    let Some(name) = string_field(&value, "name") else {
        return Ok(None);
    };
    let root = path
        .parent()
        .map_or_else(|| PathBuf::from("."), normalize_path);

    Ok(Some(LocalPubPackage {
        name: name.to_owned(),
        root,
        pubspec_path: path.to_path_buf(),
    }))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PubPackage {
    name: String,
    root: PathBuf,
    pubspec_path: PathBuf,
    dependencies: Vec<DeclaredDependency>,
    misconfigured_dependency_overrides: Vec<MisconfiguredDependencyOverride>,
    locked_packages: Option<BTreeSet<String>>,
}

impl PubPackage {
    fn declares_dependency_for_path(&self, dependency: &str, path: &Path) -> bool {
        let allow_dev_dependencies = allows_dev_dependency(&self.root, path);
        self.dependencies.iter().any(|declared| {
            declared.name == dependency
                && match declared.section {
                    DependencySection::Dependencies => true,
                    DependencySection::DevDependencies => allow_dev_dependencies,
                    DependencySection::DependencyOverrides => false,
                }
        })
    }

    fn declared_section(&self, specifier: &str) -> Option<DependencySection> {
        let dependency = package_name(specifier)?;
        self.dependencies
            .iter()
            .find(|declared| declared.name == dependency)
            .map(|declared| declared.section)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeclaredDependency {
    name: String,
    source_path: PathBuf,
    section: DependencySection,
    location: Location,
    safe_to_delete: bool,
}

fn dependency_issue(
    dependency: &DeclaredDependency,
    usage: Option<&DependencyUsage>,
    locked_packages: Option<&BTreeSet<String>>,
) -> Option<DependencyIssue> {
    match dependency.section {
        DependencySection::Dependencies => match usage {
            Some(usage) if usage.production => None,
            Some(usage) if usage.development => Some(DependencyIssue::TestOnlyDependency),
            Some(_) | None => Some(DependencyIssue::UnusedRuntimeDependency),
        },
        DependencySection::DevDependencies => usage
            .is_none()
            .then_some(DependencyIssue::UnusedDevDependency),
        DependencySection::DependencyOverrides => locked_packages.and_then(|locked| {
            (!locked.contains(&dependency.name))
                .then_some(DependencyIssue::UnusedDependencyOverride)
        }),
    }
}

pub(super) fn read_package(path: &Path) -> Result<Option<PubPackage>, DependencyHygieneError> {
    let source = read_pubspec_source(path)?;
    let value = parse_pubspec_value(path, &source)?;

    let Some(name) = string_field(&value, "name") else {
        return Ok(None);
    };

    let root = path
        .parent()
        .map_or_else(|| PathBuf::from("."), normalize_path);
    let overrides = read_pubspec_overrides(&root)?;

    Ok(Some(PubPackage {
        name: name.to_owned(),
        root,
        pubspec_path: path.to_path_buf(),
        dependencies: merged_declared_dependencies(&value, &source, path, overrides.as_ref()),
        misconfigured_dependency_overrides: merged_misconfigured_dependency_overrides(
            &value,
            &source,
            name,
            path,
            overrides.as_ref(),
        ),
        locked_packages: read_locked_packages(path),
    }))
}

#[derive(Debug, Clone)]
struct PubspecOverridesDocument {
    path: PathBuf,
    source: String,
    value: Value,
}

fn read_pubspec_source(path: &Path) -> Result<String, DependencyHygieneError> {
    fs::read_to_string(path).map_err(|source| DependencyHygieneError::ReadPubspec {
        path: path.to_path_buf(),
        source,
    })
}

fn parse_pubspec_value(path: &Path, source: &str) -> Result<Value, DependencyHygieneError> {
    serde_yaml_ng::from_str::<Value>(source).map_err(|source| {
        DependencyHygieneError::ParsePubspec {
            path: path.to_path_buf(),
            source,
        }
    })
}

fn read_pubspec_overrides(
    package_root: &Path,
) -> Result<Option<PubspecOverridesDocument>, DependencyHygieneError> {
    let path = package_root.join("pubspec_overrides.yaml");
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(DependencyHygieneError::ReadPubspec { path, source }),
    };
    let value = parse_pubspec_value(&path, &source)?;
    Ok(Some(PubspecOverridesDocument {
        path,
        source,
        value,
    }))
}

fn read_locked_packages(pubspec_path: &Path) -> Option<BTreeSet<String>> {
    let lock_path = pubspec_path.with_file_name("pubspec.lock");
    let source = fs::read_to_string(lock_path).ok()?;
    let value = serde_yaml_ng::from_str::<Value>(&source).ok()?;
    let packages = mapping_field(&value, "packages")?;
    Some(
        packages
            .keys()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect(),
    )
}

fn merged_declared_dependencies(
    value: &Value,
    source: &str,
    source_path: &Path,
    overrides: Option<&PubspecOverridesDocument>,
) -> Vec<DeclaredDependency> {
    let mut dependencies = declared_dependencies(value, source, source_path);
    if let Some(overrides) = overrides.filter(|document| {
        has_top_level_field(
            &document.value,
            DependencySection::DependencyOverrides.as_pubspec_key(),
        )
    }) {
        dependencies
            .retain(|dependency| dependency.section != DependencySection::DependencyOverrides);
        dependencies.extend(declared_dependencies_for_sections(
            &overrides.value,
            &overrides.source,
            &overrides.path,
            &[DependencySection::DependencyOverrides],
        ));
    }
    dependencies
}

fn merged_misconfigured_dependency_overrides(
    value: &Value,
    source: &str,
    package: &str,
    path: &Path,
    overrides: Option<&PubspecOverridesDocument>,
) -> Vec<MisconfiguredDependencyOverride> {
    if let Some(overrides) = overrides.filter(|document| {
        has_top_level_field(
            &document.value,
            DependencySection::DependencyOverrides.as_pubspec_key(),
        )
    }) {
        return misconfigured_dependency_overrides(
            &overrides.value,
            &overrides.source,
            package,
            &overrides.path,
        );
    }
    misconfigured_dependency_overrides(value, source, package, path)
}

fn declared_dependencies(
    value: &Value,
    source: &str,
    source_path: &Path,
) -> Vec<DeclaredDependency> {
    let has_declared_sections = [
        DependencySection::Dependencies,
        DependencySection::DevDependencies,
        DependencySection::DependencyOverrides,
    ]
    .into_iter()
    .any(|section| mapping_field(value, section.as_pubspec_key()).is_some());
    let sections = [
        DependencySection::Dependencies,
        DependencySection::DevDependencies,
        DependencySection::DependencyOverrides,
    ];
    let mut dependencies =
        declared_dependencies_for_sections(value, source, source_path, &sections);

    if dependencies.is_empty() && !has_declared_sections {
        dependencies = declared_dependencies_from_source(source, source_path);
    }

    dependencies
}

fn declared_dependencies_for_sections(
    value: &Value,
    source: &str,
    source_path: &Path,
    sections: &[DependencySection],
) -> Vec<DeclaredDependency> {
    sections
        .iter()
        .copied()
        .filter_map(|section| {
            mapping_field(value, section.as_pubspec_key()).map(|mapping| (section, mapping))
        })
        .flat_map(|(section, mapping)| {
            mapping.iter().filter_map(move |(name, value)| {
                let name = name.as_str()?.to_owned();
                if section == DependencySection::DependencyOverrides
                    && (!valid_dart_package_name(&name) || value.is_null())
                {
                    return None;
                }
                let location = dependency_location(source, section, &name);
                Some(DeclaredDependency {
                    safe_to_delete: pubspec_entry::is_simple_scalar_dependency(
                        source, section, &name, location,
                    ),
                    location,
                    name,
                    source_path: source_path.to_path_buf(),
                    section,
                })
            })
        })
        .collect()
}

fn owning_package<'package>(
    packages: &'package [PubPackage],
    path: &Path,
) -> Option<&'package PubPackage> {
    packages
        .iter()
        .filter(|package| path.starts_with(&package.root))
        .max_by_key(|package| package.root.components().count())
}

fn package_name(specifier: &str) -> Option<String> {
    specifier
        .strip_prefix("package:")
        .and_then(|rest| rest.split_once('/'))
        .map(|(package, _)| package.to_owned())
}

pub(crate) fn valid_dart_package_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_lowercase() || first == '_')
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

fn mapping_field<'value>(value: &'value Value, field: &str) -> Option<&'value Mapping> {
    value
        .as_mapping()?
        .get(Value::String(field.to_owned()))?
        .as_mapping()
}

fn string_field<'value>(value: &'value Value, field: &str) -> Option<&'value str> {
    value
        .as_mapping()?
        .get(Value::String(field.to_owned()))?
        .as_str()
}

fn has_top_level_field(value: &Value, field: &str) -> bool {
    value
        .as_mapping()
        .is_some_and(|mapping| mapping.contains_key(Value::String(field.to_owned())))
}

#[cfg(test)]
mod tests;
