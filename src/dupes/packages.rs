use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::dependencies::{LocalPubPackage, local_pub_package_from_pubspec};

use super::{CodeClone, CodeCloneInstance};

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

        let mut roots_by_identity = BTreeMap::<(String, PathBuf), BTreeSet<PathBuf>>::new();
        for instance in &group.instances {
            let Some(package) = self.owning_package(&instance.path) else {
                continue;
            };
            let Ok(relative_path) = instance.path.strip_prefix(&package.root) else {
                continue;
            };
            roots_by_identity
                .entry((package.name, relative_path.to_path_buf()))
                .or_default()
                .insert(package.root);
        }

        let canonical_roots = roots_by_identity
            .into_iter()
            .filter(|(_, roots)| roots.len() > 1)
            .filter_map(|(identity, roots)| {
                roots
                    .into_iter()
                    .min_by(package_root_order)
                    .map(|root| (identity, root))
            })
            .collect::<BTreeMap<_, _>>();
        if canonical_roots.is_empty() {
            return;
        }

        let mut seen = BTreeSet::<(PathBuf, usize, usize, usize)>::new();
        let mut instances = Vec::new();
        for instance in &group.instances {
            let canonical = self.canonical_instance(instance, &canonical_roots);
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

    fn canonical_instance(
        &mut self,
        instance: &CodeCloneInstance,
        canonical_roots: &BTreeMap<(String, PathBuf), PathBuf>,
    ) -> CodeCloneInstance {
        let Some(package) = self.owning_package(&instance.path) else {
            return instance.clone();
        };
        let Ok(relative_path) = instance.path.strip_prefix(&package.root) else {
            return instance.clone();
        };
        let identity = (package.name, relative_path.to_path_buf());
        let Some(canonical_root) = canonical_roots.get(&identity) else {
            return instance.clone();
        };
        let mut canonical = instance.clone();
        canonical.path = canonical_root.join(relative_path);
        canonical
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
