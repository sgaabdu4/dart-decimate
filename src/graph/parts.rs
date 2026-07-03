use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};

use crate::package_map::PackageMap;
use crate::{DartFile, Location};

use super::{
    DependencyEdge, DependencyGraph, DependencyKind, DependencyVisibility, UnresolvedDependency,
    normalize_against, resolve_local_uri,
};

/// A resolved `part` edge whose target file does not point back to its library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvalidPartRelationship {
    /// Library file that should own this part, when known.
    pub library_path: Option<PathBuf>,
    /// Part file resolved from the `part` directive.
    pub part_path: PathBuf,
    /// Raw URI from the library's `part` directive.
    pub specifier: String,
    /// Location of the invalid or missing `part of` directive.
    pub location: Location,
    /// Specific consistency failure.
    pub reason: InvalidPartReason,
}

/// Reason a resolved Dart `part` relationship is invalid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InvalidPartReason {
    /// The resolved part file has no `part of` directive.
    MissingPartOf,
    /// The `part of` directive has no library name or URI.
    EmptyPartOf,
    /// The file has a `part of` directive but no parsed library has a matching `part`.
    OrphanPartOf {
        /// Library name from the part file, if the directive used a name.
        actual_name: Option<String>,
        /// Library URI from the part file, if the directive used a URI.
        actual_specifier: Option<String>,
    },
    /// Another library already owns this part file.
    DuplicatePartOwner {
        /// Library path that already has a valid `part` edge to this part file.
        existing_library_path: PathBuf,
    },
    /// The `part of` URI could not be resolved as a local Dart file.
    PartOfUriUnresolved {
        /// Raw URI from the part file's `part of` directive.
        actual_specifier: String,
    },
    /// The `part of` URI resolves to a different library file.
    PartOfUriMismatch {
        /// Path the `part` edge expects as the owning library.
        expected_path: PathBuf,
        /// Path resolved from the part file's `part of` URI.
        actual_path: PathBuf,
        /// Raw URI from the part file's `part of` directive.
        actual_specifier: String,
    },
    /// The `part of` library name does not match the source file's library name.
    PartOfNameMismatch {
        /// Library name declared by the source file, if any.
        expected_name: Option<String>,
        /// Library name declared by the part file.
        actual_name: String,
    },
}

#[expect(
    clippy::too_many_arguments,
    reason = "part edges need graph insertion plus reciprocal Dart library validation"
)]
pub(super) fn add_part_dependency(
    root: &Path,
    packages: &PackageMap,
    known_paths: &BTreeSet<PathBuf>,
    nodes_by_path: &BTreeMap<PathBuf, NodeIndex>,
    files_by_path: &BTreeMap<PathBuf, &DartFile>,
    graph: &mut DependencyGraph,
    unresolved: &mut Vec<UnresolvedDependency>,
    invalid_part_relationships: &mut Vec<InvalidPartRelationship>,
    referenced_part_paths: &mut BTreeSet<PathBuf>,
    part_owners: &mut BTreeMap<PathBuf, PathBuf>,
    from_index: NodeIndex,
    from_path: &Path,
    library: &DartFile,
    specifier: &str,
    location: Location,
) {
    let Some(target) = resolve_local_uri(root, packages, from_path, specifier) else {
        return;
    };
    let target_path = target.path;

    if !known_paths.contains(&target_path) {
        if target.local {
            unresolved.push(UnresolvedDependency {
                from_path: from_path.to_path_buf(),
                specifier: specifier.to_owned(),
                kind: DependencyKind::Part,
                location,
                attempted_path: target_path,
                visibility: DependencyVisibility::default(),
            });
        }
        return;
    }

    referenced_part_paths.insert(target_path.clone());
    if let Some(existing_library_path) = part_owners.get(&target_path) {
        if existing_library_path == from_path {
            return;
        }
        invalid_part_relationships.push(InvalidPartRelationship {
            library_path: Some(from_path.to_path_buf()),
            part_path: target_path.clone(),
            specifier: specifier.to_owned(),
            location: files_by_path
                .get(&target_path)
                .and_then(|part| part.part_of.as_ref().map(|part_of| part_of.location))
                .unwrap_or(location),
            reason: InvalidPartReason::DuplicatePartOwner {
                existing_library_path: existing_library_path.clone(),
            },
        });
        return;
    }
    if let Some(part) = files_by_path.get(&target_path).copied() {
        if let Some(relationship) =
            invalid_part_relationship(root, packages, library, part, specifier)
        {
            invalid_part_relationships.push(relationship);
            return;
        }
    }
    part_owners.insert(target_path.clone(), from_path.to_path_buf());

    let Some(target_index) = nodes_by_path.get(&target_path).copied() else {
        return;
    };

    graph.add_edge(
        from_index,
        target_index,
        DependencyEdge {
            specifier: specifier.to_owned(),
            kind: DependencyKind::Part,
            location,
            visibility: DependencyVisibility::default(),
        },
    );
}

