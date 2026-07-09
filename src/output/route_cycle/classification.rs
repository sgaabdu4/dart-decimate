use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use tree_sitter::Node;

use super::{
    helper_has_typed_route_navigation_call,
    navigation::term_identifier_shadowed_at,
    registry_api::{VisibleNonRouteRegistryApi, visible_non_route_registry_api},
    visit_named,
};
use crate::{
    DartFile, DependencyCycle, DependencyKind, Location, ResolvedDependency, scan::ScannedProject,
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
    let dependencies = project.graph.dependencies();
    let internal_dependencies = dependencies
        .iter()
        .filter(|dependency| {
            cycle_files.contains(&dependency.from_path)
                && cycle_files.contains(&dependency.to_path)
                && dependency.from_path != dependency.to_path
        })
        .cloned()
        .collect::<Vec<_>>();
    let typed_back_edges = internal_dependencies
        .iter()
        .filter(|dependency| {
            is_typed_go_router_navigation_back_edge(
                dependency,
                &route_files,
                &files_by_path,
                &dependencies,
            )
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
        &route_files,
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
    dependencies: &[ResolvedDependency],
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
    is_typed_go_router_navigation_helper(
        dependency,
        helper_file,
        route_file,
        files_by_path,
        dependencies,
    )
}

fn is_typed_go_router_navigation_helper(
    dependency: &ResolvedDependency,
    helper_file: &DartFile,
    route_file: &DartFile,
    files_by_path: &BTreeMap<PathBuf, &DartFile>,
    dependencies: &[ResolvedDependency],
) -> bool {
    let route_classes = route_file
        .routes
        .iter()
        .map(|route| route.route_class.clone())
        .collect::<BTreeSet<_>>();
    if route_classes.is_empty() {
        return false;
    }
    let Ok(source) = fs::read_to_string(&helper_file.path) else {
        return false;
    };
    let Ok(parsed) = crate::dart_parser::parse_dart_source_lossy(&helper_file.path, &source) else {
        return false;
    };
    let root = parsed.tree().root_node();
    let source = parsed.source();
    if helper_references_non_route_registry_api(
        dependency,
        helper_file,
        route_file,
        files_by_path,
        dependencies,
        root,
        source,
    ) {
        return false;
    }
    helper_has_typed_route_navigation_call(root, source, &route_classes)
}

fn helper_references_non_route_registry_api(
    dependency: &ResolvedDependency,
    helper_file: &DartFile,
    route_file: &DartFile,
    files_by_path: &BTreeMap<PathBuf, &DartFile>,
    dependencies: &[ResolvedDependency],
    root: Node<'_>,
    source: &str,
) -> bool {
    let non_route_api =
        visible_non_route_registry_api(dependency, route_file, files_by_path, dependencies);
    !non_route_api.is_empty()
        && helper_references_imported_api(dependency, helper_file, root, source, &non_route_api)
}

fn helper_references_imported_api(
    dependency: &ResolvedDependency,
    helper_file: &DartFile,
    root: Node<'_>,
    source: &str,
    imported_api: &VisibleNonRouteRegistryApi,
) -> bool {
    let mut found = false;
    let mut syntactic_references = BTreeSet::new();
    visit_named(root, &mut |node| {
        if found {
            return;
        }
        let Some(name) = reference_identifier_text(node, source) else {
            return;
        };
        syntactic_references.insert((
            name.to_owned(),
            node.start_position().row + 1,
            node.start_position().column,
        ));
        if imported_api.top_level_names.contains(name)
            && top_level_reference_uses_imported_api(
                dependency,
                helper_file,
                root,
                node,
                source,
                name,
            )
        {
            found = true;
            return;
        }
        if imported_api.member_names.contains(name)
            && member_reference_uses_imported_api(dependency, root, node, source)
        {
            found = true;
        }
    });
    found
        || (dependency.visibility.prefix.is_none()
            && helper_file.references.iter().any(|reference| {
                let matched = imported_api
                    .top_level_names
                    .contains(reference.name.as_str())
                    && !syntactic_references.contains(&(
                        reference.name.clone(),
                        reference.location.line,
                        reference.location.column,
                    ))
                    && !helper_declares_or_aliases_name(helper_file, &reference.name)
                    && !reference_is_declaration_name(
                        root,
                        source,
                        &reference.name,
                        reference.location,
                    )
                    && !reference_shadowed_at(root, source, &reference.name, reference.location);
                matched
            }))
}

fn top_level_reference_uses_imported_api(
    dependency: &ResolvedDependency,
    helper_file: &DartFile,
    root: Node<'_>,
    node: Node<'_>,
    source: &str,
    name: &str,
) -> bool {
    if let Some(prefix) = dependency.visibility.prefix.as_deref() {
        return identifier_uses_prefix(source, node.start_byte(), prefix)
            && !term_identifier_shadowed_at(root, node, prefix, source);
    }
    !identifier_has_prefix(source, node.start_byte())
        && !helper_declares_or_aliases_name(helper_file, name)
        && !term_identifier_shadowed_at(root, node, name, source)
}

fn member_reference_uses_imported_api(
    dependency: &ResolvedDependency,
    root: Node<'_>,
    node: Node<'_>,
    source: &str,
) -> bool {
    if let Some(prefix) = dependency.visibility.prefix.as_deref() {
        return member_reference_receiver_uses_prefix(node, source, prefix)
            && !term_identifier_shadowed_at(root, node, prefix, source);
    }
    identifier_has_prefix(source, node.start_byte())
}

fn member_reference_receiver_uses_prefix(node: Node<'_>, source: &str, prefix: &str) -> bool {
    let Some(receiver) = member_reference_receiver_text(node, source) else {
        return false;
    };
    let compact = receiver
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    compact == prefix || compact.starts_with(&format!("{prefix}."))
}

fn member_reference_receiver_text<'source>(
    node: Node<'_>,
    source: &'source str,
) -> Option<&'source str> {
    let mut current = node.parent();
    while let Some(parent) = current {
        if matches!(
            parent.kind(),
            "member_expression" | "null_aware_member_expression" | "assignable_expression"
        ) && parent
            .child_by_field_name("property")
            .is_some_and(|property| same_node(property, node))
        {
            return parent
                .child_by_field_name("object")
                .and_then(|object| object.utf8_text(source.as_bytes()).ok());
        }
        current = parent.parent();
    }
    None
}

