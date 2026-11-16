use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use crate::graph::{normalize_path, resolve_local_uri};
use crate::package_map::{PackageMap, parse_yaml};

mod loader;
mod resolver;
pub(crate) use loader::SharedLoader;
pub(crate) use resolver::OutsideTypes;
#[cfg(test)]
mod loader_tests;
#[cfg(test)]
mod tests;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Target {
    Outside(PathBuf),
    Local,
    Unknown,
}

pub(crate) struct OutsideSources {
    root: PathBuf,
    packages: PackageMap,
    dart_libraries: BTreeMap<String, PathBuf>,
    sdk_lib: PathBuf,
}

impl OutsideSources {
    pub(crate) fn discover(root: &Path) -> Option<Self> {
        Self::discover_with_path(root, std::env::var_os("PATH").as_deref())
    }

    fn discover_with_path(root: &Path, path_var: Option<&OsStr>) -> Option<Self> {
        let packages = PackageMap::discover(root).ok()?;
        let toolchain = packages.toolchain()?;
        let flutter_sdk = toolchain
            .flutter_root
            .as_ref()
            .map(|flutter| flutter.join("bin/cache/dart-sdk"));
        let version = toolchain.generator_version.as_deref();
        let (dart_libraries, sdk_lib) =
            if let Some(sky_engine) = packages.package_path("sky_engine") {
                if flutter_sdk
                    .as_ref()
                    .is_some_and(|sdk| !sdk_matches(sdk, version))
                {
                    return None;
                }
                (embedder_libraries(sky_engine)?, sky_engine.to_path_buf())
            } else {
                let sdk = flutter_sdk.or_else(|| sdk_on_path(path_var))?;
                if !sdk_matches(&sdk, version) {
                    return None;
                }
                (sdk_libraries(&sdk)?, sdk.join("lib"))
            };
        Some(Self {
            root: root.to_path_buf(),
            packages,
            dart_libraries,
            sdk_lib,
        })
    }

    pub(crate) fn is_sdk_source(&self, path: &Path) -> bool {
        path.starts_with(&self.sdk_lib)
    }

    pub(crate) fn resolve(&self, from: &Path, specifier: &str, from_outside: bool) -> Target {
        if let Some(library) = specifier.strip_prefix("dart:") {
            let name = library.split('/').next().unwrap_or(library);
            return self
                .dart_libraries
                .get(name)
                .map_or(Target::Unknown, |path| Target::Outside(path.clone()));
        }
        match resolve_local_uri(&self.root, &self.packages, from, specifier) {
            None => Target::Unknown,
            Some(target) if target.local && !from_outside => Target::Local,
            Some(target) => Target::Outside(target.path),
        }
    }
}

fn sdk_matches(sdk: &Path, generator_version: Option<&str>) -> bool {
    generator_version.is_some_and(|expected| {
        fs::read_to_string(sdk.join("version")).is_ok_and(|version| version.trim() == expected)
    })
}

fn sdk_on_path(path_var: Option<&OsStr>) -> Option<PathBuf> {
    std::env::split_paths(path_var?)
        .map(|directory| directory.join("dart"))
        .find_map(|binary| {
            let binary = fs::canonicalize(binary).ok()?;
            binary
                .parent()?
                .parent()
                .filter(|sdk| sdk.join("lib/libraries.json").is_file())
                .map(Path::to_path_buf)
        })
}

fn embedder_libraries(sky_engine: &Path) -> Option<BTreeMap<String, PathBuf>> {
    let source = fs::read_to_string(sky_engine.join("_embedder.yaml")).ok()?;
    let document = parse_yaml(&source).ok()?;
    let libraries = document.as_mapping()?.get("embedded_libs")?.as_mapping()?;
    Some(
        libraries
            .iter()
            .filter_map(|(name, path)| {
                let name = name.strip_prefix("dart:")?;
                Some((
                    name.to_owned(),
                    normalize_path(&sky_engine.join(path.as_str()?)),
                ))
            })
            .collect(),
    )
}

fn sdk_libraries(sdk: &Path) -> Option<BTreeMap<String, PathBuf>> {
    let source = fs::read_to_string(sdk.join("lib/libraries.json")).ok()?;
    let document = serde_json::from_str::<serde_json::Value>(&source).ok()?;
    let mut libraries = BTreeMap::new();
    for target in ["vm_common", "vm"] {
        let entries = document.get(target)?.get("libraries")?.as_object()?;
        for (name, entry) in entries {
            let uri = entry.get("uri")?.as_str()?;
            libraries.insert(name.clone(), normalize_path(&sdk.join("lib").join(uri)));
        }
    }
    Some(libraries)
}
