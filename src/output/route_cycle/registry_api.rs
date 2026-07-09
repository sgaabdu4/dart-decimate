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
    let mut names = route_file
        .declarations
        .iter()
        .filter(|declaration| {
            is_non_route_registry_api_name(&declaration.name, &route_classes)
                && dependency_imports_name(dependency, &declaration.name)
        })
        .map(|declaration| declaration.name.clone())
        .collect::<BTreeSet<_>>();
    collect_visible_exported_non_route_api_names(
        dependency,
        files_by_path,
        dependencies,
        &route_file.path,
        &[],
        0,
        &route_classes,
        &mut names,
    );
    names
}

fn collect_visible_exported_non_route_api_names(
    dependency: &ResolvedDependency,
    files_by_path: &BTreeMap<PathBuf, &DartFile>,
    dependencies: &[ResolvedDependency],
    from_path: &Path,
    chain: &[DependencyVisibility],
    depth: usize,
    route_classes: &BTreeSet<&str>,
    names: &mut BTreeSet<String>,
) {
    if depth > 8 {
        return;
    }

    for edge in dependencies
        .iter()
        .filter(|edge| edge.kind == DependencyKind::Export && edge.from_path == from_path)
    {
        let mut next_chain = chain.to_owned();
        next_chain.push(edge.visibility.clone());
        if let Some(file) = files_by_path.get(&edge.to_path) {
            names.extend(
                file.declarations
                    .iter()
                    .filter(|declaration| {
                        is_non_route_registry_api_name(&declaration.name, route_classes)
                            && dependency_imports_name(dependency, &declaration.name)
                            && is_visible_through_export_chain(&declaration.name, &next_chain)
                    })
                    .map(|declaration| declaration.name.clone()),
            );
        }
        collect_visible_exported_non_route_api_names(
            dependency,
            files_by_path,
            dependencies,
            &edge.to_path,
            &next_chain,
            depth + 1,
            route_classes,
            names,
        );
    }
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
