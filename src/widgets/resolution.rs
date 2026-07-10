use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::graph::normalize_against;
use crate::{DartCombinator, DartCombinatorKind, DependencyKind, DependencyVisibility};
use crate::{ResolvedDependency, ScannedProject};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ClassKey {
    pub(super) path: PathBuf,
    pub(super) name: String,
}

#[derive(Debug)]
pub(super) struct DeclarationResolver {
    declarations: BTreeMap<PathBuf, BTreeSet<String>>,
    imports: Vec<ResolvedDependency>,
    exports: Vec<ResolvedDependency>,
    library_by_path: BTreeMap<PathBuf, PathBuf>,
    paths_by_library: BTreeMap<PathBuf, BTreeSet<PathBuf>>,
}

impl DeclarationResolver {
    pub(super) fn new(
        project: &ScannedProject,
        declarations: impl IntoIterator<Item = ClassKey>,
    ) -> Self {
        let declarations = declarations.into_iter().fold(
            BTreeMap::<PathBuf, BTreeSet<String>>::new(),
            |mut declarations, class| {
                declarations
                    .entry(class.path)
                    .or_default()
                    .insert(class.name);
                declarations
            },
        );
        let dependencies = project
            .graph
            .dependencies()
            .into_iter()
            .map(|mut dependency| {
                dependency.from_path = normalize_against(&project.root, &dependency.from_path);
                dependency.to_path = normalize_against(&project.root, &dependency.to_path);
                dependency
            })
            .collect::<Vec<_>>();
        let mut library_by_path = declarations
            .keys()
            .map(|path| (path.clone(), path.clone()))
            .collect::<BTreeMap<_, _>>();
        for dependency in dependencies
            .iter()
            .filter(|dependency| dependency.kind == DependencyKind::Part)
        {
            library_by_path.insert(dependency.to_path.clone(), dependency.from_path.clone());
        }
        let paths_by_library = library_by_path.iter().fold(
            BTreeMap::<PathBuf, BTreeSet<PathBuf>>::new(),
            |mut paths, (path, library)| {
                paths
                    .entry(library.clone())
                    .or_default()
                    .insert(path.clone());
                paths
            },
        );
        let imports = dependencies
            .iter()
            .filter(|dependency| dependency.kind == DependencyKind::Import)
            .cloned()
            .collect();
        let exports = dependencies
            .into_iter()
            .filter(|dependency| dependency.kind == DependencyKind::Export)
            .collect();
        Self {
            declarations,
            imports,
            exports,
            library_by_path,
            paths_by_library,
        }
    }

    pub(super) fn resolve(&self, from: &Path, reference: &str) -> Option<ClassKey> {
        let reference = compact_reference(reference);
        let segments = reference.split('.').collect::<Vec<_>>();
        let first = *segments.first()?;
        let library = self.library_path(from);
        let imports = self
            .imports
            .iter()
            .filter(|dependency| dependency.from_path == library)
            .collect::<Vec<_>>();

        if let Some(name) = segments.get(1).filter(|name| type_like_name(name)) {
            let prefixed_imports = imports
                .iter()
                .filter(|dependency| dependency.visibility.prefix.as_deref() == Some(first))
                .collect::<Vec<_>>();
            if !prefixed_imports.is_empty() {
                if name.starts_with('_') {
                    return None;
                }
                let prefixed = prefixed_imports
                    .into_iter()
                    .filter(|dependency| visible(name, &dependency.visibility))
                    .flat_map(|dependency| self.exported_classes(&dependency.to_path, name))
                    .collect::<BTreeSet<_>>();
                return exactly_one(prefixed);
            }
        }

        let local = self.library_classes(&library, first);
        if !local.is_empty() {
            return exactly_one(local);
        }
        if first.starts_with('_') {
            return None;
        }
        let imported = imports
            .into_iter()
            .filter(|dependency| dependency.visibility.prefix.is_none())
            .filter(|dependency| visible(first, &dependency.visibility))
            .flat_map(|dependency| self.exported_classes(&dependency.to_path, first))
            .collect::<BTreeSet<_>>();
        exactly_one(imported)
    }

    fn exported_classes(&self, library: &Path, name: &str) -> BTreeSet<ClassKey> {
        let mut visited = BTreeSet::new();
        self.collect_exported_classes(library, name, &mut visited)
    }

    fn collect_exported_classes(
        &self,
        library: &Path,
        name: &str,
        visited: &mut BTreeSet<PathBuf>,
    ) -> BTreeSet<ClassKey> {
        let library = self.library_path(library);
        if !visited.insert(library.clone()) {
            return BTreeSet::new();
        }
        let mut classes = self.library_classes(&library, name);
        for dependency in self
            .exports
            .iter()
            .filter(|dependency| dependency.from_path == library)
            .filter(|dependency| visible(name, &dependency.visibility))
        {
            classes.extend(self.collect_exported_classes(&dependency.to_path, name, visited));
        }
        classes
    }

    fn library_classes(&self, library: &Path, name: &str) -> BTreeSet<ClassKey> {
        self.paths_by_library
            .get(library)
            .into_iter()
            .flat_map(|paths| paths.iter())
            .filter(|path| {
                self.declarations
                    .get(*path)
                    .is_some_and(|declarations| declarations.contains(name))
            })
            .map(|path| ClassKey {
                path: path.clone(),
                name: name.to_owned(),
            })
            .collect()
    }

    fn library_path(&self, path: &Path) -> PathBuf {
        self.library_by_path
            .get(path)
            .cloned()
            .unwrap_or_else(|| path.to_path_buf())
    }
}

fn compact_reference(reference: &str) -> String {
    reference
        .trim()
        .trim_end_matches('?')
        .split('<')
        .next()
        .unwrap_or(reference)
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn type_like_name(name: &&str) -> bool {
    name.chars()
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_uppercase())
}

fn visible(name: &str, visibility: &DependencyVisibility) -> bool {
    visible_through_combinators(name, &visibility.combinators)
}

fn visible_through_combinators(name: &str, combinators: &[DartCombinator]) -> bool {
    let mut is_visible = true;
    for combinator in combinators {
        match combinator.kind {
            DartCombinatorKind::Show => {
                is_visible = combinator.names.iter().any(|shown| shown == name);
            }
            DartCombinatorKind::Hide => {
                if combinator.names.iter().any(|hidden| hidden == name) {
                    is_visible = false;
                }
            }
        }
    }
    is_visible
}

fn exactly_one(mut classes: BTreeSet<ClassKey>) -> Option<ClassKey> {
    let class = classes.pop_first()?;
    classes.is_empty().then_some(class)
}
