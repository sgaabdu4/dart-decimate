use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::dependencies::{LocalPubPackage, local_pub_package_from_pubspec};

use super::CodeClone;

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
