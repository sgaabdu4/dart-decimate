use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::dependencies::{LocalPubPackage, local_pub_package_from_pubspec};

use super::{CodeClone, CodeCloneInstance};

type PackageIdentity = (String, PathBuf);
type InstanceKey = (PathBuf, usize, usize, usize);
type OccurrenceLayout = Vec<(usize, usize, usize)>;

pub(super) struct CopiedPackageFilter {
    root: PathBuf,
    package_cache: BTreeMap<PathBuf, Option<LocalPubPackage>>,
}

impl CopiedPackageFilter {
    pub(super) fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            package_cache: BTreeMap::new(),
        }
    }

    pub(super) fn is_copied_package_clone(&mut self, group: &CodeClone) -> bool {
        if group.instances.len() < 2 {
            return false;
        }
        let mut roots = BTreeSet::new();
        let mut identity = None;
        for instance in &group.instances {
            let Some(package) = self.owning_package(&instance.path) else {
                return false;
            };
            let Ok(relative_path) = instance.path.strip_prefix(&package.root) else {
                return false;
            };
            roots.insert(package.root.clone());
            let current = (
                package.name.clone(),
                relative_path.to_path_buf(),
                instance.start_line,
                instance.end_line,
                instance.column,
            );
            if identity.is_none() {
                identity = Some(current);
            } else if identity != Some(current) {
                return false;
            }
        }
        roots.len() > 1
    }

    pub(super) fn canonicalize_copied_package_instances(&mut self, group: &mut CodeClone) {
        if group.instances.len() < 2 {
            return;
        }

        let mut instances_by_identity =
            BTreeMap::<PackageIdentity, BTreeMap<PathBuf, Vec<CodeCloneInstance>>>::new();
        for instance in &group.instances {
            let Some(package) = self.owning_package(&instance.path) else {
                continue;
            };
            let Ok(relative_path) = instance.path.strip_prefix(&package.root) else {
                continue;
            };
            instances_by_identity
                .entry((package.name, relative_path.to_path_buf()))
                .or_default()
                .entry(package.root)
                .or_default()
                .push(instance.clone());
        }

        let canonical_instances = canonical_instances_by_occurrence(instances_by_identity);
        if canonical_instances.is_empty() {
            return;
        }

        let mut seen = BTreeSet::<(PathBuf, usize, usize, usize)>::new();
        let mut instances = Vec::new();
        for instance in &group.instances {
            let canonical = canonical_instances
                .get(&instance_key(instance))
                .cloned()
                .unwrap_or_else(|| instance.clone());
            if seen.insert((
                canonical.path.clone(),
                canonical.start_line,
                canonical.end_line,
                canonical.column,
            )) {
                instances.push(canonical);
            }
        }
        instances.sort_by(|left, right| {
            (&left.path, left.start_line, left.end_line).cmp(&(
                &right.path,
                right.start_line,
                right.end_line,
            ))
        });
        group.instances = instances;
    }

    fn owning_package(&mut self, path: &Path) -> Option<LocalPubPackage> {
        let mut current = path.parent();
        while let Some(dir) = current {
            if !dir.starts_with(&self.root) {
                break;
            }
            let pubspec_path = dir.join("pubspec.yaml");
            if pubspec_path.is_file() {
                return self
                    .package_cache
                    .entry(pubspec_path.clone())
                    .or_insert_with(|| local_pub_package_from_pubspec(&pubspec_path).ok().flatten())
                    .clone();
            }
            if dir == self.root {
                break;
            }
            current = dir.parent();
        }
        None
    }
}

fn package_root_order(left: &PathBuf, right: &PathBuf) -> std::cmp::Ordering {
    let left_components = left.components().count();
    let right_components = right.components().count();
    (left_components, left).cmp(&(right_components, right))
}

fn canonical_instances_by_occurrence(
    instances_by_identity: BTreeMap<PackageIdentity, BTreeMap<PathBuf, Vec<CodeCloneInstance>>>,
) -> BTreeMap<InstanceKey, CodeCloneInstance> {
    let mut canonical_instances = BTreeMap::new();
    for roots in instances_by_identity.into_values() {
        let mut roots_by_layout =
            BTreeMap::<OccurrenceLayout, BTreeMap<PathBuf, Vec<CodeCloneInstance>>>::new();
        for (root, root_instances) in roots {
            roots_by_layout
                .entry(occurrence_layout(&root_instances))
                .or_default()
                .insert(root, root_instances);
        }
        for roots in roots_by_layout.into_values() {
            if roots.len() < 2 {
                continue;
            }
            let Some((canonical_root, canonical_root_instances)) = roots
                .iter()
                .min_by(|left, right| package_root_order(left.0, right.0))
            else {
                continue;
            };
            let canonical_root_instances = sorted_instances(canonical_root_instances);
            for (root, root_instances) in &roots {
                if root == canonical_root {
                    continue;
                }
                for (index, instance) in sorted_instances(root_instances).into_iter().enumerate() {
                    let Some(canonical) = canonical_root_instances.get(index) else {
                        continue;
                    };
                    canonical_instances.insert(instance_key(&instance), canonical.clone());
                }
            }
        }
    }
    canonical_instances
}

fn occurrence_layout(instances: &[CodeCloneInstance]) -> OccurrenceLayout {
    let instances = sorted_instances(instances);
    let first_start = instances.first().map_or(0, |instance| instance.start_line);
    instances
        .iter()
        .map(|instance| {
            (
                instance.start_line.saturating_sub(first_start),
                instance.end_line.saturating_sub(instance.start_line),
                instance.column,
            )
        })
        .collect()
}

fn sorted_instances(instances: &[CodeCloneInstance]) -> Vec<CodeCloneInstance> {
    let mut instances = instances.to_vec();
    instances.sort_by(|left, right| {
        (left.start_line, left.end_line, left.column, &left.path).cmp(&(
            right.start_line,
            right.end_line,
            right.column,
            &right.path,
        ))
    });
    instances
}

fn instance_key(instance: &CodeCloneInstance) -> InstanceKey {
    (
        instance.path.clone(),
        instance.start_line,
        instance.end_line,
        instance.column,
    )
}