fn reference_shadowed_at(root: Node<'_>, source: &str, name: &str, location: Location) -> bool {
    let Some(site) = reference_node_at(root, source, name, location) else {
        return false;
    };
    term_identifier_shadowed_at(root, site, name, source)
}

fn reference_is_declaration_name(
    root: Node<'_>,
    source: &str,
    name: &str,
    location: Location,
) -> bool {
    let Some(node) = reference_node_at(root, source, name, location) else {
        return false;
    };
    node.utf8_text(source.as_bytes()).ok() == Some(name) && identifier_is_declaration_name(node)
}

fn reference_node_at<'tree>(
    root: Node<'tree>,
    source: &str,
    name: &str,
    location: Location,
) -> Option<Node<'tree>> {
    let byte = byte_offset_at_location(source, location)?;
    root.descendant_for_byte_range(byte, byte + name.len())
}

fn byte_offset_at_location(source: &str, location: Location) -> Option<usize> {
    if location.line == 0 {
        return None;
    }
    let mut line = 1;
    let mut line_start = 0;
    loop {
        if line == location.line {
            let line_end = source
                .get(line_start..)?
                .find('\n')
                .map_or(source.len(), |relative| line_start + relative);
            let offset = line_start.checked_add(location.column)?;
            return (offset <= line_end).then_some(offset);
        }
        let newline = source.get(line_start..)?.find('\n')?;
        line_start += newline + 1;
        line += 1;
    }
}

fn reference_identifier_text<'source>(
    node: Node<'_>,
    source: &'source str,
) -> Option<&'source str> {
    if !matches!(node.kind(), "identifier" | "type_identifier") {
        return None;
    }
    let name = node.utf8_text(source.as_bytes()).ok()?;
    if name == "_" || has_ancestor_kind(node, &["import_or_export", "part_directive"]) {
        return None;
    }
    if identifier_is_declaration_name(node) {
        return None;
    }
    Some(name)
}

fn identifier_is_declaration_name(node: Node<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    if parent.kind() == "type_alias" && is_type_alias_name(parent, node) {
        return true;
    }
    if parent.kind() == "identifier_list" {
        return true;
    }
    DECLARATION_NAME_OWNER_KINDS.contains(&parent.kind()) && is_child_field(parent, node, "name")
}

