use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::dependency_scripts::package_used_in_tooling;
use crate::generated::is_generated_dart_path;
use crate::graph::resolve_local_uri;
use crate::package_map::PackageMap;
use crate::{DartFile, DependencyKind, Location, extract_dart_file, scan::ScannedProject};

use super::codegen::codegen_dependencies_for_file;
use super::usage::DependencyUsage;
use super::{
    DependencyHygieneError, DependencyHygieneReport, DependencyIssue, PrivateSrcImport, PubPackage,
    UnlistedPackageDependency, UnusedPackageDependency, dependency_issue, discover_packages,
    imports_private_src, owning_package, package_name,
};

type PackageUsage = BTreeMap<PathBuf, BTreeMap<String, DependencyUsage>>;

#[derive(Default)]
struct ImportUsage {
    used_by_package: PackageUsage,
    unlisted_by_identity: BTreeMap<(PathBuf, String), UnlistedPackageDependency>,
    private_src_imports: Vec<PrivateSrcImport>,
}

/// Analyze Dart `package:` imports against pubspec dependency declarations.
///
/// # Errors
///
/// Returns [`DependencyHygieneError`] if pubspec discovery or parsing fails.
pub fn analyze_dependency_hygiene(
    project: &ScannedProject,
) -> Result<DependencyHygieneReport, DependencyHygieneError> {
    let packages = discover_packages(&project.root)?;
    let generated_runtime_files = reachable_generated_runtime_files(project, &packages)?;
    let mut imports = collect_import_usage(project, &packages, &generated_runtime_files);
    let mut unused_dependencies = unused_dependencies(&packages, &imports.used_by_package);

    sort_unused_dependencies(&mut unused_dependencies);
    sort_private_src_imports(&mut imports.private_src_imports);

    Ok(DependencyHygieneReport {
        unused_dependencies,
        misconfigured_dependency_overrides: packages
            .into_iter()
            .flat_map(|package| package.misconfigured_dependency_overrides)
            .collect(),
        unlisted_dependencies: imports.unlisted_by_identity.into_values().collect(),
        private_src_imports: imports.private_src_imports,
    })
}

fn collect_import_usage(
    project: &ScannedProject,
    packages: &[PubPackage],
    generated_runtime_files: &[DartFile],
) -> ImportUsage {
    let mut usage = ImportUsage::default();
    for file in &project.files {
        let Some(owner) = owning_package(packages, &file.path) else {
            continue;
        };
        for (specifier, kind, location) in file
            .imports
            .iter()
            .map(|import| (&import.uri, DependencyKind::Import, import.location))
            .chain(
                file.exports
                    .iter()
                    .map(|export| (&export.uri, DependencyKind::Export, export.location)),
            )
        {
            record_directive(&mut usage, owner, &file.path, specifier, kind, location);
        }
        for dependency in codegen_dependencies_for_file(file) {
            let dependency_usage = usage
                .used_by_package
                .entry(owner.root.clone())
                .or_default()
                .entry(dependency.name.to_owned())
                .or_default();
            if dependency.production {
                dependency_usage.record(&owner.root, &file.path);
            } else {
                dependency_usage.record_tooling();
            }
        }
    }
    for file in generated_runtime_files {
        let Some(owner) = owning_package(packages, &file.path) else {
            continue;
        };
        for specifier in file
            .imports
            .iter()
            .map(|import| import.uri.as_str())
            .chain(file.exports.iter().map(|export| export.uri.as_str()))
        {
            let Some(dependency) = package_name(specifier) else {
                continue;
            };
            if dependency != owner.name {
                record_dependency_usage(&mut usage, owner, &file.path, &dependency);
            }
        }
    }
    usage
}

