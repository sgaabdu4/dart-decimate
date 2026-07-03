use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_yaml_ng::{Mapping, Value};

use crate::graph::{normalize_path, GraphError};

#[derive(Debug, Clone, Default)]
pub(crate) struct PackageMap {
    by_name: BTreeMap<String, Vec<PackageRoot>>,
    by_owner_dependency: BTreeMap<PathBuf, BTreeMap<String, PackageRoot>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageRoot {
    name: String,
    root: PathBuf,
    package_path: PathBuf,
    local: bool,
    from_package_config: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageResolution {
    pub(crate) path: PathBuf,
    pub(crate) local: bool,
}

impl PackageMap {
    pub(crate) fn discover(root: &Path) -> Result<Self, GraphError> {
        if let Some(config) = PackageConfigFile::read(root)? {
            let mut packages = Self::from_package_config(root, &config.config_dir, &config.config);
            let mut visited = BTreeSet::new();
            packages.discover_pubspec(root, &mut visited)?;
            packages.discover_nested_pubspecs(root, &mut visited)?;
            return Ok(packages);
        }

        let mut packages = Self::default();
        let mut visited = BTreeSet::new();
        packages.discover_pubspec(root, &mut visited)?;
        packages.discover_nested_pubspecs(root, &mut visited)?;
        Ok(packages)
    }

    pub(crate) fn names(&self) -> Vec<&str> {
        self.by_name.keys().map(String::as_str).collect()
    }

    pub(crate) fn local_roots(&self) -> BTreeSet<PathBuf> {
        self.by_name
            .values()
            .flatten()
            .filter(|package| package.local)
            .map(|package| package.root.clone())
            .collect()
    }

    pub(crate) fn resolve_from(
        &self,
        from_path: &Path,
        package: &str,
        path: &str,
    ) -> Option<PackageResolution> {
        let roots = self.by_name.get(package)?;
        let owner_package = self.owner_package(from_path);
        let owner_root = owner_package.map(|package| package.root.as_path());
        if let Some(owner_package) = owner_package {
            if owner_package.name == package {
                return Some(package_resolution(owner_package, path));
            }
        }
        let configured_root = roots.iter().find(|root| root.from_package_config);
        if owner_package.is_some_and(|package| package.from_package_config) {
            if let Some(root) = configured_root {
                return Some(package_resolution(root, path));
            }
        }
        if let Some(owner_root) = owner_root {
            if let Some(root) = self
                .by_owner_dependency
                .get(owner_root)
                .and_then(|dependencies| dependencies.get(package))
            {
                return Some(package_resolution(root, path));
            }
        }
        if let Some(root) = configured_root {
            return Some(package_resolution(root, path));
        }
        if roots.len() == 1 {
            return roots.first().map(|root| package_resolution(root, path));
        }

        owner_root
            .and_then(|owner_root| {
                roots
                    .iter()
                    .filter(|candidate| candidate.root.starts_with(owner_root))
                    .min_by_key(|candidate| candidate.root.components().count())
            })
            .or_else(|| roots.first())
            .map(|root| package_resolution(root, path))
    }

    fn owner_package(&self, path: &Path) -> Option<&PackageRoot> {
        self.by_name
            .values()
            .flatten()
            .filter(|package| path.starts_with(&package.root))
            .max_by_key(|package| package.root.components().count())
    }

    fn from_package_config(root: &Path, config_dir: &Path, config: &PackageConfig) -> Self {
        let config_root = config_dir.parent().unwrap_or(config_dir);
        let by_name = config
            .packages
            .iter()
            .filter_map(|package| {
                let root_uri = package.root_uri.as_deref()?;
                let package_uri = package.package_uri.as_deref().unwrap_or("lib/");
                let package_root = resolve_config_uri(config_dir, root_uri)?;
                let package_path = resolve_package_uri(&package_root, package_uri)?;
                let local =
                    is_local_package_config_root(root, config_root, root_uri, &package_root);
                Some((
                    package.name.clone(),
                    vec![PackageRoot {
                        name: package.name.clone(),
                        root: package_root,
                        package_path,
                        local,
                        from_package_config: true,
                    }],
                ))
            })
            .collect();

        Self {
            by_name,
            by_owner_dependency: BTreeMap::new(),
        }
    }

    fn discover_pubspec(
        &mut self,
        package_root: &Path,
        visited: &mut BTreeSet<PathBuf>,
    ) -> Result<(), GraphError> {
        let package_root = normalize_path(package_root);
        if !visited.insert(package_root.clone()) {
            return Ok(());
        }

        let Some(pubspec) = Pubspec::read(&package_root)? else {
            return Ok(());
        };

        if let Some(name) = pubspec.name {
            self.insert(name, &package_root, PathBuf::from("lib"));
        }

        for dependency in pubspec.path_dependencies {
            let dependency_root = normalize_path(&package_root.join(dependency.path));
            if !dependency_root.join("pubspec.yaml").is_file() {
                continue;
            }
            let package = self.insert(
                dependency.name.clone(),
                &dependency_root,
                PathBuf::from("lib"),
            );
            self.by_owner_dependency
                .entry(package_root.clone())
                .or_default()
                .insert(dependency.name, package);
            self.discover_pubspec(&dependency_root, visited)?;
        }

        for member in pubspec.workspace_members {
            for member_root in expand_workspace_member(&package_root, &member)? {
                self.discover_pubspec(&member_root, visited)?;
            }
        }

        Ok(())
    }

    fn discover_nested_pubspecs(
        &mut self,
        dir: &Path,
        visited: &mut BTreeSet<PathBuf>,
    ) -> Result<(), GraphError> {
        let entries = fs::read_dir(dir).map_err(|source| GraphError::ReadDir {
            path: dir.to_path_buf(),
            source,
        })?;

        for entry in entries {
            let entry = entry.map_err(|source| GraphError::ReadDirEntry {
                path: dir.to_path_buf(),
                source,
            })?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|source| GraphError::FileType {
                path: path.clone(),
                source,
            })?;

            if file_type.is_dir() {
                if should_skip_dir(&path) {
                    continue;
                }
                self.discover_nested_pubspecs(&path, visited)?;
            } else if file_type.is_file()
                && path.file_name().is_some_and(|name| name == "pubspec.yaml")
            {
                if let Some(package_root) = path.parent() {
                    self.discover_pubspec(package_root, visited)?;
                }
            }
        }

        Ok(())
    }

    fn insert(&mut self, name: String, root: &Path, package_uri: PathBuf) -> PackageRoot {
        let packages = self.by_name.entry(name.clone()).or_default();
        let root = normalize_path(root);
        if let Some(package) = packages.iter().find(|package| package.root == root) {
            return package.clone();
        }
        let package = PackageRoot {
            name,
            package_path: normalize_path(&root.join(package_uri)),
            root,
            local: true,
            from_package_config: false,
        };
        packages.push(package.clone());
        packages.sort_by(|left, right| left.root.cmp(&right.root));
        package
    }
}

fn package_resolution(root: &PackageRoot, path: &str) -> PackageResolution {
    PackageResolution {
        path: normalize_path(&root.package_path.join(path)),
        local: root.local,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct PackageConfig {
    #[serde(default)]
    packages: Vec<PackageConfigPackage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageConfigFile {
    config_dir: PathBuf,
    config: PackageConfig,
}

impl PackageConfigFile {
    fn read(root: &Path) -> Result<Option<Self>, GraphError> {
        let Some(path) = find_package_config(root) else {
            return Ok(None);
        };
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(source) => return Err(GraphError::ReadPackageConfig { path, source }),
        };

        let config =
            serde_json::from_str(&source).map_err(|source| GraphError::ParsePackageConfig {
                path: path.clone(),
                source,
            })?;
        let config_dir = path
            .parent()
            .map_or_else(|| root.join(".dart_tool"), Path::to_path_buf);
        Ok(Some(Self { config_dir, config }))
    }
}

fn find_package_config(root: &Path) -> Option<PathBuf> {
    root.ancestors()
        .map(|dir| dir.join(".dart_tool/package_config.json"))
        .find(|path| path.is_file())
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageConfigPackage {
    name: String,
    root_uri: Option<String>,
    package_uri: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PathDependency {
    name: String,
    path: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Pubspec {
    name: Option<String>,
    workspace_members: Vec<String>,
    path_dependencies: Vec<PathDependency>,
}

impl Pubspec {
    fn read(package_root: &Path) -> Result<Option<Self>, GraphError> {
        let path = package_root.join("pubspec.yaml");
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(GraphError::ReadPubspec { path, source }),
        };
        let value = serde_yaml_ng::from_str::<Value>(&source).map_err(|source| {
            GraphError::ParsePubspec {
                path: path.clone(),
                source,
            }
        })?;

        let overrides = read_pubspec_overrides(package_root)?;
        Ok(Some(Self::from_values(&value, overrides.as_ref())))
    }

    fn from_values(value: &Value, overrides: Option<&Value>) -> Self {
        let workspace_source = overrides
            .filter(|value| has_top_level_field(value, "workspace"))
            .unwrap_or(value);
        Self {
            name: string_field(value, "name").map(str::to_owned),
            workspace_members: string_sequence_field(workspace_source, "workspace"),
            path_dependencies: path_dependencies(value, overrides),
        }
    }
}

fn read_pubspec_overrides(package_root: &Path) -> Result<Option<Value>, GraphError> {
    let path = package_root.join("pubspec_overrides.yaml");
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(GraphError::ReadPubspec { path, source }),
    };
    serde_yaml_ng::from_str::<Value>(&source)
        .map(Some)
        .map_err(|source| GraphError::ParsePubspec { path, source })
}

fn resolve_config_uri(config_dir: &Path, uri: &str) -> Option<PathBuf> {
    if let Some(path) = uri.strip_prefix("file://") {
        let path = percent_decode(path)?;
        return Some(normalize_path(Path::new(&path)));
    }
    if uri.contains(':') {
        return None;
    }

    let uri = percent_decode(uri)?;
    Some(normalize_path(&config_dir.join(uri)))
}

fn resolve_package_uri(package_root: &Path, uri: &str) -> Option<PathBuf> {
    if let Some(path) = uri.strip_prefix("file://") {
        let path = percent_decode(path)?;
        return Some(normalize_path(Path::new(&path)));
    }
    if uri.contains(':') {
        return None;
    }

    let uri = percent_decode(uri)?;
    Some(normalize_path(&package_root.join(uri)))
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex_value(*bytes.get(index + 1)?)?;
            let low = hex_value(*bytes.get(index + 2)?)?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(decoded).ok()
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn is_local_package_config_root(
    root: &Path,
    config_root: &Path,
    root_uri: &str,
    package_root: &Path,
) -> bool {
    (is_relative_uri(root_uri)
        || package_root.starts_with(root)
        || package_root.starts_with(config_root))
        && !is_pub_cache_path(package_root)
}

fn is_relative_uri(uri: &str) -> bool {
    !uri.contains(':')
}

fn is_pub_cache_path(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str() == ".pub-cache")
}

fn path_dependencies(value: &Value, overrides: Option<&Value>) -> Vec<PathDependency> {
    let mut dependencies = ["dependencies", "dev_dependencies"]
        .into_iter()
        .filter_map(|section| mapping_field(value, section))
        .flat_map(path_dependencies_from_mapping)
        .collect::<Vec<_>>();

    let override_source = overrides
        .filter(|value| has_top_level_field(value, "dependency_overrides"))
        .unwrap_or(value);
    if let Some(overrides) = mapping_field(override_source, "dependency_overrides") {
        let override_names = overrides
            .keys()
            .filter_map(Value::as_str)
            .collect::<BTreeSet<_>>();
        dependencies.retain(|dependency| !override_names.contains(dependency.name.as_str()));
    }
    dependencies.extend(
        mapping_field(override_source, "dependency_overrides")
            .into_iter()
            .flat_map(path_dependencies_from_mapping),
    );
    dependencies
}

fn path_dependencies_from_mapping(mapping: &Mapping) -> Vec<PathDependency> {
    mapping
        .iter()
        .filter_map(|(name, dependency)| {
            let name = name.as_str()?.to_owned();
            let path = string_field(dependency, "path")?;
            Some(PathDependency {
                name,
                path: PathBuf::from(path),
            })
        })
        .collect()
}

fn mapping_field<'value>(value: &'value Value, field: &str) -> Option<&'value Mapping> {
    value
        .as_mapping()?
        .get(Value::String(field.to_owned()))?
        .as_mapping()
}

fn string_field<'value>(value: &'value Value, field: &str) -> Option<&'value str> {
    value
        .as_mapping()?
        .get(Value::String(field.to_owned()))?
        .as_str()
}

