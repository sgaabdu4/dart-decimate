use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::widgets::resolution::{ClassKey, compact_reference, visible_through_combinators};
use crate::{DartFile, DartImport, DeclarationKind, MemberKind};

use super::loader::{Outline, OutlineLoader};
use super::{OutsideSources, Target};

pub(crate) struct OutsideTypes {
    reader: Option<Reader>,
    lookups: BTreeMap<(PathBuf, String), Option<ClassKey>>,
    pending: Vec<ClassKey>,
    seen: BTreeSet<ClassKey>,
}

#[derive(Default)]
pub(crate) struct OutsideClass {
    pub(crate) extended: BTreeSet<ClassKey>,
    pub(crate) mixed: BTreeSet<ClassKey>,
    pub(crate) implemented: BTreeSet<ClassKey>,
    pub(crate) unread_extended: BTreeSet<String>,
    pub(crate) unread_interface: BTreeSet<String>,
    pub(crate) names: BTreeSet<String>,
}

struct Reader {
    sources: OutsideSources,
    loader: OutlineLoader,
    library_of: BTreeMap<PathBuf, PathBuf>,
    exported: BTreeMap<(PathBuf, String), Walk>,
}

#[derive(Clone, Default)]
struct Walk {
    classes: BTreeSet<ClassKey>,
    complete: bool,
}

impl Walk {
    fn empty() -> Self {
        Self {
            classes: BTreeSet::new(),
            complete: true,
        }
    }

    fn unread() -> Self {
        Self {
            classes: BTreeSet::new(),
            complete: false,
        }
    }
}

impl OutsideTypes {
    pub(crate) fn new(root: &Path) -> Self {
        Self {
            reader: OutsideSources::discover(root).map(|sources| Reader {
                sources,
                loader: OutlineLoader::default(),
                library_of: BTreeMap::new(),
                exported: BTreeMap::new(),
            }),
            lookups: BTreeMap::new(),
            pending: Vec::new(),
            seen: BTreeSet::new(),
        }
    }

    pub(crate) fn link(
        &mut self,
        library: &Path,
        imports: &[DartImport],
        reference: &str,
        from_outside: bool,
    ) -> Option<ClassKey> {
        let reference = compact_reference(reference);
        let key = (library.to_path_buf(), reference.clone());
        let found = if let Some(known) = self.lookups.get(&key) {
            known.clone()
        } else {
            let found = self
                .reader
                .as_mut()
                .and_then(|reader| reader.lookup(library, imports, &reference, from_outside));
            self.lookups.insert(key, found.clone());
            found
        };
        if let Some(class) = &found
            && self.seen.insert(class.clone())
        {
            self.pending.push(class.clone());
        }
        found
    }

    pub(crate) fn expand(&mut self) -> BTreeMap<ClassKey, OutsideClass> {
        let mut classes = BTreeMap::new();
        while let Some(key) = self.pending.pop() {
            let class = self.expand_class(&key);
            classes.insert(key, class);
        }
        classes
    }

    fn expand_class(&mut self, key: &ClassKey) -> OutsideClass {
        let Some(reader) = self.reader.as_mut() else {
            return OutsideClass::default();
        };
        let library = reader
            .library_of
            .get(&key.path)
            .cloned()
            .unwrap_or_else(|| key.path.clone());
        let (Outline::Read(file), Outline::Read(library_file)) = (
            reader.loader.load(&key.path, &key.name),
            reader.loader.load(&library, &key.name),
        ) else {
            return OutsideClass {
                unread_extended: BTreeSet::from([key.name.clone()]),
                ..OutsideClass::default()
            };
        };
        let mut class = OutsideClass {
            names: declared_names(&file, &key.name),
            ..OutsideClass::default()
        };
        let Some(clause) = file
            .class_supertypes
            .iter()
            .find(|clause| clause.owner == key.name)
        else {
            return class;
        };
        let lists = [
            (&clause.names, true, false),
            (&clause.constraints, true, false),
            (&clause.mixins, false, true),
            (&clause.implemented, false, false),
        ];
        for (references, extends, mixes) in lists {
            for reference in references {
                if matches!(reference.as_str(), "Object" | "core.Object") {
                    continue;
                }
                let target = self.link(&library, &library_file.imports, reference, true);
                match target {
                    Some(target) if extends => {
                        class.extended.insert(target);
                    }
                    Some(target) if mixes => {
                        class.mixed.insert(target);
                    }
                    Some(target) => {
                        class.implemented.insert(target);
                    }
                    None if extends => {
                        class.unread_extended.insert(reference.clone());
                    }
                    None => {
                        class.unread_interface.insert(reference.clone());
                    }
                }
            }
        }
        class
    }
}

