use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;

use super::helper_has_typed_route_navigation_call;
use crate::{
    DartCombinatorKind, DartFile, DependencyCycle, DependencyKind, ResolvedDependency,
    scan::ScannedProject,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::output) struct TypedGoRouterCycle {
    pub residual_cycles: Vec<ResidualCycle>,
    pub typed_route_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::output) struct ResidualCycle {
    pub files: Vec<PathBuf>,
    pub edge: Option<ResolvedDependency>,
}

pub(in crate::output) fn decompose_typed_go_router_cycle(
    project: &ScannedProject,
    cycle: &DependencyCycle,
) -> Option<TypedGoRouterCycle> {
    let cycle_files = cycle.files.iter().cloned().collect::<BTreeSet<_>>();
    let route_files = project
        .files
        .iter()
        .filter(|file| cycle_files.contains(&file.path) && is_typed_go_router_registry(file))
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    if route_files.is_empty() {
        return None;
    }

    let files_by_path = project
        .files
        .iter()
        .map(|file| (file.path.clone(), file))
        .collect::<BTreeMap<_, _>>();
    let internal_dependencies = project
        .graph
        .dependencies()
        .into_iter()
        .filter(|dependency| {
            cycle_files.contains(&dependency.from_path)
                && cycle_files.contains(&dependency.to_path)
                && dependency.from_path != dependency.to_path
        })
        .collect::<Vec<_>>();
    let typed_back_edges = internal_dependencies
        .iter()
        .filter(|dependency| {
            is_typed_go_router_navigation_back_edge(dependency, &route_files, &files_by_path)
        })
        .cloned()
        .collect::<Vec<_>>();
    if typed_back_edges.is_empty() {
        return None;
    }

    let residual_dependencies = internal_dependencies
        .iter()
        .filter(|dependency| {
            !typed_back_edges
                .iter()
                .any(|typed| same_dependency(typed, dependency))
        })
        .cloned()
        .collect::<Vec<_>>();
    let residual_reachability = ResidualReachability::new(&residual_dependencies);
    let typed_route_files = typed_route_cycle_files(
        &typed_back_edges,
        &residual_dependencies,
        &residual_reachability,
    );
    if typed_route_files.is_empty() {
        return None;
    }

    let mut residual_cycles = residual_dependency_cycles(&cycle_files, &residual_dependencies);
    append_typed_route_path_edge_errors(
        &typed_back_edges,
        &residual_dependencies,
        &residual_reachability,
        &mut residual_cycles,
    );
    append_route_registry_edge_errors(&route_files, &residual_dependencies, &mut residual_cycles);
    residual_cycles.sort_by(|left, right| left.files.cmp(&right.files));

    Some(TypedGoRouterCycle {
        residual_cycles,
        typed_route_files,
    })
}

fn is_typed_go_router_navigation_back_edge(
    dependency: &ResolvedDependency,
    route_files: &BTreeSet<PathBuf>,
    files_by_path: &BTreeMap<PathBuf, &DartFile>,
) -> bool {
    if dependency.kind != DependencyKind::Import
        || !route_files.contains(&dependency.to_path)
        || route_files.contains(&dependency.from_path)
    {
        return false;
    }
    let Some(route_file) = files_by_path.get(&dependency.to_path) else {
        return false;
    };
    let Some(helper_file) = files_by_path.get(&dependency.from_path) else {
        return false;
    };
    is_typed_go_router_navigation_helper(dependency, helper_file, route_file)
}

fn is_typed_go_router_navigation_helper(
    dependency: &ResolvedDependency,
    helper_file: &DartFile,
    route_file: &DartFile,
) -> bool {
    let route_classes = route_file
        .routes
        .iter()
        .map(|route| route.route_class.clone())
        .collect::<BTreeSet<_>>();
    if route_classes.is_empty()
        || helper_references_non_route_registry_api(dependency, helper_file, route_file)
    {
        return false;
    }
    let Ok(source) = fs::read_to_string(&helper_file.path) else {
        return false;
    };
    let Ok(parsed) = crate::dart_parser::parse_dart_source_lossy(&helper_file.path, &source) else {
        return false;
    };
    helper_has_typed_route_navigation_call(
        parsed.tree().root_node(),
        parsed.source(),
        &route_classes,
    )
}

