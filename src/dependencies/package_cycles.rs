use std::collections::BTreeMap;
use std::path::PathBuf;

use petgraph::algo::tarjan_scc;
use petgraph::graph::DiGraph;
use serde::{Deserialize, Serialize};

use super::{DependencySection, PubPackage};

/// Local pub packages that depend on each other through pubspec dependency sections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageCycle {
    /// Package names in the cycle, sorted for deterministic output.
    pub packages: Vec<String>,
    /// Pubspec paths of those packages, in the same order.
    pub pubspec_paths: Vec<PathBuf>,
}

pub(super) fn package_cycles(packages: &[PubPackage]) -> Vec<PackageCycle> {
    let mut graph = DiGraph::<usize, ()>::new();
    let nodes = (0..packages.len())
        .map(|index| graph.add_node(index))
        .collect::<Vec<_>>();
    let mut by_name = BTreeMap::new();
    for (index, package) in packages.iter().enumerate() {
        by_name.entry(package.name.as_str()).or_insert(index);
    }

    for (index, package) in packages.iter().enumerate() {
        for dependency in &package.dependencies {
            let Some(&target) = by_name.get(dependency.name.as_str()) else {
                continue;
            };
            if target != index && dependency.section != DependencySection::DependencyOverrides {
                graph.update_edge(nodes[index], nodes[target], ());
            }
        }
    }

    let mut cycles = tarjan_scc(&graph)
        .into_iter()
        .filter(|component| component.len() > 1)
        .map(|component| {
            let mut members = component
                .into_iter()
                .map(|node| &packages[graph[node]])
                .collect::<Vec<_>>();
            members.sort_by(|left, right| {
                (&left.name, &left.pubspec_path).cmp(&(&right.name, &right.pubspec_path))
            });
            PackageCycle {
                packages: members.iter().map(|package| package.name.clone()).collect(),
                pubspec_paths: members
                    .iter()
                    .map(|package| package.pubspec_path.clone())
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    cycles.sort_by(|left, right| left.packages.cmp(&right.packages));
    cycles
}