fn declared_names(file: &DartFile, owner: &str) -> BTreeSet<String> {
    file.members
        .iter()
        .filter(|member| {
            member.owner == owner
                && !member.name.starts_with('_')
                && matches!(
                    member.kind,
                    MemberKind::Field
                        | MemberKind::Getter
                        | MemberKind::Setter
                        | MemberKind::Method
                )
        })
        .map(|member| member.name.clone())
        .collect()
}

fn declares(file: &DartFile, name: &str) -> bool {
    file.declarations.iter().any(|declaration| {
        declaration.name == name
            && matches!(
                declaration.kind,
                DeclarationKind::Class
                    | DeclarationKind::Mixin
                    | DeclarationKind::TypeAlias
                    | DeclarationKind::Enum
                    | DeclarationKind::ExtensionType
            )
    })
}

impl Reader {
    fn lookup(
        &mut self,
        library: &Path,
        imports: &[DartImport],
        reference: &str,
        from_outside: bool,
    ) -> Option<ClassKey> {
        let mut segments = reference.split('.');
        let first = segments.next()?;
        let second = segments.next();
        if segments.next().is_some() {
            return None;
        }
        let (prefix, name) = match second {
            Some(name)
                if imports
                    .iter()
                    .any(|import| import.prefix.as_deref() == Some(first)) =>
            {
                (Some(first), name)
            }
            Some(_) => return None,
            None => (None, first),
        };
        if from_outside && prefix.is_none() {
            let own = self.library_classes(library, name);
            if !own.classes.is_empty() {
                return exactly_one(own.classes);
            }
        }
        if name.starts_with('_') {
            return None;
        }

        let selected = imports
            .iter()
            .filter(|import| import.prefix.as_deref() == prefix)
            .filter(|import| visible_through_combinators(name, &import.combinators))
            .collect::<Vec<_>>();
        let (core, others): (Vec<_>, Vec<_>) = selected
            .into_iter()
            .partition(|import| import.uri == "dart:core");
        let explicit_core = imports.iter().any(|import| import.uri == "dart:core");

        let (found, incomplete) = self.directive_classes(library, &others, name, from_outside);
        if !found.is_empty() {
            return exactly_one(found);
        }
        if incomplete {
            return None;
        }
        let (found, _) = if explicit_core || prefix.is_some() {
            self.directive_classes(library, &core, name, from_outside)
        } else {
            self.core_classes(name)
        };
        exactly_one(found)
    }

    fn core_classes(&mut self, name: &str) -> (BTreeSet<ClassKey>, bool) {
        let core = self.sources.resolve(Path::new(""), "dart:core", true);
        match core {
            Target::Outside(path) => {
                let walk = self.exported(&path, name);
                (walk.classes, !walk.complete)
            }
            Target::Local | Target::Unknown => (BTreeSet::new(), true),
        }
    }

    fn directive_classes(
        &mut self,
        library: &Path,
        imports: &[&DartImport],
        name: &str,
        from_outside: bool,
    ) -> (BTreeSet<ClassKey>, bool) {
        let mut groups = BTreeMap::<(usize, usize), Vec<&DartImport>>::new();
        for import in imports {
            groups
                .entry((import.location.line, import.location.column))
                .or_default()
                .push(import);
        }
        let targets = imports
            .iter()
            .filter_map(
                |import| match self.sources.resolve(library, &import.uri, from_outside) {
                    Target::Outside(path) => Some(path),
                    Target::Local | Target::Unknown => None,
                },
            )
            .collect::<Vec<_>>();
        self.loader.prefetch(&targets, name);
        let mut found = BTreeSet::new();
        let mut matching = 0_usize;
        let mut incomplete = false;
        for branches in groups.into_values() {
            let results = branches
                .iter()
                .map(
                    |import| match self.sources.resolve(library, &import.uri, from_outside) {
                        Target::Outside(path) => self.exported(&path, name),
                        Target::Local => Walk::empty(),
                        Target::Unknown => Walk::unread(),
                    },
                )
                .collect::<Vec<_>>();
            let (classes, complete) = agree(&results);
            incomplete |= !complete;
            if !classes.is_empty() {
                matching += 1;
                found.extend(classes);
            }
        }
        if matching > 1 && found.len() > 1 {
            incomplete = true;
            found.clear();
        }
        (found, incomplete)
    }