pub(super) fn add_orphan_part_relationships(
    root: &Path,
    packages: &PackageMap,
    files_by_path: &BTreeMap<PathBuf, &DartFile>,
    referenced_part_paths: &BTreeSet<PathBuf>,
    invalid_part_relationships: &mut Vec<InvalidPartRelationship>,
) {
    for (part_path, file) in files_by_path {
        let Some(part_of) = &file.part_of else {
            continue;
        };
        if referenced_part_paths.contains(part_path) {
            continue;
        }
        let library_path = part_of.uri.as_ref().and_then(|uri| {
            resolve_local_uri(root, packages, part_path, uri)
                .filter(|target| target.local)
                .map(|target| target.path)
        });
        invalid_part_relationships.push(InvalidPartRelationship {
            library_path,
            part_path: part_path.clone(),
            specifier: part_of
                .uri
                .clone()
                .or_else(|| part_of.name.clone())
                .unwrap_or_default(),
            location: part_of.location,
            reason: InvalidPartReason::OrphanPartOf {
                actual_name: part_of.name.clone(),
                actual_specifier: part_of.uri.clone(),
            },
        });
    }
}

fn invalid_part_relationship(
    root: &Path,
    packages: &PackageMap,
    library: &DartFile,
    part: &DartFile,
    specifier: &str,
) -> Option<InvalidPartRelationship> {
    let library_path = normalize_against(root, &library.path);
    let part_path = normalize_against(root, &part.path);
    let Some(part_of) = &part.part_of else {
        return Some(InvalidPartRelationship {
            library_path: Some(library_path),
            part_path,
            specifier: specifier.to_owned(),
            location: Location { line: 1, column: 0 },
            reason: InvalidPartReason::MissingPartOf,
        });
    };

    let reason = if let Some(uri) = &part_of.uri {
        match resolve_local_uri(root, packages, &part_path, uri) {
            Some(target) if target.path == library_path => return None,
            Some(target) => InvalidPartReason::PartOfUriMismatch {
                expected_path: library_path.clone(),
                actual_path: target.path,
                actual_specifier: uri.clone(),
            },
            None => InvalidPartReason::PartOfUriUnresolved {
                actual_specifier: uri.clone(),
            },
        }
    } else if let Some(name) = &part_of.name {
        let expected_name = library
            .library
            .as_ref()
            .and_then(|library| library.name.clone());
        if expected_name.as_deref() == Some(name.as_str()) {
            return None;
        }
        InvalidPartReason::PartOfNameMismatch {
            expected_name,
            actual_name: name.clone(),
        }
    } else {
        InvalidPartReason::EmptyPartOf
    };

    Some(InvalidPartRelationship {
        library_path: Some(library_path),
        part_path,
        specifier: specifier.to_owned(),
        location: part_of.location,
        reason,
    })
}
