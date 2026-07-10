use std::fs;
use std::path::{Component, Path, PathBuf};

use super::CliError;
use crate::scan::ScannedProject;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EntryPointMode {
    All,
    Production,
}

pub(super) fn entry_points_for_check(
    project: &ScannedProject,
    explicit: &[PathBuf],
    mode: EntryPointMode,
) -> Vec<PathBuf> {
    if explicit.is_empty() {
        default_entry_points(project, mode)
    } else {
        explicit.to_vec()
    }
}

pub(super) fn entry_points_for_dead_code(
    project: &ScannedProject,
    explicit: &[PathBuf],
    mode: EntryPointMode,
) -> Result<Vec<PathBuf>, CliError> {
    let entries = entry_points_for_check(project, explicit, mode);
    if entries.is_empty() {
        return Err(CliError::MissingEntryPoints {
            root: project.root.clone(),
        });
    }
    Ok(entries)
}

fn default_entry_points(project: &ScannedProject, mode: EntryPointMode) -> Vec<PathBuf> {
    let roots = entry_point_roots(project);
    let patrol_suffixes = roots
        .iter()
        .map(|root| (root, patrol_test_suffix(root)))
        .collect::<Vec<_>>();
    let mut entries = project
        .files
        .iter()
        .filter(|file| {
            patrol_suffixes.iter().any(|(root, suffix)| {
                is_default_entry_point(
                    root,
                    &file.path,
                    file_has_main(file),
                    suffix.as_deref(),
                    mode,
                )
            })
        })
        .map(|file| file.path.clone())
        .collect::<Vec<_>>();
    entries.sort();
    entries.dedup();
    entries
}

fn entry_point_roots(project: &ScannedProject) -> Vec<PathBuf> {
    let mut roots = project.scan_roots.clone();
    if !roots.iter().any(|root| root == &project.root) {
        roots.push(project.root.clone());
    }
    roots.sort();
    roots.dedup();
    roots
}

fn is_default_entry_point(
    root: &Path,
    path: &Path,
    has_main: bool,
    patrol_suffix: Option<&str>,
    mode: EntryPointMode,
) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    is_public_library_entry_point(relative)
        || has_path_suffix(relative, &["lib", "main.dart"])
        || is_bin_entry_point(relative)
        || (mode == EntryPointMode::All
            && (is_direct_script_entry_point(relative)
                || is_test_entry_point(relative)
                || is_test_tree_main(relative, has_main)
                || is_patrol_entry_point(relative, patrol_suffix)))
}

fn file_has_main(file: &crate::DartFile) -> bool {
    file.declarations
        .iter()
        .any(|declaration| declaration.name == "main")
}

fn patrol_test_suffix(root: &Path) -> Option<String> {
    let source = fs::read_to_string(root.join("pubspec.yaml")).ok()?;
    let pubspec = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&source).ok()?;
    pubspec
        .get("patrol")?
        .get("test_file_suffix")?
        .as_str()
        .filter(|suffix| !suffix.is_empty())
        .map(str::to_owned)
}

fn is_public_library_entry_point(path: &Path) -> bool {
    let mut components = path.components();
    components
        .next()
        .is_some_and(|component| component.as_os_str() == "lib")
        && components
            .next()
            .is_none_or(|component| component.as_os_str() != "src")
        && path
            .extension()
            .is_some_and(|extension| extension == "dart")
}

fn is_bin_entry_point(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "dart")
        && path
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == "bin")
}

fn is_direct_script_entry_point(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "dart")
        && path.parent().and_then(Path::file_name).is_some_and(|name| {
            matches!(
                name.to_str(),
                Some("test" | "integration_test" | "test_driver" | "tool" | "scripts" | "pigeon")
            )
        })
}

fn is_test_entry_point(path: &Path) -> bool {
    let file_name = path.file_name().and_then(|name| name.to_str());
    let is_test_file = file_name.is_some_and(|name| name.ends_with("_test.dart"));
    is_test_file
        && path.components().any(|component| {
            matches!(
                component,
                Component::Normal(name)
                    if name == "test" || name == "integration_test" || name == "test_driver"
            )
        })
}

fn is_test_tree_main(path: &Path, has_main: bool) -> bool {
    has_main && has_test_component(path)
}

fn is_patrol_entry_point(path: &Path, patrol_suffix: Option<&str>) -> bool {
    let Some(suffix) = patrol_suffix else {
        return false;
    };
    has_component(path, "integration_test")
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(suffix))
}

fn has_test_component(path: &Path) -> bool {
    ["test", "integration_test", "test_driver"]
        .iter()
        .any(|name| has_component(path, name))
}

fn has_component(path: &Path, expected: &str) -> bool {
    path.components()
        .any(|component| matches!(component, Component::Normal(name) if name == expected))
}

fn has_path_suffix(path: &Path, suffix: &[&str]) -> bool {
    if suffix.is_empty() {
        return true;
    }

    let components = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>();

    components
        .windows(suffix.len())
        .last()
        .is_some_and(|window| window == suffix)
}