fn helper_references_non_route_registry_api(
    dependency: &ResolvedDependency,
    helper_file: &DartFile,
    route_file: &DartFile,
) -> bool {
    let route_classes = route_file
        .routes
        .iter()
        .map(|route| route.route_class.as_str())
        .collect::<BTreeSet<_>>();
    let non_route_api_names = route_file
        .declarations
        .iter()
        .filter(|declaration| {
            !declaration.name.starts_with('_')
                && !route_classes.contains(declaration.name.as_str())
                && !is_route_registry_infrastructure_declaration(&declaration.name)
                && dependency_imports_name(dependency, &declaration.name)
        })
        .map(|declaration| declaration.name.as_str())
        .collect::<BTreeSet<_>>();
    !non_route_api_names.is_empty()
        && helper_file
            .references
            .iter()
            .any(|reference| non_route_api_names.contains(reference.name.as_str()))
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

fn typed_route_cycle_files(
    typed_back_edges: &[ResolvedDependency],
    residual_dependencies: &[ResolvedDependency],
    residual_reachability: &ResidualReachability,
) -> Vec<PathBuf> {
    let mut files = BTreeSet::new();
    for edge in typed_back_edges {
        if !residual_reachability.reaches(&edge.to_path, &edge.from_path) {
            continue;
        }
        files.insert(edge.to_path.clone());
        files.insert(edge.from_path.clone());
        for dependency in residual_dependencies.iter().filter(|dependency| {
            dependency_lies_on_typed_route_path(dependency, edge, residual_reachability)
        }) {
            files.insert(dependency.from_path.clone());
            files.insert(dependency.to_path.clone());
        }
    }
    files.into_iter().collect()
}

fn dependency_lies_on_typed_route_path(
    dependency: &ResolvedDependency,
    typed_back_edge: &ResolvedDependency,
    residual_reachability: &ResidualReachability,
) -> bool {
    residual_reachability.reaches(&typed_back_edge.to_path, &dependency.from_path)
        && residual_reachability.reaches(&dependency.to_path, &typed_back_edge.from_path)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResidualReachability {
    reachable: BTreeMap<PathBuf, BTreeSet<PathBuf>>,
}

impl ResidualReachability {
    fn new(dependencies: &[ResolvedDependency]) -> Self {
        let mut adjacency = BTreeMap::<PathBuf, Vec<PathBuf>>::new();
        for dependency in dependencies {
            adjacency
                .entry(dependency.from_path.clone())
                .or_default()
                .push(dependency.to_path.clone());
            adjacency.entry(dependency.to_path.clone()).or_default();
        }
        for next_paths in adjacency.values_mut() {
            next_paths.sort();
            next_paths.dedup();
        }
        let reachable = adjacency
            .keys()
            .map(|path| (path.clone(), reachable_from(path, &adjacency)))
            .collect();
        Self { reachable }
    }

    fn reaches(&self, from: &Path, to: &Path) -> bool {
        from == to
            || self
                .reachable
                .get(from)
                .is_some_and(|targets| targets.contains(to))
    }
}

fn reachable_from(start: &Path, adjacency: &BTreeMap<PathBuf, Vec<PathBuf>>) -> BTreeSet<PathBuf> {
    let mut reachable = BTreeSet::new();
    let mut queue = VecDeque::new();
    if let Some(next_paths) = adjacency.get(start) {
        for next_path in next_paths {
            if reachable.insert(next_path.clone()) {
                queue.push_back(next_path.clone());
            }
        }
    }
    while let Some(path) = queue.pop_front() {
        let Some(next_paths) = adjacency.get(&path) else {
            continue;
        };
        for next_path in next_paths {
            if reachable.insert(next_path.clone()) {
                queue.push_back(next_path.clone());
            }
        }
    }
    reachable
}

fn residual_dependency_cycles(
    cycle_files: &BTreeSet<PathBuf>,
    dependencies: &[ResolvedDependency],
) -> Vec<ResidualCycle> {
    let mut graph = DiGraph::<PathBuf, ()>::new();
    let nodes = cycle_files
        .iter()
        .map(|path| (path.clone(), graph.add_node(path.clone())))
        .collect::<BTreeMap<_, _>>();
    for dependency in dependencies {
        let Some(from) = nodes.get(&dependency.from_path).copied() else {
            continue;
        };
        let Some(to) = nodes.get(&dependency.to_path).copied() else {
            continue;
        };
        graph.add_edge(from, to, ());
    }

    let mut cycles = tarjan_scc(&graph)
        .into_iter()
        .filter_map(|component| residual_cycle_from_component(&graph, component, dependencies))
        .collect::<Vec<_>>();
    cycles.sort_by(|left, right| left.files.cmp(&right.files));
    cycles
}

fn residual_cycle_from_component(
    graph: &DiGraph<PathBuf, ()>,
    component: Vec<NodeIndex>,
    dependencies: &[ResolvedDependency],
) -> Option<ResidualCycle> {
    let first = component.first().copied()?;
    let is_cycle = component.len() > 1 || component_has_self_loop(graph, first);
    if !is_cycle {
        return None;
    }
    let mut files = component
        .into_iter()
        .map(|node| graph[node].clone())
        .collect::<Vec<_>>();
    files.sort();
    let edge = first_dependency_in_cycle(&files, dependencies).cloned();
    Some(ResidualCycle { files, edge })
}

fn component_has_self_loop(graph: &DiGraph<PathBuf, ()>, node: NodeIndex) -> bool {
    graph.edges(node).any(|edge| edge.target() == node)
}

fn first_dependency_in_cycle<'dependencies>(
    files: &[PathBuf],
    dependencies: &'dependencies [ResolvedDependency],
) -> Option<&'dependencies ResolvedDependency> {
    dependencies
        .iter()
        .filter(|dependency| {
            files.contains(&dependency.from_path) && files.contains(&dependency.to_path)
        })
        .min_by(|left, right| {
            (
                &left.from_path,
                &left.to_path,
                dependency_kind_order(left.kind),
                &left.specifier,
            )
                .cmp(&(
                    &right.from_path,
                    &right.to_path,
                    dependency_kind_order(right.kind),
                    &right.specifier,
                ))
        })
}

