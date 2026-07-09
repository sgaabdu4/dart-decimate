use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::{
    DartCombinator, DartCombinatorKind, DartFile, DeclarationKind, DependencyKind,
    DependencyVisibility, MemberDeclaration, MemberKind, ResolvedDependency,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct VisibleNonRouteRegistryApi {
    pub(super) top_level_names: BTreeSet<String>,
    pub(super) member_names: BTreeSet<String>,
}

impl VisibleNonRouteRegistryApi {
    pub(super) fn is_empty(&self) -> bool {
        self.top_level_names.is_empty() && self.member_names.is_empty()
    }
}

pub(super) fn visible_non_route_registry_api(
    dependency: &ResolvedDependency,
    route_file: &DartFile,
    files_by_path: &BTreeMap<PathBuf, &DartFile>,
    dependencies: &[ResolvedDependency],
) -> VisibleNonRouteRegistryApi {
    let route_classes = route_file
        .routes
        .iter()
        .map(|route| route.route_class.clone())
        .collect::<BTreeSet<_>>();
    let navigation_member_owners = route_registry_navigation_member_owners(
        route_file,
        files_by_path,
        dependencies,
        &route_classes,
    );
    let mut api = VisibleNonRouteRegistryApi::default();
    let collector = ExportApiCollector {
        dependency,
        files_by_path,
        dependencies,
        route_classes: &route_classes,
        navigation_member_owners: &navigation_member_owners,
    };
    collector.collect_library(&route_file.path, &[], &mut api);
    collector.collect_exports(&route_file.path, &[], 0, &mut api);
    api
}

struct ExportApiCollector<'a> {
    dependency: &'a ResolvedDependency,
    files_by_path: &'a BTreeMap<PathBuf, &'a DartFile>,
    dependencies: &'a [ResolvedDependency],
    route_classes: &'a BTreeSet<String>,
    navigation_member_owners: &'a BTreeSet<String>,
}

impl ExportApiCollector<'_> {
    fn collect_exports(
        &self,
        from_path: &Path,
        chain: &[DependencyVisibility],
        depth: usize,
        api: &mut VisibleNonRouteRegistryApi,
    ) {
        if depth > 8 {
            return;
        }

        for edge in self
            .dependencies
            .iter()
            .filter(|edge| edge.kind == DependencyKind::Export && edge.from_path == from_path)
        {
            let mut next_chain = chain.to_owned();
            next_chain.push(edge.visibility.clone());
            self.collect_library(&edge.to_path, &next_chain, api);
            self.collect_exports(&edge.to_path, &next_chain, depth + 1, api);
        }
    }

    fn collect_library(
        &self,
        library_path: &Path,
        chain: &[DependencyVisibility],
        api: &mut VisibleNonRouteRegistryApi,
    ) {
        for path in library_paths(self.dependencies, library_path) {
            let Some(file) = self.files_by_path.get(&path) else {
                continue;
            };
            api.top_level_names.extend(
                file.declarations
                    .iter()
                    .filter(|declaration| {
                        is_non_route_registry_api_name(&declaration.name, self.route_classes)
                            && dependency_imports_name(self.dependency, &declaration.name)
                            && is_visible_through_export_chain(&declaration.name, chain)
                    })
                    .map(|declaration| declaration.name.clone()),
            );
            api.member_names.extend(
                file.members
                    .iter()
                    .filter(|member| {
                        is_non_route_registry_api_member(member, self.navigation_member_owners)
                            && dependency_imports_name(self.dependency, &member.owner)
                            && is_visible_through_export_chain(&member.owner, chain)
                    })
                    .map(|member| member.name.clone()),
            );
        }
    }
}

fn library_paths(dependencies: &[ResolvedDependency], library_path: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::from([library_path.to_path_buf()]);
    let mut seen = BTreeSet::from([library_path.to_path_buf()]);
    let mut index = 0;

    while let Some(path) = paths.get(index).cloned() {
        index += 1;
        for edge in dependencies
            .iter()
            .filter(|edge| edge.kind == DependencyKind::Part && edge.from_path == path)
        {
            if seen.insert(edge.to_path.clone()) {
                paths.push(edge.to_path.clone());
            }
        }
    }

    paths
}

