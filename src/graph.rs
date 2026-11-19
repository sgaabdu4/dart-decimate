use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::package_map::PackageMap;
use crate::{DartCombinator, DartFile, DartUriCondition, Location};

mod conditionals;
mod parts;

use conditionals::{selected_exports, selected_imports};
use parts::add_orphan_part_relationships;
pub use parts::{InvalidPartReason, InvalidPartRelationship};

/// A directed dependency graph where nodes are Dart files.
pub type DependencyGraph = DiGraph<ModuleNode, DependencyEdge>;

/// A Dart file node in the module graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleNode {
    /// Normalized filesystem path for the Dart file.
    pub path: PathBuf,
}

/// A directed Dart library relationship edge in the module graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyEdge {
    /// Raw import/export/part/augment URI from the source file.
    pub specifier: String,
    /// Whether the dependency came from an import, export, part, or augment directive.
    pub kind: DependencyKind,
    /// Location of the directive in the source file.
    pub location: Location,
    /// Import/export visibility metadata carried by this edge.
    pub visibility: DependencyVisibility,
}

/// Dart import/export visibility metadata attached to a dependency edge.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyVisibility {
    /// Optional import prefix after `as`.
    pub prefix: Option<String>,
    /// Whether the import uses `deferred as`.
    pub deferred: bool,
    /// Conditional URI guard that selected this edge, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<DartUriCondition>,
    /// `show` and `hide` combinators applied to the import or export.
    pub combinators: Vec<DartCombinator>,
}

/// Options controlling dependency graph construction.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphOptions {
    /// Conditional URI environment values. Empty means include every branch.
    pub conditional_environment: BTreeMap<String, String>,
}

/// Import/export/part/augment dependency category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DependencyKind {
    /// An `import` directive.
    Import,
    /// An `export` directive.
    Export,
    /// A `part` directive.
    Part,
    /// A `library augment` directive.
    Augment,
}

/// A resolved graph edge with source and target paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedDependency {
    /// File containing the directive.
    pub from_path: PathBuf,
    /// File resolved from the directive.
    pub to_path: PathBuf,
    /// Raw import/export/part/augment URI from the source file.
    pub specifier: String,
    /// Whether the dependency came from an import, export, part, or augment directive.
    pub kind: DependencyKind,
    /// Location of the directive in the source file.
    pub location: Location,
    /// Import/export visibility metadata carried by this edge.
    pub visibility: DependencyVisibility,
}

/// A local dependency URI that did not match a parsed Dart file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnresolvedDependency {
    /// File containing the directive.
    pub from_path: PathBuf,
    /// Raw import/export/part/augment URI from the source file.
    pub specifier: String,
    /// Whether the dependency came from an import, export, part, or augment directive.
    pub kind: DependencyKind,
    /// Location of the directive in the source file.
    pub location: Location,
    /// Local filesystem path Dart Decimate tried to resolve.
    pub attempted_path: PathBuf,
    /// Import/export visibility metadata carried by this unresolved edge.
    pub visibility: DependencyVisibility,
}

/// A Phase 2 module graph plus path lookup indexes.
#[derive(Debug, Clone)]
pub struct ModuleGraph {
    root: PathBuf,
    graph: DependencyGraph,
    nodes_by_path: BTreeMap<PathBuf, NodeIndex>,
    unresolved: Vec<UnresolvedDependency>,
    invalid_part_relationships: Vec<InvalidPartRelationship>,
    packages: PackageMap,
}

impl ModuleGraph {
    pub(crate) fn resolve_specifier(
        &self,
        from: &Path,
        specifier: &str,
    ) -> Option<(PathBuf, bool)> {
        resolve_local_uri(&self.root, &self.packages, from, specifier)
            .map(|target| (target.path, target.local))
    }

    /// Project root used to resolve package and relative imports.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Underlying petgraph directed graph.
    #[must_use]
    pub fn graph(&self) -> &DependencyGraph {
        &self.graph
    }

    /// Number of Dart file nodes.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Number of resolved dependency edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Local dependencies that resolved to paths absent from the parsed input.
    #[must_use]
    pub fn unresolved(&self) -> &[UnresolvedDependency] {
        &self.unresolved
    }

    pub(crate) fn suppress_existing_unresolved_targets(&mut self) {
        self.unresolved
            .retain(|dependency| !dependency.attempted_path.is_file());
    }

    /// Resolved `part` edges whose target file does not declare a matching `part of`.
    #[must_use]
    pub fn invalid_part_relationships(&self) -> &[InvalidPartRelationship] {
        &self.invalid_part_relationships
    }

    /// Local package names discovered from Pub package resolution metadata.
    #[must_use]
    pub fn package_names(&self) -> Vec<&str> {
        self.packages.names()
    }

    /// Lookup a node by normalized path.
    #[must_use]
    pub fn node_index(&self, path: impl AsRef<Path>) -> Option<NodeIndex> {
        let path = normalize_against(&self.root, path.as_ref());
        self.nodes_by_path.get(&path).copied()
    }

