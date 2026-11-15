use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Deserialize;

#[derive(Deserialize)]
struct PackageGraphFile {
    roots: Vec<String>,
    packages: Vec<PackageGraphEntry>,
}

#[derive(Deserialize)]
struct PackageGraphEntry {
    name: String,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default, rename = "devDependencies")]
    dev_dependencies: Vec<String>,
}

pub(super) fn needed_packages(package_root: &Path, package_name: &str) -> Option<BTreeSet<String>> {
    let source = package_root
        .ancestors()
        .map(|dir| dir.join(".dart_tool/package_graph.json"))
        .find(|path| path.is_file())
        .and_then(|path| fs::read_to_string(path).ok())?;
    let graph = serde_json::from_str::<PackageGraphFile>(&source).ok()?;
    if !graph.roots.iter().any(|root| root == package_name) {
        return None;
    }

    let edges = graph
        .packages
        .iter()
        .map(|package| (package.name.as_str(), package))
        .collect::<BTreeMap<_, _>>();
    let mut needed = BTreeSet::new();
    let mut frontier = graph.roots.iter().map(String::as_str).collect::<Vec<_>>();
    while let Some(name) = frontier.pop() {
        if !needed.insert(name.to_owned()) {
            continue;
        }
        let Some(package) = edges.get(name) else {
            continue;
        };
        frontier.extend(package.dependencies.iter().map(String::as_str));
        if graph.roots.iter().any(|root| root == name) {
            frontier.extend(package.dev_dependencies.iter().map(String::as_str));
        }
    }
    Some(needed)
}
