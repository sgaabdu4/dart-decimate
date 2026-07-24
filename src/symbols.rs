use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::generated::is_generated_dart_path;
use crate::graph::normalize_against;
use crate::{
    DartCombinatorKind, DeadCodeReport, DeclarationKind, DependencyKind, Location,
    MemberDeclaration, MemberKind, ScannedProject, TopLevelDeclaration,
};

mod extensions;
mod index;
mod path_filters;
mod private_type_leaks;
mod riverpod;
use extensions::extend_implicit_extension_references;
use path_filters::{is_library_source, is_private, is_public_library_entry};
pub use private_type_leaks::PrivateTypeLeak;
use private_type_leaks::private_type_leaks;
use riverpod::extend_generated_provider_owner_references;

/// Symbol-level dead-code result.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolReport {
    /// Public top-level declarations with no reachable syntactic references.
    pub unused_exports: Vec<UnusedExport>,
    /// Unused enum constants and conservative private class-like members.
    pub unused_members: Vec<UnusedMember>,
    /// Public signatures exposing same-library private types.
    pub private_type_leaks: Vec<PrivateTypeLeak>,
    /// Public API entries that expose multiple declarations with the same name.
    pub duplicate_exports: Vec<DuplicateExport>,
}

/// Options for symbol-level dead-code analysis.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolAnalysisOptions {
    /// Report public declarations in entry libraries when they are otherwise unused.
    pub include_entry_exports: bool,
    /// Report exported signatures that expose same-library private types.
    pub private_type_leaks: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnusedExport {
    pub path: PathBuf,
    pub kind: DeclarationKind,
    pub name: String,
    pub location: Location,
    pub reference_count: usize,
    pub safe_to_delete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnusedMember {
    pub path: PathBuf,
    pub owner: String,
    pub kind: MemberKind,
    pub name: String,
    pub location: Location,
    pub reference_count: usize,
    pub safe_to_delete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateExport {
    pub entry_path: PathBuf,
    pub name: String,
    pub declarations: Vec<DuplicateExportDeclaration>,
    pub safe_to_delete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateExportDeclaration {
    pub path: PathBuf,
    pub kind: DeclarationKind,
    pub location: Location,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolIndex {
    declarations: Vec<IndexedDeclaration>,
    members: Vec<IndexedMember>,
    references_by_name: BTreeMap<String, Vec<IndexedReference>>,
    qualified_references_by_name: BTreeMap<(String, String), Vec<IndexedReference>>,
    library_by_path: BTreeMap<PathBuf, PathBuf>,
    public_exports: BTreeSet<(PathBuf, String)>,
    public_exported_declarations: Vec<PublicExportedDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IndexedDeclaration {
    path: PathBuf,
    declaration: TopLevelDeclaration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IndexedMember {
    path: PathBuf,
    member: MemberDeclaration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IndexedReference {
    path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PublicExportedDeclaration {
    entry_path: PathBuf,
    path: PathBuf,
    declaration: TopLevelDeclaration,
}

/// Find the first conservative unused-export symbol findings.
#[must_use]
pub fn analyze_symbols(
    project: &ScannedProject,
    dead_code: Option<&DeadCodeReport>,
) -> SymbolReport {
    analyze_symbols_with_options(project, dead_code, SymbolAnalysisOptions::default())
}

/// Find conservative symbol findings with explicit analysis options.
#[must_use]
pub fn analyze_symbols_with_options(
    project: &ScannedProject,
    dead_code: Option<&DeadCodeReport>,
    options: SymbolAnalysisOptions,
) -> SymbolReport {
    let index = SymbolIndex::from_project(project);
    let duplicate_exports = duplicate_exports_from_public(&index.public_exported_declarations);
    let private_type_leaks = if options.private_type_leaks {
        private_type_leaks(project, &index)
    } else {
        Vec::new()
    };
    let Some(dead_code) = dead_code else {
        return SymbolReport {
            unused_exports: Vec::new(),
            unused_members: Vec::new(),
            private_type_leaks,
            duplicate_exports,
        };
    };

    let unused_exports = unused_exports(project, dead_code, &index, options);
    let unused_members = unused_members(project, dead_code, &index);

    SymbolReport {
        unused_exports,
        unused_members,
        private_type_leaks,
        duplicate_exports,
    }
}

/// Find conservative unused-export and duplicate-export symbol findings.
#[must_use]
pub fn analyze_unused_exports(
    project: &ScannedProject,
    dead_code: &DeadCodeReport,
) -> SymbolReport {
    analyze_symbols_with_options(project, Some(dead_code), SymbolAnalysisOptions::default())
}
fn unused_exports(
    project: &ScannedProject,
    dead_code: &DeadCodeReport,
    index: &SymbolIndex,
    options: SymbolAnalysisOptions,
) -> Vec<UnusedExport> {
    let reachable_files = dead_code
        .reachable_files
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let entry_points = dead_code
        .entry_points
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let dead_files = dead_code
        .dead_files
        .iter()
        .map(|dead_file| dead_file.path.clone())
        .collect::<BTreeSet<_>>();

    let mut unused_exports = index
        .declarations
        .iter()
        .filter_map(|declaration| {
            unused_export_from_declaration(
                project,
                index,
                &reachable_files,
                &entry_points,
                &dead_files,
                declaration,
                options,
            )
        })
        .collect::<Vec<_>>();

    unused_exports.sort_by(|left, right| {
        (
            &left.path,
            left.location.line,
            left.location.column,
            &left.name,
        )
            .cmp(&(
                &right.path,
                right.location.line,
                right.location.column,
                &right.name,
            ))
    });

    unused_exports
}

fn unused_members(
    project: &ScannedProject,
    dead_code: &DeadCodeReport,
    index: &SymbolIndex,
) -> Vec<UnusedMember> {
    let reachable_files = dead_code
        .reachable_files
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let dead_files = dead_code
        .dead_files
        .iter()
        .map(|dead_file| dead_file.path.clone())
        .collect::<BTreeSet<_>>();

    let mut unused_members = index
        .members
        .iter()
        .filter_map(|member| {
            unused_member_from_declaration(project, index, &reachable_files, &dead_files, member)
        })
        .collect::<Vec<_>>();

    unused_members.sort_by(|left, right| {
        (
            &left.path,
            left.location.line,
            left.location.column,
            &left.owner,
            &left.name,
        )
            .cmp(&(
                &right.path,
                right.location.line,
                right.location.column,
                &right.owner,
                &right.name,
            ))
    });
    let mut seen = BTreeSet::new();
    unused_members.retain(|member| {
        seen.insert((
            member.path.clone(),
            member.owner.clone(),
            member.name.clone(),
        ))
    });

    unused_members
}

fn unused_export_from_declaration(
    project: &ScannedProject,
    index: &SymbolIndex,
    reachable_files: &BTreeSet<PathBuf>,
    entry_points: &BTreeSet<PathBuf>,
    dead_files: &BTreeSet<PathBuf>,
    indexed: &IndexedDeclaration,
    options: SymbolAnalysisOptions,
) -> Option<UnusedExport> {
    if !indexed.path.starts_with(&project.root)
        || !reachable_files.contains(&indexed.path)
        || (entry_points.contains(&indexed.path)
            && (!options.include_entry_exports || indexed.declaration.name == "main"))
        || dead_files.contains(&indexed.path)
        || is_private(&indexed.declaration.name)
        || is_generated_dart_path(&indexed.path)
        || !is_library_source(&project.root, &indexed.path)
    {
        return None;
    }

    let reference_count = index.reference_count(&indexed.declaration.name, reachable_files);
    let public_export = index.is_public_export(&indexed.path, &indexed.declaration.name)
        && !(options.include_entry_exports && entry_points.contains(&indexed.path));
    if reference_count != 0 || public_export {
        return None;
    }

    Some(UnusedExport {
        path: indexed.path.clone(),
        kind: indexed.declaration.kind,
        name: indexed.declaration.name.clone(),
        location: indexed.declaration.location,
        reference_count,
        safe_to_delete: is_simple_unused_declaration(index, indexed),
    })
}

fn is_simple_unused_declaration(index: &SymbolIndex, indexed: &IndexedDeclaration) -> bool {
    let range = indexed.declaration.range;
    range.start_line == range.end_line
        && index
            .declarations
            .iter()
            .filter(|candidate| {
                candidate.path == indexed.path
                    && candidate.declaration.location.line == indexed.declaration.location.line
            })
            .take(2)
            .count()
            == 1
}

fn unused_member_from_declaration(
    project: &ScannedProject,
    index: &SymbolIndex,
    reachable_files: &BTreeSet<PathBuf>,
    dead_files: &BTreeSet<PathBuf>,
    indexed: &IndexedMember,
) -> Option<UnusedMember> {
    if !indexed.path.starts_with(&project.root)
        || !reachable_files.contains(&indexed.path)
        || dead_files.contains(&indexed.path)
        || is_generated_dart_path(&indexed.path)
        || !is_library_source(&project.root, &indexed.path)
        || !should_check_member(index, indexed)
    {
        return None;
    }

    let reference_count = if indexed.member.kind == MemberKind::EnumConstant {
        index.qualified_reference_count(
            &indexed.member.owner,
            &indexed.member.name,
            reachable_files,
        ) + index.qualified_reference_count(&indexed.member.owner, "values", reachable_files)
    } else {
        let library = index.library_path(&indexed.path);
        index.library_reference_count(&indexed.member.name, library, reachable_files)
    };
    if reference_count != 0 {
        return None;
    }

    Some(UnusedMember {
        path: indexed.path.clone(),
        owner: indexed.member.owner.clone(),
        kind: indexed.member.kind,
        name: indexed.member.name.clone(),
        location: indexed.member.location,
        reference_count,
        safe_to_delete: false,
    })
}

fn should_check_member(index: &SymbolIndex, indexed: &IndexedMember) -> bool {
    match indexed.member.kind {
        MemberKind::EnumConstant => {
            !is_private(&indexed.member.name)
                && !index.is_public_export(&indexed.path, &indexed.member.owner)
        }
        MemberKind::Field | MemberKind::Getter | MemberKind::Setter | MemberKind::Method => {
            is_private(&indexed.member.name)
        }
        MemberKind::Constructor | MemberKind::Operator => false,
    }
}

fn library_by_path(project: &ScannedProject) -> BTreeMap<PathBuf, PathBuf> {
    let dependencies = project.graph.dependencies();
    let part_edges = dependencies
        .iter()
        .filter(|edge| edge.kind == DependencyKind::Part)
        .cloned()
        .collect::<Vec<_>>();
    let mut libraries = project
        .files
        .iter()
        .map(|file| {
            let path = normalize_against(&project.root, &file.path);
            (path.clone(), path)
        })
        .collect::<BTreeMap<_, _>>();

    for edge in &part_edges {
        for path in library_paths(&part_edges, &edge.from_path) {
            libraries.insert(path, edge.from_path.clone());
        }
    }

    libraries
}

fn public_exported_declarations(project: &ScannedProject) -> Vec<PublicExportedDeclaration> {
    let declarations = declarations_by_path(project);
    let public_entry_files = project
        .files
        .iter()
        .map(|file| normalize_against(&project.root, &file.path))
        .filter(|path| is_public_library_entry(&project.root, path))
        .collect::<BTreeSet<_>>();

    let dependencies = project.graph.dependencies();
    let export_edges = dependencies
        .iter()
        .filter(|edge| edge.kind == DependencyKind::Export)
        .cloned()
        .collect::<Vec<_>>();
    let part_edges = dependencies
        .iter()
        .filter(|edge| edge.kind == DependencyKind::Part)
        .cloned()
        .collect::<Vec<_>>();
    let mut exported = Vec::new();
    for public_entry in public_entry_files {
        extend_library_declarations(
            &declarations,
            &part_edges,
            &public_entry,
            &public_entry,
            None,
            &mut exported,
        );

        collect_public_exports(
            &export_edges,
            &part_edges,
            &declarations,
            &public_entry,
            &public_entry,
            &[],
            &mut exported,
        );
    }

    exported
}

fn collect_public_exports(
    export_edges: &[crate::ResolvedDependency],
    part_edges: &[crate::ResolvedDependency],
    declarations: &BTreeMap<PathBuf, Vec<TopLevelDeclaration>>,
    entry_path: &Path,
    from_path: &Path,
    chain: &[crate::DependencyVisibility],
    exported: &mut Vec<PublicExportedDeclaration>,
) {
    if chain.len() > 8 {
        return;
    }

    for edge in export_edges
        .iter()
        .filter(|edge| edge.from_path == from_path)
    {
        let mut next_chain = chain.to_owned();
        next_chain.push(edge.visibility.clone());

        extend_library_declarations(
            declarations,
            part_edges,
            entry_path,
            &edge.to_path,
            Some(&next_chain),
            exported,
        );

        collect_public_exports(
            export_edges,
            part_edges,
            declarations,
            entry_path,
            &edge.to_path,
            &next_chain,
            exported,
        );
    }
}

fn extend_library_declarations(
    declarations: &BTreeMap<PathBuf, Vec<TopLevelDeclaration>>,
    part_edges: &[crate::ResolvedDependency],
    entry_path: &Path,
    library_path: &Path,
    chain: Option<&[crate::DependencyVisibility]>,
    exported: &mut Vec<PublicExportedDeclaration>,
) {
    for path in library_paths(part_edges, library_path) {
        let Some(declarations) = declarations.get(&path) else {
            continue;
        };
        exported.extend(
            declarations
                .iter()
                .filter(|declaration| {
                    chain.is_none_or(|chain| is_visible_through_chain(&declaration.name, chain))
                })
                .cloned()
                .map(|declaration| PublicExportedDeclaration {
                    entry_path: entry_path.to_path_buf(),
                    path: path.clone(),
                    declaration,
                }),
        );
    }
}

fn library_paths(part_edges: &[crate::ResolvedDependency], library_path: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::from([library_path.to_path_buf()]);
    let mut seen = BTreeSet::from([library_path.to_path_buf()]);
    let mut index = 0;

    while let Some(path) = paths.get(index).cloned() {
        index += 1;
        for edge in part_edges.iter().filter(|edge| edge.from_path == path) {
            if seen.insert(edge.to_path.clone()) {
                paths.push(edge.to_path.clone());
            }
        }
    }

    paths
}

fn duplicate_exports_from_public(
    exported_declarations: &[PublicExportedDeclaration],
) -> Vec<DuplicateExport> {
    let mut grouped =
        BTreeMap::<(PathBuf, String), BTreeMap<PathBuf, DuplicateExportDeclaration>>::new();

    for exported in exported_declarations {
        if is_private(&exported.declaration.name) {
            continue;
        }

        grouped
            .entry((
                exported.entry_path.clone(),
                exported.declaration.name.clone(),
            ))
            .or_default()
            .entry(exported.path.clone())
            .or_insert_with(|| DuplicateExportDeclaration {
                path: exported.path.clone(),
                kind: exported.declaration.kind,
                location: exported.declaration.location,
            });
    }

    let mut duplicates = grouped
        .into_iter()
        .filter_map(|((entry_path, name), declarations_by_path)| {
            if declarations_by_path.len() < 2 {
                return None;
            }

            Some(DuplicateExport {
                entry_path,
                name,
                declarations: declarations_by_path.into_values().collect(),
                safe_to_delete: false,
            })
        })
        .collect::<Vec<_>>();

    duplicates.sort_by(|left, right| {
        (&left.entry_path, &left.name).cmp(&(&right.entry_path, &right.name))
    });
    duplicates
}

fn declarations_by_path(project: &ScannedProject) -> BTreeMap<PathBuf, Vec<TopLevelDeclaration>> {
    project
        .files
        .iter()
        .map(|file| {
            (
                normalize_against(&project.root, &file.path),
                file.declarations.clone(),
            )
        })
        .collect()
}

fn is_visible_through_export(name: &str, combinators: &[crate::DartCombinator]) -> bool {
    let mut visible = true;
    for combinator in combinators {
        match combinator.kind {
            DartCombinatorKind::Show => {
                visible = combinator.names.iter().any(|shown| shown == name);
            }
            DartCombinatorKind::Hide => {
                if combinator.names.iter().any(|hidden| hidden == name) {
                    visible = false;
                }
            }
        }
    }
    visible
}

fn is_visible_through_chain(name: &str, chain: &[crate::DependencyVisibility]) -> bool {
    chain
        .iter()
        .all(|visibility| is_visible_through_export(name, &visibility.combinators))
}

#[cfg(test)]
mod tests;
