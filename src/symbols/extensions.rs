use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::{DartCombinatorKind, DeclarationKind, DependencyKind, ScannedProject};

use super::{IndexedDeclaration, IndexedMember, IndexedReference, PublicExportedDeclaration};

pub(super) fn extend_implicit_extension_references(
    references_by_name: &mut BTreeMap<String, Vec<IndexedReference>>,
    declarations: &[IndexedDeclaration],
    members: &[IndexedMember],
    library_by_path: &BTreeMap<PathBuf, PathBuf>,
    public_exports: &[PublicExportedDeclaration],
    project: &ScannedProject,
) {
    let dependencies = project.graph.dependencies();
    for declaration in declarations
        .iter()
        .filter(|declaration| declaration.declaration.kind == DeclarationKind::Extension)
    {
        let entry_paths = public_exports
            .iter()
            .filter(|exported| {
                exported.path == declaration.path
                    && exported.declaration.name == declaration.declaration.name
            })
            .map(|exported| exported.entry_path.clone())
            .chain(std::iter::once(declaration.path.clone()))
            .collect::<BTreeSet<_>>();
        let implicit_references = members
            .iter()
            .filter(|member| {
                member.path == declaration.path
                    && member.member.owner == declaration.declaration.name
            })
            .filter_map(|member| references_by_name.get(&member.member.name))
            .flatten()
            .filter(|reference| {
                extension_visible_from(
                    reference,
                    declaration,
                    &entry_paths,
                    library_by_path,
                    &dependencies,
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        if implicit_references.is_empty() {
            continue;
        }

        references_by_name
            .entry(declaration.declaration.name.clone())
            .or_default()
            .extend(implicit_references);
    }
}

fn extension_visible_from(
    reference: &IndexedReference,
    declaration: &IndexedDeclaration,
    entry_paths: &BTreeSet<PathBuf>,
    library_by_path: &BTreeMap<PathBuf, PathBuf>,
    dependencies: &[crate::ResolvedDependency],
) -> bool {
    let reference_library = library_by_path
        .get(&reference.path)
        .unwrap_or(&reference.path);
    let declaration_library = library_by_path
        .get(&declaration.path)
        .unwrap_or(&declaration.path);
    if reference_library == declaration_library {
        return true;
    }

    dependencies.iter().any(|dependency| {
        dependency.kind == DependencyKind::Import
            && dependency.from_path == reference.path
            && entry_paths.contains(&dependency.to_path)
            && dependency.visibility.prefix.is_none()
            && dependency
                .visibility
                .combinators
                .iter()
                .all(|combinator| match combinator.kind {
                    DartCombinatorKind::Show => {
                        combinator.names.contains(&declaration.declaration.name)
                    }
                    DartCombinatorKind::Hide => {
                        !combinator.names.contains(&declaration.declaration.name)
                    }
                })
    })
}