fn append_route_registry_edge_errors(
    route_files: &BTreeSet<PathBuf>,
    dependencies: &[ResolvedDependency],
    residual_cycles: &mut Vec<ResidualCycle>,
) {
    for dependency in dependencies.iter().filter(|dependency| {
        route_files.contains(&dependency.from_path)
            && !route_files.contains(&dependency.to_path)
            && dependency.kind != DependencyKind::Import
    }) {
        if residual_cycles.iter().any(|cycle| {
            cycle.files.contains(&dependency.from_path) && cycle.files.contains(&dependency.to_path)
        }) {
            continue;
        }
        let mut files = vec![dependency.from_path.clone(), dependency.to_path.clone()];
        files.sort();
        residual_cycles.push(ResidualCycle {
            files,
            edge: Some(dependency.clone()),
        });
    }
}

fn append_typed_route_path_edge_errors(
    typed_back_edges: &[ResolvedDependency],
    residual_dependencies: &[ResolvedDependency],
    residual_reachability: &ResidualReachability,
    residual_cycles: &mut Vec<ResidualCycle>,
) {
    let mut path_edges = residual_dependencies
        .iter()
        .filter(|dependency| dependency.kind != DependencyKind::Import)
        .filter(|dependency| {
            typed_back_edges.iter().any(|typed_back_edge| {
                dependency_lies_on_typed_route_path(
                    dependency,
                    typed_back_edge,
                    residual_reachability,
                )
            })
        })
        .cloned()
        .collect::<Vec<_>>();
    path_edges.sort_by(|left, right| {
        (
            &left.from_path,
            &left.to_path,
            dependency_kind_order(left.kind),
            &left.specifier,
        )
            .cmp(&(
                &right.from_path,
                &right.to_path,
                dependency_kind_order(right.kind),
                &right.specifier,
            ))
    });
    for dependency in path_edges {
        if residual_cycles.iter().any(|cycle| {
            cycle.files.contains(&dependency.from_path) && cycle.files.contains(&dependency.to_path)
        }) {
            continue;
        }
        let mut files = vec![dependency.from_path.clone(), dependency.to_path.clone()];
        files.sort();
        residual_cycles.push(ResidualCycle {
            files,
            edge: Some(dependency),
        });
    }
}

fn same_dependency(left: &ResolvedDependency, right: &ResolvedDependency) -> bool {
    left.from_path == right.from_path
        && left.to_path == right.to_path
        && left.kind == right.kind
        && left.specifier == right.specifier
        && left.location == right.location
}

fn dependency_kind_order(kind: DependencyKind) -> u8 {
    match kind {
        DependencyKind::Import => 0,
        DependencyKind::Export => 1,
        DependencyKind::Part => 2,
        DependencyKind::Augment => 3,
    }
}

fn is_typed_go_router_registry(file: &DartFile) -> bool {
    if file.routes.is_empty()
        && !file.references.iter().any(|reference| {
            matches!(
                reference.name.as_str(),
                "TypedGoRoute"
                    | "TypedRelativeGoRoute"
                    | "TypedShellRoute"
                    | "TypedStatefulShellRoute"
                    | "TypedStatefulShellBranch"
            )
        })
    {
        return false;
    }

    file.parts.iter().any(|part| part.uri.ends_with(".g.dart"))
}
