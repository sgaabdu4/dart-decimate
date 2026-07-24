use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::ScannedProject;
use crate::graph::normalize_against;

use super::{
    IndexedDeclaration, IndexedMember, IndexedReference, SymbolIndex,
    extend_generated_provider_owner_references, extend_implicit_extension_references,
    library_by_path, public_exported_declarations,
};

impl SymbolIndex {
    /// Build a symbol index from parsed Dart files.
    #[must_use]
    pub fn from_project(project: &ScannedProject) -> Self {
        let mut declarations = Vec::new();
        let mut members = Vec::new();
        let mut references_by_name = BTreeMap::<String, Vec<IndexedReference>>::new();
        let mut qualified_references_by_name =
            BTreeMap::<(String, String), Vec<IndexedReference>>::new();
        let library_by_path = library_by_path(project);
        let public_exported_declarations = public_exported_declarations(project);
        let public_exports = public_exported_declarations
            .iter()
            .map(|exported| (exported.path.clone(), exported.declaration.name.clone()))
            .collect();

        for file in &project.files {
            let path = normalize_against(&project.root, &file.path);
            declarations.extend(file.declarations.iter().cloned().map(|declaration| {
                IndexedDeclaration {
                    path: path.clone(),
                    declaration,
                }
            }));
            members.extend(file.members.iter().cloned().map(|member| IndexedMember {
                path: path.clone(),
                member,
            }));
            for reference in &file.references {
                let indexed = IndexedReference { path: path.clone() };
                references_by_name
                    .entry(reference.name.clone())
                    .or_default()
                    .push(indexed.clone());
                if let Some(qualifier) = &reference.qualifier {
                    qualified_references_by_name
                        .entry((qualifier.clone(), reference.name.clone()))
                        .or_default()
                        .push(indexed);
                }
            }
        }
        extend_generated_provider_owner_references(&mut references_by_name, &declarations);
        extend_implicit_extension_references(
            &mut references_by_name,
            &declarations,
            &members,
            &library_by_path,
            &public_exported_declarations,
            project,
        );

        Self {
            declarations,
            members,
            references_by_name,
            qualified_references_by_name,
            library_by_path,
            public_exports,
            public_exported_declarations,
        }
    }

    pub(super) fn qualified_reference_count(
        &self,
        owner: &str,
        name: &str,
        reachable_files: &BTreeSet<PathBuf>,
    ) -> usize {
        self.qualified_references_by_name
            .get(&(owner.to_owned(), name.to_owned()))
            .map_or(0, |references| {
                references
                    .iter()
                    .filter(|reference| reachable_files.contains(&reference.path))
                    .count()
            })
    }

    pub(super) fn reference_count(&self, name: &str, reachable_files: &BTreeSet<PathBuf>) -> usize {
        self.references_by_name.get(name).map_or(0, |references| {
            references
                .iter()
                .filter(|reference| reachable_files.contains(&reference.path))
                .count()
        })
    }

    pub(super) fn library_reference_count(
        &self,
        name: &str,
        library: &Path,
        reachable_files: &BTreeSet<PathBuf>,
    ) -> usize {
        self.references_by_name.get(name).map_or(0, |references| {
            references
                .iter()
                .filter(|reference| reachable_files.contains(&reference.path))
                .filter(|reference| self.library_path(&reference.path) == library)
                .count()
        })
    }

    pub(super) fn library_path<'a>(&'a self, path: &'a Path) -> &'a Path {
        self.library_by_path
            .get(path)
            .map_or(path, std::path::PathBuf::as_path)
    }

    pub(super) fn is_public_export(&self, path: &Path, name: &str) -> bool {
        self.public_exports
            .contains(&(path.to_path_buf(), name.to_owned()))
    }
}
