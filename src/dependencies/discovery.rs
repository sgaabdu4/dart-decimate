use std::fs;
use std::path::{Path, PathBuf};

use crate::graph::normalize_path;
use crate::scan::is_repository_root;

use super::{DependencyHygieneError, PubPackage, read_package};

pub(super) fn discover_packages(root: &Path) -> Result<Vec<PubPackage>, DependencyHygieneError> {
    let mut pubspecs = Vec::new();
    discover_pubspecs(root, &mut pubspecs)?;
    let mut packages = pubspecs
        .into_iter()
        .filter_map(|path| read_package(&path).transpose())
        .collect::<Result<Vec<_>, _>>()?;
    packages.sort_by(|left, right| left.root.cmp(&right.root));
    Ok(packages)
}

fn discover_pubspecs(
    dir: &Path,
    pubspecs: &mut Vec<PathBuf>,
) -> Result<(), DependencyHygieneError> {
    let entries = fs::read_dir(dir).map_err(|source| DependencyHygieneError::ReadDir {
        path: dir.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| DependencyHygieneError::ReadDirEntry {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| DependencyHygieneError::FileType {
                path: path.clone(),
                source,
            })?;

        if file_type.is_dir() {
            if should_skip_dir(&path) || is_repository_root(&path) {
                continue;
            }
            discover_pubspecs(&path, pubspecs)?;
        } else if file_type.is_file() && path.file_name().is_some_and(|name| name == "pubspec.yaml")
        {
            pubspecs.push(normalize_path(&path));
        }
    }

    Ok(())
}

pub(super) fn should_skip_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".dart_tool" | ".git" | ".idea" | ".pub-cache" | "build" | "target")
    )
}