    fn exported(&mut self, library: &Path, name: &str) -> Walk {
        let key = (library.to_path_buf(), name.to_owned());
        if let Some(walk) = self.exported.get(&key) {
            return walk.clone();
        }
        let walk = self.export_walk(library, name, &mut BTreeSet::new());
        self.exported.insert(key, walk.clone());
        walk
    }

    fn export_walk(&mut self, library: &Path, name: &str, visited: &mut BTreeSet<PathBuf>) -> Walk {
        if !visited.insert(library.to_path_buf()) {
            return Walk::empty();
        }
        let own = self.library_classes(library, name);
        let mut walk = Walk {
            classes: own.classes,
            complete: own.complete,
        };
        let Outline::Read(file) = self.loader.load(library, name) else {
            return walk;
        };
        let targets = file
            .exports
            .iter()
            .filter(|export| visible_through_combinators(name, &export.combinators))
            .filter_map(
                |export| match self.sources.resolve(library, &export.uri, true) {
                    Target::Outside(path) => Some(path),
                    Target::Local | Target::Unknown => None,
                },
            )
            .collect::<Vec<_>>();
        self.loader.prefetch(&targets, name);
        let mut groups = BTreeMap::<(usize, usize), Vec<&crate::DartExport>>::new();
        for export in &file.exports {
            groups
                .entry((export.location.line, export.location.column))
                .or_default()
                .push(export);
        }
        for branches in groups.into_values() {
            let results = branches
                .iter()
                .map(|export| {
                    if !visible_through_combinators(name, &export.combinators) {
                        return Walk::empty();
                    }
                    match self.sources.resolve(library, &export.uri, true) {
                        Target::Outside(path) => self.export_walk(&path, name, visited),
                        Target::Local => Walk::empty(),
                        Target::Unknown => Walk::unread(),
                    }
                })
                .collect::<Vec<_>>();
            let (classes, complete) = agree(&results);
            walk.classes.extend(classes);
            walk.complete &= complete;
        }
        walk
    }

    fn library_classes(&mut self, library: &Path, name: &str) -> Walk {
        let file = match self.loader.load(library, name) {
            Outline::Read(file) => file,
            Outline::Skipped => return Walk::empty(),
            Outline::Missing | Outline::Unparsed => return Walk::unread(),
        };
        self.library_of
            .insert(library.to_path_buf(), library.to_path_buf());
        let mut walk = Walk::empty();
        if declares(&file, name) {
            walk.classes.insert(ClassKey {
                path: library.to_path_buf(),
                name: name.to_owned(),
            });
        }
        let parts = file
            .parts
            .iter()
            .filter_map(
                |part| match self.sources.resolve(library, &part.uri, true) {
                    Target::Outside(path) => Some(path),
                    Target::Local | Target::Unknown => None,
                },
            )
            .collect::<Vec<_>>();
        self.loader.prefetch(&parts, name);
        for part in &file.parts {
            let Target::Outside(path) = self.sources.resolve(library, &part.uri, true) else {
                walk.complete = false;
                continue;
            };
            match self.loader.load(&path, name) {
                Outline::Read(part_file) => {
                    self.library_of.insert(path.clone(), library.to_path_buf());
                    if declares(&part_file, name) {
                        walk.classes.insert(ClassKey {
                            path,
                            name: name.to_owned(),
                        });
                    }
                }
                Outline::Skipped => {}
                Outline::Missing | Outline::Unparsed => walk.complete = false,
            }
        }
        walk
    }
}

fn agree(results: &[Walk]) -> (BTreeSet<ClassKey>, bool) {
    let complete = results.iter().all(|walk| walk.complete);
    let Some(first) = results.first() else {
        return (BTreeSet::new(), true);
    };
    if results.iter().all(|walk| walk.classes == first.classes) {
        (first.classes.clone(), complete)
    } else {
        (BTreeSet::new(), false)
    }
}

fn exactly_one(mut classes: BTreeSet<ClassKey>) -> Option<ClassKey> {
    let class = classes.pop_first()?;
    classes.is_empty().then_some(class)
}