fn string_sequence_field(value: &Value, field: &str) -> Vec<String> {
    value
        .as_mapping()
        .and_then(|mapping| mapping.get(Value::String(field.to_owned())))
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn has_top_level_field(value: &Value, field: &str) -> bool {
    value
        .as_mapping()
        .is_some_and(|mapping| mapping.contains_key(Value::String(field.to_owned())))
}

fn expand_workspace_member(root: &Path, member: &str) -> Result<Vec<PathBuf>, GraphError> {
    if !contains_glob_pattern(member) {
        return Ok(vec![normalize_path(&root.join(member))]);
    }

    let pattern_path = root.join(member).join("pubspec.yaml");
    let pattern = pattern_path.to_string_lossy().into_owned();
    let entries = glob::glob(&pattern).map_err(|source| GraphError::WorkspacePattern {
        pattern: pattern.clone(),
        source,
    })?;

    entries
        .map(|entry| {
            let pubspec_path = entry.map_err(|source| GraphError::WorkspaceGlob {
                pattern: pattern.clone(),
                source,
            })?;
            Ok(pubspec_path
                .parent()
                .map_or_else(|| normalize_path(root), normalize_path))
        })
        .collect()
}

fn contains_glob_pattern(value: &str) -> bool {
    value.contains('*') || value.contains('?') || value.contains('[')
}

fn should_skip_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".dart_tool" | ".git" | ".idea" | ".pub-cache" | "build" | "target")
    )
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn skips_missing_path_dependencies_from_nested_pubspec_overrides(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixture = TempDir::new()?;
        write(&fixture, "pubspec.yaml", "name: hydrated_bloc\n")?;
        write(
            &fixture,
            "example/pubspec.yaml",
            "name: example\ndependencies:\n  hydrated_bloc: ^10.0.0\n",
        )?;
        write(
            &fixture,
            "example/pubspec_overrides.yaml",
            "dependency_overrides:\n  bloc:\n    path: ../../bloc\n",
        )?;

        let packages = PackageMap::discover(fixture.path())?;

        assert_eq!(packages.names(), vec!["example", "hydrated_bloc"]);
        assert!(packages
            .resolve_from(
                &fixture.path().join("lib/hydrated_bloc.dart"),
                "bloc",
                "bloc.dart"
            )
            .is_none());

        Ok(())
    }

    fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
        let path = fixture.path().join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, source)
    }
}