    /// Return all resolved dependencies in graph edge order.
    #[must_use]
    pub fn dependencies(&self) -> Vec<ResolvedDependency> {
        self.graph
            .edge_references()
            .map(|edge| {
                let from = &self.graph[edge.source()].path;
                let to = &self.graph[edge.target()].path;
                let weight = edge.weight();
                ResolvedDependency {
                    from_path: from.clone(),
                    to_path: to.clone(),
                    specifier: weight.specifier.clone(),
                    kind: weight.kind,
                    location: weight.location,
                    visibility: weight.visibility.clone(),
                }
            })
            .collect()
    }
}

/// Errors produced while building a module graph.
#[derive(Debug, Error)]
pub enum GraphError {
    /// A directory could not be read during pubspec discovery.
    #[error("failed to read directory {path}: {source}")]
    ReadDir {
        /// Directory path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A directory entry could not be read during pubspec discovery.
    #[error("failed to read directory entry under {path}: {source}")]
    ReadDirEntry {
        /// Directory being scanned.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A file type could not be read during pubspec discovery.
    #[error("failed to read file type for {path}: {source}")]
    FileType {
        /// Entry path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A discovered `pubspec.yaml` could not be read.
    #[error("failed to read pubspec {path}: {source}")]
    ReadPubspec {
        /// Pubspec path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A discovered `pubspec.yaml` could not be parsed.
    #[error("failed to parse pubspec {path}: {source}")]
    ParsePubspec {
        /// Pubspec path.
        path: PathBuf,
        /// Underlying YAML parse error.
        source: Box<noyalib::Error>,
    },
    /// A discovered `.dart_tool/package_config.json` could not be read.
    #[error("failed to read package config {path}: {source}")]
    ReadPackageConfig {
        /// Package config path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A discovered `.dart_tool/package_config.json` could not be parsed.
    #[error("failed to parse package config {path}: {source}")]
    ParsePackageConfig {
        /// Package config path.
        path: PathBuf,
        /// Underlying JSON parse error.
        source: serde_json::Error,
    },
    /// A pub workspace glob pattern was invalid.
    #[error("invalid pub workspace glob pattern {pattern}: {source}")]
    WorkspacePattern {
        /// Invalid pattern.
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
}

/// # Errors
///
/// Returns [`GraphError`] if Pub package metadata cannot be read or parsed, or a workspace glob is invalid.
pub fn build_module_graph(
    root: impl AsRef<Path>,
    files: &[DartFile],
) -> Result<ModuleGraph, GraphError> {
    build_module_graph_with_options(root, files, &GraphOptions::default())
}

/// # Errors
///
/// Returns [`GraphError`] under the same conditions as [`build_module_graph`].
pub fn build_module_graph_with_options(
    root: impl AsRef<Path>,
    files: &[DartFile],
    options: &GraphOptions,
) -> Result<ModuleGraph, GraphError> {
    let root = normalize_path(root.as_ref());
    let packages = PackageMap::discover(&root)?;
    let mut graph = DependencyGraph::new();
    let mut nodes_by_path = BTreeMap::new();

    for file in files {
        let path = normalize_against(&root, &file.path);
        let node = graph.add_node(ModuleNode { path: path.clone() });
        nodes_by_path.insert(path, node);
    }

    let known_paths = nodes_by_path.keys().cloned().collect::<BTreeSet<_>>();
    let files_by_path = files
        .iter()
        .map(|file| (normalize_against(&root, &file.path), file))
        .collect::<BTreeMap<_, _>>();
    let mut builder = EdgeBuilder {
        root: &root,
        packages: &packages,
        known_paths: &known_paths,
        nodes_by_path: &nodes_by_path,
        files_by_path: &files_by_path,
        graph,
        unresolved: Vec::new(),
        invalid_part_relationships: Vec::new(),
        referenced_part_paths: BTreeSet::new(),
        part_owners: BTreeMap::new(),
    };

    for file in files {
        builder.add_file_dependencies(options, file);
    }

    let EdgeBuilder {
        graph,
        unresolved,
        mut invalid_part_relationships,
        referenced_part_paths,
        ..
    } = builder;

    add_orphan_part_relationships(
        &root,
        &packages,
        &files_by_path,
        &referenced_part_paths,
        &mut invalid_part_relationships,
    );
    invalid_part_relationships.sort_by(|left, right| {
        (
            &left.part_path,
            &left.library_path,
            left.location.line,
            &left.specifier,
        )
            .cmp(&(
                &right.part_path,
                &right.library_path,
                right.location.line,
                &right.specifier,
            ))
    });

    Ok(ModuleGraph {
        root,
        graph,
        nodes_by_path,
        unresolved,
        invalid_part_relationships,
        packages,
    })
}

struct EdgeBuilder<'a> {
    root: &'a Path,
    packages: &'a PackageMap,
    known_paths: &'a BTreeSet<PathBuf>,
    nodes_by_path: &'a BTreeMap<PathBuf, NodeIndex>,
    files_by_path: &'a BTreeMap<PathBuf, &'a DartFile>,
    graph: DependencyGraph,
    unresolved: Vec<UnresolvedDependency>,
    invalid_part_relationships: Vec<InvalidPartRelationship>,
    referenced_part_paths: BTreeSet<PathBuf>,
    part_owners: BTreeMap<PathBuf, PathBuf>,
}

struct EdgeSource<'a> {
    index: NodeIndex,
    path: &'a Path,
}

impl EdgeBuilder<'_> {
    fn add_file_dependencies(&mut self, options: &GraphOptions, file: &DartFile) {
        let from_path = normalize_against(self.root, &file.path);
        let Some(index) = self.nodes_by_path.get(&from_path).copied() else {
            return;
        };
        let from = EdgeSource {
            index,
            path: &from_path,
        };

        if let Some(library) = &file.library
            && let Some(augment_uri) = &library.augment_uri
        {
            self.add_dependency(
                &from,
                augment_uri,
                DependencyKind::Augment,
                library.location,
                DependencyVisibility::default(),
            );
        }

        let imports = selected_imports(&file.imports, &options.conditional_environment);
        for import in imports {
            self.add_dependency(
                &from,
                &import.uri,
                DependencyKind::Import,
                import.location,
                DependencyVisibility {
                    prefix: import.prefix.clone(),
                    deferred: import.deferred,
                    condition: import.condition.clone(),
                    combinators: import.combinators.clone(),
                },
            );
        }

        let exports = selected_exports(&file.exports, &options.conditional_environment);
        for export in exports {
            self.add_dependency(
                &from,
                &export.uri,
                DependencyKind::Export,
                export.location,
                DependencyVisibility {
                    condition: export.condition.clone(),
                    combinators: export.combinators.clone(),
                    ..DependencyVisibility::default()
                },
            );
        }

        for part in &file.parts {
            self.add_part_dependency(&from, file, &part.uri, part.location);
        }
    }

    fn add_dependency(
        &mut self,
        from: &EdgeSource<'_>,
        specifier: &str,
        kind: DependencyKind,
        location: Location,
        visibility: DependencyVisibility,
    ) {
        let Some(target) = resolve_local_uri(self.root, self.packages, from.path, specifier) else {
            return;
        };
        let target_path = target.path;

        if !self.known_paths.contains(&target_path) {
            if !target.local {
                return;
            }

            self.unresolved.push(UnresolvedDependency {
                from_path: from.path.to_path_buf(),
                specifier: specifier.to_owned(),
                kind,
                location,
                attempted_path: target_path,
                visibility,
            });
            return;
        }

        let Some(target_index) = self.nodes_by_path.get(&target_path).copied() else {
            return;
        };

        self.graph.add_edge(
            from.index,
            target_index,
            DependencyEdge {
                specifier: specifier.to_owned(),
                kind,
                location,
                visibility,
            },
        );
    }
}

pub(super) struct ResolvedTarget {
    pub(super) path: PathBuf,
    pub(super) local: bool,
}

pub(super) fn resolve_local_uri(
    root: &Path,
    packages: &PackageMap,
    from_path: &Path,
    specifier: &str,
) -> Option<ResolvedTarget> {
    if specifier.starts_with("dart:") {
        return None;
    }

    if let Some(rest) = specifier.strip_prefix("package:") {
        let (package, path) = rest.split_once('/')?;
        let path = percent_decode_uri_path(path);
        return packages
            .resolve_from(from_path, package, &path)
            .map(|resolution| ResolvedTarget {
                path: resolution.path,
                local: resolution.local,
            });
    }

    if specifier.contains(':') {
        return None;
    }

    let specifier = percent_decode_uri_path(specifier);
    let base = from_path.parent().unwrap_or(root);
    Some(ResolvedTarget {
        path: normalize_path(&base.join(&specifier)),
        local: true,
    })
}

fn percent_decode_uri_path(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%'
            && let (Some(high), Some(low)) = (bytes.get(index + 1), bytes.get(index + 2))
            && let Some(decoded_byte) = decode_hex_pair(*high, *low)
        {
            decoded.push(decoded_byte);
            index += 3;
            continue;
        }

        decoded.push(bytes[index]);
        index += 1;
    }

    String::from_utf8(decoded).unwrap_or_else(|_| value.to_owned())
}

fn decode_hex_pair(high: u8, low: u8) -> Option<u8> {
    Some(hex_value(high)? * 16 + hex_value(low)?)
}

const fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

pub(crate) fn normalize_against(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() || path.starts_with(root) {
        normalize_path(path)
    } else {
        normalize_path(&root.join(path))
    }
}

pub(crate) fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir | Component::Normal(_) => normalized.push(component.as_os_str()),
        }
    }

    if normalized.as_os_str().is_empty() {
        return PathBuf::from(".");
    }

    normalized
}

#[cfg(test)]
mod augment_tests;
#[cfg(test)]
mod package_config_tests;
#[cfg(test)]
mod pubspec_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod uri_tests;