fn identifier_uses_prefix(source: &str, identifier_start: usize, prefix: &str) -> bool {
    let Some(dot) = previous_non_whitespace(source, identifier_start) else {
        return false;
    };
    if source.as_bytes().get(dot) != Some(&b'.') {
        return false;
    }
    let Some(prefix_end) = previous_non_whitespace(source, dot) else {
        return false;
    };
    let Some(prefix_start) = (prefix_end + 1).checked_sub(prefix.len()) else {
        return false;
    };
    source
        .get(prefix_start..=prefix_end)
        .is_some_and(|candidate| candidate == prefix)
        && source
            .get(..prefix_start)
            .and_then(|before| before.chars().next_back())
            .is_none_or(|character| !is_identifier_character(character) && character != '.')
}

fn identifier_has_prefix(source: &str, identifier_start: usize) -> bool {
    previous_non_whitespace(source, identifier_start)
        .is_some_and(|index| source.as_bytes().get(index) == Some(&b'.'))
}

fn previous_non_whitespace(source: &str, before: usize) -> Option<usize> {
    source
        .get(..before)?
        .char_indices()
        .rev()
        .find_map(|(index, character)| (!character.is_whitespace()).then_some(index))
}

fn helper_declares_or_aliases_name(helper_file: &DartFile, name: &str) -> bool {
    helper_file
        .declarations
        .iter()
        .any(|declaration| declaration.name == name)
        || helper_file
            .imports
            .iter()
            .any(|import| import.prefix.as_deref() == Some(name))
}

const DECLARATION_NAME_OWNER_KINDS: &[&str] = &[
    "class_declaration",
    "constant_constructor_signature",
    "constructor_signature",
    "default_formal_parameter",
    "enum_declaration",
    "enum_constant",
    "extension_declaration",
    "extension_type_declaration",
    "factory_constructor_signature",
    "field_formal_parameter",
    "formal_parameter",
    "function_signature",
    "getter_signature",
    "initialized_identifier",
    "initialized_variable_definition",
    "mixin_declaration",
    "normal_formal_parameter",
    "operator_signature",
    "redirecting_factory_constructor_signature",
    "setter_signature",
    "static_final_declaration",
    "typed_identifier",
    "type_alias",
    "variable_pattern",
];

fn has_ancestor_kind(node: Node<'_>, kinds: &[&str]) -> bool {
    let mut parent = node.parent();
    while let Some(ancestor) = parent {
        if kinds.contains(&ancestor.kind()) {
            return true;
        }
        parent = ancestor.parent();
    }
    false
}

fn is_child_field(parent: Node<'_>, child: Node<'_>, field_name: &str) -> bool {
    let mut cursor = parent.walk();
    parent
        .children_by_field_name(field_name, &mut cursor)
        .any(|field| same_node(field, child))
}

fn is_type_alias_name(parent: Node<'_>, child: Node<'_>) -> bool {
    let mut cursor = parent.walk();
    parent
        .named_children(&mut cursor)
        .find(|node| matches!(node.kind(), "identifier" | "type_identifier"))
        .is_some_and(|name| same_node(name, child))
}

fn same_node(left: Node<'_>, right: Node<'_>) -> bool {
    left.kind() == right.kind()
        && left.start_byte() == right.start_byte()
        && left.end_byte() == right.end_byte()
}

fn is_identifier_character(character: char) -> bool {
    character == '_' || character == '$' || character.is_ascii_alphanumeric()
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
    route_files: &BTreeSet<PathBuf>,
    typed_back_edges: &[ResolvedDependency],
    residual_dependencies: &[ResolvedDependency],
    residual_reachability: &ResidualReachability,
    residual_cycles: &mut Vec<ResidualCycle>,
) {
    let mut path_edges = residual_dependencies
        .iter()
        .filter(|dependency| {
            dependency.kind != DependencyKind::Import
                || route_registry_import_edge(route_files, dependency)
        })
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

fn route_registry_import_edge(
    route_files: &BTreeSet<PathBuf>,
    dependency: &ResolvedDependency,
) -> bool {
    dependency.kind == DependencyKind::Import
        && route_files.contains(&dependency.from_path)
        && route_files.contains(&dependency.to_path)
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
