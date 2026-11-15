use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::generated::is_generated_dart_path;
use crate::{DartCombinator, DeadCodeReport, DependencyKind, ScannedProject};

use super::path_filters::{is_library_source, is_private};
use super::{DuplicateExport, DuplicateExportDeclaration, SymbolIndex, is_visible_through_export};

type Exposure = BTreeMap<String, BTreeSet<PathBuf>>;

struct Directive {
    exports: bool,
    prefix: Option<String>,
    targets: BTreeSet<PathBuf>,
    combinators: Vec<DartCombinator>,
}

struct LibraryGraph {
    declared: BTreeMap<PathBuf, BTreeMap<String, DuplicateExportDeclaration>>,
    directives: BTreeMap<PathBuf, Vec<Directive>>,
}

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
        .map(|duplicate| {
            (
                duplicate.name.as_str(),
                duplicate
                    .declarations
                    .iter()
                    .map(|declaration| &declaration.path)
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeSet<_>>();
    let graph = LibraryGraph {
        declared: declared_names(project, &reachable, &dead, index),
        directives: directives_by_library(project, index),
    };

    let mut memo = BTreeMap::new();
    let mut seen = BTreeMap::<(String, BTreeSet<PathBuf>), PathBuf>::new();
    for (library, directives) in &graph.directives {
        if dead.contains(library) || is_generated_dart_path(library) {
            continue;
        }
        let mut namespaces =
            BTreeMap::<(bool, Option<&str>), BTreeMap<String, Vec<BTreeSet<PathBuf>>>>::new();
        for directive in directives {
            let names = namespaces
                .entry((directive.exports, directive.prefix.as_deref()))
                .or_default();
            for (name, libraries) in directive_supply(directive, &graph, &mut memo) {
                names.entry(name).or_default().push(libraries);
            }
        }
        for (name, suppliers) in namespaces.into_values().flatten() {
            let declaring = suppliers.iter().flatten().cloned().collect::<BTreeSet<_>>();
            if suppliers.len() > 1 && declaring.len() > 1 {
                seen.entry((name, declaring))
                    .or_insert_with(|| library.clone());
            }
        }
    }

    seen.into_iter()
        .filter_map(|((name, libraries), entry_path)| {
            let mut declarations = libraries
                .iter()
                .filter_map(|library| graph.declared.get(library)?.get(&name))
                .cloned()
                .collect::<Vec<_>>();
            declarations.sort_by(|left, right| left.path.cmp(&right.path));
            let already_reported = {
                let paths = declarations
                    .iter()
                    .map(|declaration| &declaration.path)
                    .collect::<BTreeSet<_>>();
                reported.contains(&(name.as_str(), paths))
            };
            (declarations.len() > 1 && !already_reported).then_some(DuplicateExport {
                entry_path,
                name,
                declarations,
                safe_to_delete: false,
            })
        })
        .collect()
}

fn declared_names(
    project: &ScannedProject,
    reachable: &BTreeSet<&PathBuf>,
    dead: &BTreeSet<&PathBuf>,
    index: &SymbolIndex,
) -> BTreeMap<PathBuf, BTreeMap<String, DuplicateExportDeclaration>> {
    let mut declared = BTreeMap::<PathBuf, BTreeMap<String, DuplicateExportDeclaration>>::new();
    for indexed in &index.declarations {
        let name = indexed.declaration.name.as_str();
        if name.is_empty()
            || name == "main"
            || is_private(name)
            || !reachable.contains(&indexed.path)
            || dead.contains(&indexed.path)
            || is_generated_dart_path(&indexed.path)
            || !is_library_source(&project.root, &indexed.path)
        {
            continue;
        }
        declared
            .entry(index.library_path(&indexed.path).to_path_buf())
            .or_default()
            .entry(name.to_owned())
            .or_insert_with(|| DuplicateExportDeclaration {
                path: indexed.path.clone(),
                kind: indexed.declaration.kind,
                location: indexed.declaration.location,
            });
    }
    declared
}

fn directives_by_library(
    project: &ScannedProject,
    index: &SymbolIndex,
) -> BTreeMap<PathBuf, Vec<Directive>> {
    let mut grouped = BTreeMap::<(PathBuf, u8, usize, usize), Directive>::new();
    for edge in project.graph.dependencies() {
        if !matches!(edge.kind, DependencyKind::Import | DependencyKind::Export) {
            continue;
        }
        grouped
            .entry((
                edge.from_path.clone(),
                edge.kind as u8,
                edge.location.line,
                edge.location.column,
            ))
            .or_insert_with(|| Directive {
                exports: edge.kind == DependencyKind::Export,
                prefix: edge.visibility.prefix.clone(),
                targets: BTreeSet::new(),
                combinators: edge.visibility.combinators.clone(),
            })
            .targets
            .insert(index.library_path(&edge.to_path).to_path_buf());
    }
    let mut by_library = BTreeMap::<PathBuf, Vec<Directive>>::new();
    for ((from_path, ..), directive) in grouped {
        by_library
            .entry(index.library_path(&from_path).to_path_buf())
            .or_default()
            .push(directive);
    }
    by_library
}

fn directive_supply(
    directive: &Directive,
    graph: &LibraryGraph,
    memo: &mut BTreeMap<PathBuf, Exposure>,
) -> Exposure {
    directive_supply_guarded(directive, graph, memo, &mut BTreeSet::new())
}

fn directive_supply_guarded(
    directive: &Directive,
    graph: &LibraryGraph,
    memo: &mut BTreeMap<PathBuf, Exposure>,
    active: &mut BTreeSet<PathBuf>,
) -> Exposure {
    let mut supply = Exposure::new();
    for target in &directive.targets {
        for (name, libraries) in exposure(target, graph, memo, active) {
            if is_visible_through_export(&name, &directive.combinators) {
                supply.entry(name).or_default().extend(libraries);
            }
        }
    }
    supply
}

fn exposure(
    library: &Path,
    graph: &LibraryGraph,
    memo: &mut BTreeMap<PathBuf, Exposure>,
    active: &mut BTreeSet<PathBuf>,
) -> Exposure {
    if let Some(done) = memo.get(library) {
        return done.clone();
    }
    if !active.insert(library.to_path_buf()) {
        return Exposure::new();
    }
    let mut result = Exposure::new();
    for name in graph
        .declared
        .get(library)
        .into_iter()
        .flat_map(BTreeMap::keys)
    {
        result
            .entry(name.clone())
            .or_default()
            .insert(library.to_path_buf());
    }
    for directive in graph
        .directives
        .get(library)
        .into_iter()
        .flatten()
        .filter(|directive| directive.exports)
    {
        for (name, libraries) in directive_supply_guarded(directive, graph, memo, active) {
            result.entry(name).or_default().extend(libraries);
        }
    }
    active.remove(library);
    memo.insert(library.to_path_buf(), result.clone());
    result
}
