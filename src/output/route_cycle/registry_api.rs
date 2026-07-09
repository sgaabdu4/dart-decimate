use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::{
    DartCombinator, DartCombinatorKind, DartFile, DependencyKind, DependencyVisibility,
    ResolvedDependency,
};

pub(super) fn visible_non_route_registry_api_names(
    dependency: &ResolvedDependency,
    route_file: &DartFile,
    files_by_path: &BTreeMap<PathBuf, &DartFile>,
    dependencies: &[ResolvedDependency],
) -> BTreeSet<String> {
    let route_classes = route_file
        .routes
        .iter()
        .map(|route| route.route_class.as_str())
        .collect::<BTreeSet<_>>();
    let mut names = BTreeSet::new();
    let collector = ExportApiCollector {
        dependency,
        files_by_path,
        dependencies,
        route_classes: &route_classes,
    };
    collector.collect_library(&route_file.path, &[], &mut names);
    collector.collect_exports(&route_file.path, &[], 0, &mut names);
    names
}

struct ExportApiCollector<'a> {
    dependency: &'a ResolvedDependency,
    files_by_path: &'a BTreeMap<PathBuf, &'a DartFile>,
    dependencies: &'a [ResolvedDependency],
    route_classes: &'a BTreeSet<&'a str>,
}

impl ExportApiCollector<'_> {
    fn collect_exports(
        &self,
        from_path: &Path,
        chain: &[DependencyVisibility],
        depth: usize,
        names: &mut BTreeSet<String>,
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
            self.collect_library(&edge.to_path, &next_chain, names);
            self.collect_exports(&edge.to_path, &next_chain, depth + 1, names);
        }
    }

    fn collect_library(
        &self,
        library_path: &Path,
        chain: &[DependencyVisibility],
        names: &mut BTreeSet<String>,
    ) {
        for path in library_paths(self.dependencies, library_path) {
            let Some(file) = self.files_by_path.get(&path) else {
                continue;
            };
            names.extend(
                file.declarations
                    .iter()
                    .filter(|declaration| {
                        is_non_route_registry_api_name(&declaration.name, self.route_classes)
                            && dependency_imports_name(self.dependency, &declaration.name)
                            && is_visible_through_export_chain(&declaration.name, chain)
                    })
                    .map(|declaration| declaration.name.clone()),
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

fn is_non_route_registry_api_name(name: &str, route_classes: &BTreeSet<&str>) -> bool {
    !name.starts_with('_')
        && !route_classes.contains(name)
        && !is_route_registry_infrastructure_declaration(name)
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
