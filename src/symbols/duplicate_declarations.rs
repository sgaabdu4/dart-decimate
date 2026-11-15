use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::generated::is_generated_dart_path;
use crate::{DeadCodeReport, DependencyKind, ScannedProject};

use super::path_filters::{is_library_source, is_private};
use super::{DuplicateExport, DuplicateExportDeclaration, SymbolIndex};

pub(super) fn duplicate_declarations_across_libraries(
    project: &ScannedProject,
    dead_code: &DeadCodeReport,
    index: &SymbolIndex,
    already_reported: &[DuplicateExport],
) -> Vec<DuplicateExport> {
    let reachable = dead_code
        .reachable_files
        .iter()
        .collect::<BTreeSet<&PathBuf>>();
    let dead = dead_code
        .dead_files
        .iter()
        .map(|dead_file| &dead_file.path)
        .collect::<BTreeSet<&PathBuf>>();
    let reported = already_reported
        .iter()
        .map(|duplicate| duplicate.name.as_str())
        .collect::<BTreeSet<_>>();
    let alternatives = platform_alternatives(project);

    let mut by_name = BTreeMap::<&str, BTreeMap<&Path, DuplicateExportDeclaration>>::new();
    for indexed in &index.declarations {
        let name = indexed.declaration.name.as_str();
        if name.is_empty()
            || name == "main"
            || is_private(name)
            || reported.contains(name)
            || !reachable.contains(&indexed.path)
            || dead.contains(&indexed.path)
            || is_generated_dart_path(&indexed.path)
            || !is_library_source(&project.root, &indexed.path)
        {
            continue;
        }
        by_name
            .entry(name)
            .or_default()
            .entry(index.library_path(&indexed.path))
            .or_insert_with(|| DuplicateExportDeclaration {
                path: indexed.path.clone(),
                kind: indexed.declaration.kind,
                location: indexed.declaration.location,
            });
    }

    by_name
        .into_iter()
        .filter(|(_, libraries)| libraries.len() > 1)
        .filter(|(_, libraries)| {
            !alternatives
                .iter()
                .any(|group| libraries.keys().all(|library| group.contains(*library)))
        })
        .map(|(name, libraries)| {
            let mut declarations = libraries.into_values().collect::<Vec<_>>();
            declarations.sort_by(|left, right| left.path.cmp(&right.path));
            DuplicateExport {
                entry_path: declarations[0].path.clone(),
                name: name.to_owned(),
                declarations,
                safe_to_delete: false,
            }
        })
        .collect()
}

fn platform_alternatives(project: &ScannedProject) -> Vec<BTreeSet<PathBuf>> {
    let mut by_directive = BTreeMap::<_, (BTreeSet<PathBuf>, bool)>::new();
    for edge in project.graph.dependencies() {
        if !matches!(edge.kind, DependencyKind::Import | DependencyKind::Export) {
            continue;
        }
        let entry = by_directive
            .entry((
                edge.from_path.clone(),
                edge.kind as u8,
                edge.location.line,
                edge.location.column,
            ))
            .or_default();
        entry.0.insert(edge.to_path.clone());
        entry.1 |= edge.visibility.condition.is_some();
    }
    by_directive
        .into_values()
        .filter(|(_, conditional)| *conditional)
        .map(|(paths, _)| paths)
        .collect()
}