fn route_registry_navigation_member_owners(
    route_file: &DartFile,
    files_by_path: &BTreeMap<PathBuf, &DartFile>,
    dependencies: &[ResolvedDependency],
    route_classes: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut owners = route_classes.clone();
    for path in library_paths(dependencies, &route_file.path) {
        let Some(file) = files_by_path.get(&path) else {
            continue;
        };
        owners.extend(file.declarations.iter().filter_map(|declaration| {
            matches!(
                declaration.kind,
                DeclarationKind::Extension | DeclarationKind::Mixin
            )
            .then_some(declaration.name.as_str())
            .filter(|name| generated_typed_route_navigation_owner(name, route_classes))
            .map(str::to_owned)
        }));
    }
    owners
}

fn generated_typed_route_navigation_owner(name: &str, route_classes: &BTreeSet<String>) -> bool {
    let Some(generated) = name.strip_prefix('$') else {
        return false;
    };
    route_classes.iter().any(|route_class| {
        generated == route_class
            || generated
                .strip_suffix("Extension")
                .is_some_and(|owner| owner == route_class)
    })
}

fn is_non_route_registry_api_name(name: &str, route_classes: &BTreeSet<String>) -> bool {
    !name.starts_with('_')
        && !route_classes.contains(name)
        && !is_route_registry_infrastructure_declaration(name)
}

fn is_non_route_registry_api_member(
    member: &MemberDeclaration,
    navigation_member_owners: &BTreeSet<String>,
) -> bool {
    !member.name.starts_with('_')
        && !member.owner.starts_with('_')
        && member.kind != MemberKind::Constructor
        && !is_route_registry_navigation_member(member, navigation_member_owners)
        && !is_route_registry_infrastructure_declaration(&member.owner)
}

fn is_route_registry_navigation_member(
    member: &MemberDeclaration,
    navigation_member_owners: &BTreeSet<String>,
) -> bool {
    navigation_member_owners.contains(&member.owner)
        && matches!(
            member.name.as_str(),
            "go" | "push" | "pushReplacement" | "replace" | "location"
        )
}

fn is_route_registry_infrastructure_declaration(name: &str) -> bool {
    matches!(
        name,
        "BuildContext"
            | "ConsumerState"
            | "CustomTransitionPage"
            | "GoRouteData"
            | "GoRouter"
            | "GoRouterState"
            | "MaterialPage"
            | "NoTransitionPage"
            | "Page"
            | "ShellRouteData"
            | "State"
            | "StatefulWidget"
            | "StatefulShellRouteData"
            | "StatelessWidget"
            | "TypedGoRoute"
            | "TypedRelativeGoRoute"
            | "TypedShellRoute"
            | "TypedStatefulShellBranch"
            | "TypedStatefulShellRoute"
            | "Widget"
    )
}

fn dependency_imports_name(dependency: &ResolvedDependency, name: &str) -> bool {
    let mut show_seen = false;
    let mut shown = false;
    for combinator in &dependency.visibility.combinators {
        match combinator.kind {
            DartCombinatorKind::Show => {
                show_seen = true;
                if combinator.names.iter().any(|shown| shown == name) {
                    shown = true;
                }
            }
            DartCombinatorKind::Hide if combinator.names.iter().any(|hidden| hidden == name) => {
                return false;
            }
            DartCombinatorKind::Hide => {}
        }
    }
    !show_seen || shown
}

fn is_visible_through_export_chain(name: &str, chain: &[DependencyVisibility]) -> bool {
    chain
        .iter()
        .all(|visibility| is_visible_through_combinators(name, &visibility.combinators))
}

fn is_visible_through_combinators(name: &str, combinators: &[DartCombinator]) -> bool {
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
