use std::path::Path;

use crate::graph::normalize_path;
use crate::scan::{IgnoreMatcher, project_walk};

use super::{DependencyHygieneError, PubPackage, read_package};

pub(super) fn discover_packages(root: &Path) -> Result<Vec<PubPackage>, DependencyHygieneError> {
    let mut pubspecs = Vec::new();
    for entry in project_walk(root, root, &IgnoreMatcher::default()) {
        let entry = entry.map_err(|error| DependencyHygieneError::ReadDir {
            path: root.to_path_buf(),
            source: std::io::Error::other(error),
        })?;
        if entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
            && entry.file_name() == "pubspec.yaml"
        {
            pubspecs.push(normalize_path(entry.path()));
        }
    }
    let mut packages = pubspecs
        .into_iter()
        .filter_map(|path| read_package(&path).transpose())
        .collect::<Result<Vec<_>, _>>()?;
    packages.sort_by(|left, right| left.root.cmp(&right.root));
    Ok(packages)
}
