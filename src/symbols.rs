use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::dependencies::{PackageScope, package_scopes};
use crate::generated::is_generated_dart_path;
use crate::graph::normalize_against;
use crate::outside::OutsideTypes;
use crate::widgets::resolution::{ClassKey, DeclarationResolver, Resolution};
use crate::{
    DartCombinatorKind, DeadCodeReport, DeclarationKind, DependencyKind, Location,
    MemberDeclaration, MemberKind, ScannedProject, TopLevelDeclaration,
};

mod duplicate_declarations;
mod extensions;
mod index;
pub(crate) mod path_filters;
mod private_type_leaks;
mod riverpod;
mod semantic_evidence;
use duplicate_declarations::duplicate_declarations_across_libraries;
use extensions::extend_implicit_extension_references;
use path_filters::{is_bin_source, is_library_source, is_private, is_public_library_entry};
pub use private_type_leaks::PrivateTypeLeak;
use private_type_leaks::private_type_leaks;
use riverpod::extend_generated_provider_owner_references;
use semantic_evidence::semantic_report;

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
    /// Conservative Rust-native semantic evidence for retained candidates and type edges.
    pub semantic: crate::SemanticReport,
    /// Supertype names that SDK or package sources could not resolve for class member checks.
    #[serde(default, skip_serializing)]
    pub outside_types_unread: usize,
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
    let mut duplicate_exports = duplicate_exports_from_public(&index.public_exported_declarations);
    if let Some(dead_code) = dead_code {
        let across_libraries =
            duplicate_declarations_across_libraries(project, dead_code, &index, &duplicate_exports);
        duplicate_exports.extend(across_libraries);
        duplicate_exports.sort_by(|left, right| {
            (&left.entry_path, &left.name).cmp(&(&right.entry_path, &right.name))
        });
    }
    let private_type_leaks = if options.private_type_leaks {
        private_type_leaks(project, &index)
    } else {
        Vec::new()
    };
    let (unused_exports, (unused_members, outside_types_unread)) =
        if let Some(dead_code) = dead_code {
            (
                unused_exports(project, dead_code, &index, options),
                unused_members(project, dead_code, &index),
            )
        } else {
            (Vec::new(), (Vec::new(), 0))
        };
    let semantic = semantic_report(
        project,
        &index,
        &unused_exports,
        &unused_members,
        &private_type_leaks,
        &duplicate_exports,
    );

    SymbolReport {
        unused_exports,
        unused_members,
        private_type_leaks,
        duplicate_exports,
        semantic,
        outside_types_unread,
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
) -> (Vec<UnusedMember>, usize) {
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

    let public_members = PublicMemberContext::new(project, dead_code, index);
    let mut unused_members = index
        .members
        .iter()
        .filter_map(|member| {
            unused_member_from_declaration(
                project,
                index,
                (&reachable_files, &dead_files),
                &public_members,
                member,
            )
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

    (
        unused_members,
        public_members.interface_skips.unread_names.len(),
    )
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
    (reachable_files, dead_files): (&BTreeSet<PathBuf>, &BTreeSet<PathBuf>),
    public_members: &PublicMemberContext,
    indexed: &IndexedMember,
) -> Option<UnusedMember> {
    if !indexed.path.starts_with(&project.root)
        || !reachable_files.contains(&indexed.path)
        || dead_files.contains(&indexed.path)
        || is_generated_dart_path(&indexed.path)
        || !public_members.in_checked_source(&project.root, &indexed.path)
        || !should_check_member(index, public_members, indexed)
    {
        return None;
    }

    let reference_count = if indexed.member.kind == MemberKind::EnumConstant {
        index.qualified_reference_count(
            &indexed.member.owner,
            &indexed.member.name,
            reachable_files,
        ) + index.dot_shorthand_reference_count(&indexed.member.name, reachable_files)
            + index.qualified_reference_count(&indexed.member.owner, "values", reachable_files)
    } else if is_private(&indexed.member.name) {
        let library = index.library_path(&indexed.path);
        index.library_reference_count(&indexed.member.name, library, reachable_files)
    } else {
        index.reference_count(&indexed.member.name, reachable_files)
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

fn should_check_member(
    index: &SymbolIndex,
    public_members: &PublicMemberContext,
    indexed: &IndexedMember,
) -> bool {
    match indexed.member.kind {
        MemberKind::EnumConstant => {
            !is_private(&indexed.member.name)
                && !index.is_public_export(&indexed.path, &indexed.member.owner)
                && !public_members.exports_class(&indexed.path, &indexed.member.owner)
        }
        MemberKind::Field | MemberKind::Getter | MemberKind::Setter | MemberKind::Method => {
            is_private(&indexed.member.name) || public_members.checks(index, indexed)
        }
        MemberKind::Constructor | MemberKind::Operator => false,
    }
}

const CONTRACT_MEMBER_NAMES: &[&str] = &[
    "build",
    "call",
    "createState",
    "fromJson",
    "hashCode",
    "noSuchMethod",
    "toJson",
    "toString",
];

struct PublicMemberContext {
    root: PathBuf,
    class_owners: BTreeSet<(PathBuf, String)>,
    generated_part_libraries: BTreeSet<PathBuf>,
    entry_points: BTreeSet<PathBuf>,
    packages: Vec<PackageScope>,
    public_classes: BTreeSet<(PathBuf, String)>,
    interface_skips: InterfaceSkips,
}

struct HierarchyClass {
    extended: BTreeSet<ClassKey>,
    mixed: BTreeSet<ClassKey>,
    implemented: BTreeSet<ClassKey>,
    unread_extended: BTreeSet<String>,
    unread_interface: BTreeSet<String>,
}

#[derive(Default)]
struct DeclaredNames {
    all: BTreeSet<String>,
    without_body: BTreeSet<String>,
}

#[derive(Default)]
struct InterfaceSkips {
    whole_classes: BTreeSet<ClassKey>,
    named_members: BTreeMap<ClassKey, BTreeSet<String>>,
    unread_names: BTreeSet<String>,
}

impl InterfaceSkips {
    fn protects(&self, class: &ClassKey, member: &str) -> bool {
        self.whole_classes.contains(class)
            || self
                .named_members
                .get(class)
                .is_some_and(|names| names.contains(member))
    }
}

struct Hierarchy {
    classes: BTreeMap<ClassKey, HierarchyClass>,
    member_names: BTreeMap<ClassKey, DeclaredNames>,
    outside: BTreeSet<ClassKey>,
}

struct LinkResolver<'a> {
    resolver: &'a DeclarationResolver,
    outside: &'a mut OutsideTypes,
    path: &'a Path,
    library: &'a Path,
    imports: &'a [crate::DartImport],
}

impl LinkResolver<'_> {
    fn resolve_all(&mut self, names: &[String]) -> (BTreeSet<ClassKey>, BTreeSet<String>) {
        let mut found = BTreeSet::new();
        let mut unread = BTreeSet::new();
        for name in names {
            match self.resolver.resolve_detailed(self.path, name) {
                Resolution::Found(classes) if !classes.is_empty() => found.extend(classes),
                Resolution::Absent => {
                    match self.outside.link(self.library, self.imports, name, false) {
                        Some(class) => {
                            found.insert(class);
                        }
                        None => {
                            unread.insert(name.clone());
                        }
                    }
                }
                Resolution::Found(_) | Resolution::Unknown => {
                    unread.insert(name.clone());
                }
            }
        }
        (found, unread)
    }

    fn hierarchy_class(&mut self, class: &crate::ClassSupertypes) -> HierarchyClass {
        let superclasses = class
            .names
            .iter()
            .chain(&class.constraints)
            .filter(|name| !matches!(name.as_str(), "Object" | "core.Object"))
            .cloned()
            .collect::<Vec<_>>();
        let (extended, unread_extended) = self.resolve_all(&superclasses);
        let (mixed, mut unread_interface) = self.resolve_all(&class.mixins);
        let (implemented, unread_implemented) = self.resolve_all(&class.implemented);
        unread_interface.extend(unread_implemented);
        HierarchyClass {
            extended,
            mixed,
            implemented,
            unread_extended,
            unread_interface,
        }
    }
}

fn class_hierarchy(project: &ScannedProject, index: &SymbolIndex) -> Hierarchy {
    let resolver = DeclarationResolver::new(
        project,
        index.declarations.iter().map(|indexed| ClassKey {
            path: indexed.path.clone(),
            name: indexed.declaration.name.clone(),
        }),
    );
    let imports_by_path = project
        .files
        .iter()
        .map(|file| {
            (
                normalize_against(&project.root, &file.path),
                file.imports.as_slice(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut outside = OutsideTypes::new(&project.root);
    let mut classes = BTreeMap::<ClassKey, HierarchyClass>::new();
    let mut member_names = BTreeMap::<ClassKey, DeclaredNames>::new();
    for file in &project.files {
        let path = normalize_against(&project.root, &file.path);
        for member in &file.members {
            let declared = member_names
                .entry(ClassKey {
                    path: path.clone(),
                    name: member.owner.clone(),
                })
                .or_default();
            declared.all.insert(member.name.clone());
            if !member.has_body {
                declared.without_body.insert(member.name.clone());
            }
        }
        let library = index.library_path(&path).to_path_buf();
        let mut links = LinkResolver {
            resolver: &resolver,
            outside: &mut outside,
            path: &path,
            library: &library,
            imports: imports_by_path.get(&library).copied().unwrap_or_default(),
        };
        for class in &file.class_supertypes {
            let key = ClassKey {
                path: path.clone(),
                name: class.owner.clone(),
            };
            classes.insert(key, links.hierarchy_class(class));
        }
    }
    let mut outside_keys = BTreeSet::new();
    for (key, class) in outside.expand() {
        outside_keys.insert(key.clone());
        member_names.insert(
            key.clone(),
            DeclaredNames {
                all: class.names,
                without_body: BTreeSet::new(),
            },
        );
        classes.insert(
            key,
            HierarchyClass {
                extended: class.extended,
                mixed: class.mixed,
                implemented: class.implemented,
                unread_extended: class.unread_extended,
                unread_interface: class.unread_interface,
            },
        );
    }
    Hierarchy {
        classes,
        member_names,
        outside: outside_keys,
    }
}

fn reached_interfaces(
    class: &HierarchyClass,
    classes: &BTreeMap<ClassKey, HierarchyClass>,
) -> (BTreeMap<ClassKey, bool>, BTreeSet<String>) {
    let mut reached = BTreeMap::<ClassKey, bool>::new();
    let mut open = class.unread_interface.clone();
    let mut pending = class
        .mixed
        .iter()
        .map(|mixin| (mixin.clone(), false))
        .chain(
            class
                .implemented
                .iter()
                .map(|interface| (interface.clone(), true)),
        )
        .collect::<Vec<_>>();
    if !class.mixed.is_empty() {
        open.extend(class.unread_extended.iter().cloned());
        pending.extend(class.extended.iter().map(|parent| (parent.clone(), false)));
    }
    while let Some((reached_class, duty)) = pending.pop() {
        if reached
            .get(&reached_class)
            .is_some_and(|known| *known || !duty)
        {
            continue;
        }
        reached.insert(reached_class.clone(), duty);
        if let Some(parent) = classes.get(&reached_class) {
            open.extend(parent.unread_extended.iter().cloned());
            open.extend(parent.unread_interface.iter().cloned());
            pending.extend(
                parent
                    .extended
                    .iter()
                    .chain(&parent.mixed)
                    .map(|supertype| (supertype.clone(), duty))
                    .chain(
                        parent
                            .implemented
                            .iter()
                            .map(|interface| (interface.clone(), true)),
                    ),
            );
        }
    }
    (reached, open)
}

fn interface_skips(project: &ScannedProject, index: &SymbolIndex) -> InterfaceSkips {
    let Hierarchy {
        classes,
        member_names,
        outside,
    } = class_hierarchy(project, index);
    let checkable = index
        .members
        .iter()
        .filter(|indexed| {
            matches!(
                indexed.member.kind,
                MemberKind::Field | MemberKind::Getter | MemberKind::Setter | MemberKind::Method
            )
        })
        .map(|indexed| ClassKey {
            path: indexed.path.clone(),
            name: indexed.member.owner.clone(),
        })
        .collect::<BTreeSet<_>>();
    let mut skips = InterfaceSkips::default();
    for class in classes.values() {
        let (reached, open) = reached_interfaces(class, &classes);
        if reached.is_empty() && open.is_empty() {
            continue;
        }
        let mut ancestors = BTreeSet::new();
        let mut pending = class
            .extended
            .iter()
            .chain(&class.mixed)
            .cloned()
            .collect::<Vec<_>>();
        while let Some(ancestor) = pending.pop() {
            if ancestors.insert(ancestor.clone())
                && let Some(parent) = classes.get(&ancestor)
            {
                pending.extend(parent.extended.iter().chain(&parent.mixed).cloned());
            }
        }
        if !open.is_empty() {
            if ancestors
                .iter()
                .any(|ancestor| checkable.contains(ancestor))
            {
                skips.unread_names.extend(open);
            }
            skips.whole_classes.extend(ancestors);
            continue;
        }
        for ancestor in &ancestors {
            let ancestor_is_reached = reached.contains_key(ancestor);
            let names = reached
                .iter()
                .filter(|(reached_class, _)| *reached_class != ancestor)
                .filter_map(|(reached_class, duty)| {
                    member_names.get(reached_class).map(|declared| {
                        if *duty || !ancestor_is_reached || outside.contains(reached_class) {
                            &declared.all
                        } else {
                            &declared.without_body
                        }
                    })
                })
                .flatten()
                .cloned()
                .collect::<BTreeSet<_>>();
            skips
                .named_members
                .entry(ancestor.clone())
                .or_default()
                .extend(names);
        }
    }
    for (key, class) in &classes {
        if outside.contains(key) {
            continue;
        }
        let (names, unread) = inherited(class, &classes, &member_names);
        if unread.is_empty() {
            skips
                .named_members
                .entry(key.clone())
                .or_default()
                .extend(names);
        } else {
            if checkable.contains(key) {
                skips.unread_names.extend(unread);
            }
            skips.whole_classes.insert(key.clone());
        }
    }
    skips
}

fn inherited(
    class: &HierarchyClass,
    classes: &BTreeMap<ClassKey, HierarchyClass>,
    member_names: &BTreeMap<ClassKey, DeclaredNames>,
) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut names = BTreeSet::new();
    let mut unread = class
        .unread_extended
        .iter()
        .chain(&class.unread_interface)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut pending = class
        .extended
        .iter()
        .chain(&class.mixed)
        .chain(&class.implemented)
        .cloned()
        .collect::<Vec<_>>();
    while let Some(ancestor) = pending.pop() {
        if !seen.insert(ancestor.clone()) {
            continue;
        }
        if let Some(declared) = member_names.get(&ancestor) {
            names.extend(declared.all.iter().cloned());
        }
        if let Some(parent) = classes.get(&ancestor) {
            unread.extend(parent.unread_extended.iter().cloned());
            unread.extend(parent.unread_interface.iter().cloned());
            pending.extend(
                parent
                    .extended
                    .iter()
                    .chain(&parent.mixed)
                    .chain(&parent.implemented)
                    .cloned(),
            );
        }
    }
    (names, unread)
}

fn owning_scope<'scope>(
    packages: &'scope [PackageScope],
    path: &Path,
) -> Option<&'scope PackageScope> {
    packages
        .iter()
        .filter(|scope| path.starts_with(&scope.root))
        .max_by_key(|scope| scope.root.components().count())
}

impl PublicMemberContext {
    fn new(project: &ScannedProject, dead_code: &DeadCodeReport, index: &SymbolIndex) -> Self {
        let packages = package_scopes(&project.root);
        let public_classes = public_exported_declarations_from(project, |path| {
            owning_scope(&packages, path).map_or_else(
                || is_public_library_entry(&project.root, path),
                |scope| !scope.application && is_public_library_entry(&scope.root, path),
            )
        })
        .into_iter()
        .map(|exported| (exported.path, exported.declaration.name))
        .collect();
        Self {
            root: project.root.clone(),
            interface_skips: interface_skips(project, index),
            packages,
            public_classes,
            class_owners: index
                .declarations
                .iter()
                .filter(|indexed| {
                    matches!(
                        indexed.declaration.kind,
                        DeclarationKind::Class | DeclarationKind::Mixin
                    )
                })
                .map(|indexed| (indexed.path.clone(), indexed.declaration.name.clone()))
                .collect(),
            generated_part_libraries: project
                .files
                .iter()
                .filter(|file| {
                    file.parts
                        .iter()
                        .any(|part| is_generated_dart_path(Path::new(&part.uri)))
                })
                .map(|file| {
                    index
                        .library_path(&normalize_against(&project.root, &file.path))
                        .to_path_buf()
                })
                .collect(),
            entry_points: dead_code.entry_points.iter().cloned().collect(),
        }
    }

    fn exports_class(&self, path: &Path, owner: &str) -> bool {
        self.public_classes
            .contains(&(path.to_path_buf(), owner.to_owned()))
    }

    fn in_checked_source(&self, root: &Path, path: &Path) -> bool {
        let root = owning_scope(&self.packages, path).map_or(root, |scope| scope.root.as_path());
        is_library_source(root, path) || is_bin_source(root, path)
    }

    fn is_runtime_entry(&self, root: &Path, path: &Path) -> bool {
        let scope = owning_scope(&self.packages, path);
        self.entry_points.contains(path)
            && scope.is_none_or(|scope| !scope.application)
            && !is_bin_source(scope.map_or(root, |scope| scope.root.as_path()), path)
    }

    fn checks(&self, index: &SymbolIndex, indexed: &IndexedMember) -> bool {
        !indexed.member.contract
            && !CONTRACT_MEMBER_NAMES.contains(&indexed.member.name.as_str())
            && self
                .class_owners
                .contains(&(indexed.path.clone(), indexed.member.owner.clone()))
            && !self.interface_skips.protects(
                &ClassKey {
                    path: indexed.path.clone(),
                    name: indexed.member.owner.clone(),
                },
                &indexed.member.name,
            )
            && !self.is_runtime_entry(&self.root, &indexed.path)
            && !self.exports_class(&indexed.path, &indexed.member.owner)
            && !self
                .generated_part_libraries
                .contains(index.library_path(&indexed.path))
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
    public_exported_declarations_from(project, |path| is_public_library_entry(&project.root, path))
}

fn public_exported_declarations_from(
    project: &ScannedProject,
    is_public_entry: impl Fn(&Path) -> bool,
) -> Vec<PublicExportedDeclaration> {
    let declarations = declarations_by_path(project);
    let public_entry_files = project
        .files
        .iter()
        .map(|file| normalize_against(&project.root, &file.path))
        .filter(|path| is_public_entry(path))
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