fn reachable_generated_runtime_files(
    project: &ScannedProject,
    packages: &[PubPackage],
) -> Result<Vec<DartFile>, DependencyHygieneError> {
    let package_map = PackageMap::discover(&project.root)?;
    let mut known_paths = project
        .files
        .iter()
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    let mut frontier = project
        .files
        .iter()
        .filter(|file| {
            owning_package(packages, &file.path)
                .is_some_and(|owner| !super::allows_dev_dependency(&owner.root, &file.path))
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut generated = Vec::new();

    while let Some(file) = frontier.pop() {
        for specifier in file
            .imports
            .iter()
            .map(|import| import.uri.as_str())
            .chain(file.exports.iter().map(|export| export.uri.as_str()))
        {
            let Some(target) =
                resolve_local_uri(&project.root, &package_map, &file.path, specifier)
            else {
                continue;
            };
            if !target.local
                || !target.path.is_file()
                || !is_generated_dart_path(&target.path)
                || !known_paths.insert(target.path.clone())
            {
                continue;
            }
            let parsed = extract_dart_file(&target.path)?;
            frontier.push(parsed.clone());
            generated.push(parsed);
        }
    }
    generated.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(generated)
}

fn record_directive(
    usage: &mut ImportUsage,
    owner: &PubPackage,
    file_path: &Path,
    specifier: &str,
    kind: DependencyKind,
    location: Location,
) {
    let Some(dependency) = package_name(specifier) else {
        return;
    };
    if dependency == owner.name {
        return;
    }

    if imports_private_src(specifier) && !is_generated_dart_path(file_path) {
        usage.private_src_imports.push(PrivateSrcImport {
            package: owner.name.clone(),
            pubspec_path: owner.pubspec_path.clone(),
            path: file_path.to_path_buf(),
            dependency: dependency.clone(),
            specifier: specifier.to_owned(),
            kind,
            location,
        });
    }

    record_dependency_usage(usage, owner, file_path, &dependency);

    if !owner.declares_dependency_for_path(&dependency, file_path)
        && !is_known_generated_internal_import(file_path, specifier)
    {
        usage
            .unlisted_by_identity
            .entry((owner.root.clone(), dependency.clone()))
            .or_insert_with(|| UnlistedPackageDependency {
                package: owner.name.clone(),
                pubspec_path: owner.pubspec_path.clone(),
                path: file_path.to_path_buf(),
                dependency,
                specifier: specifier.to_owned(),
                kind,
                declared_section: owner.declared_section(specifier),
                location,
            });
    }
}

fn record_dependency_usage(
    usage: &mut ImportUsage,
    owner: &PubPackage,
    file_path: &Path,
    dependency: &str,
) {
    usage
        .used_by_package
        .entry(owner.root.clone())
        .or_default()
        .entry(dependency.to_owned())
        .or_default()
        .record(&owner.root, file_path);
}

fn is_known_generated_internal_import(file_path: &Path, specifier: &str) -> bool {
    is_generated_dart_path(file_path) && matches!(specifier, "package:slang/generated.dart")
}

fn unused_dependencies(
    packages: &[PubPackage],
    used_by_package: &PackageUsage,
) -> Vec<UnusedPackageDependency> {
    let mut unused_dependencies = Vec::new();
    for package in packages {
        let used = used_by_package.get(&package.root);
        for dependency in &package.dependencies {
            if let Some(unused) = unused_dependency(package, dependency, used) {
                unused_dependencies.push(unused);
            }
        }
    }
    unused_dependencies
}

fn unused_dependency(
    package: &PubPackage,
    dependency: &super::DeclaredDependency,
    used: Option<&BTreeMap<String, DependencyUsage>>,
) -> Option<UnusedPackageDependency> {
    if dependency.name == package.name {
        return None;
    }
    let mut usage = used
        .and_then(|used| used.get(&dependency.name))
        .copied()
        .unwrap_or_default();
    if package_used_in_tooling(&package.root, &dependency.name) {
        usage.record_tooling();
    }
    let usage = usage.any().then_some(usage);
    let issue = dependency_issue(dependency, usage.as_ref(), package.locked_packages.as_ref())?;
    Some(UnusedPackageDependency {
        package: package.name.clone(),
        pubspec_path: dependency.source_path.clone(),
        dependency: dependency.name.clone(),
        section: dependency.section,
        issue,
        location: dependency.location,
        safe_to_delete: dependency.safe_to_delete
            && matches!(
                issue,
                DependencyIssue::UnusedRuntimeDependency | DependencyIssue::UnusedDevDependency
            ),
    })
}

fn sort_unused_dependencies(unused_dependencies: &mut [UnusedPackageDependency]) {
    unused_dependencies.sort_by(|left, right| {
        (
            &left.package,
            left.section.as_pubspec_key(),
            &left.dependency,
        )
            .cmp(&(
                &right.package,
                right.section.as_pubspec_key(),
                &right.dependency,
            ))
    });
}

fn sort_private_src_imports(private_src_imports: &mut [PrivateSrcImport]) {
    private_src_imports.sort_by(|left, right| {
        (
            &left.package,
            &left.path,
            left.location.line,
            &left.dependency,
            &left.specifier,
        )
            .cmp(&(
                &right.package,
                &right.path,
                right.location.line,
                &right.dependency,
                &right.specifier,
            ))
    });
}
