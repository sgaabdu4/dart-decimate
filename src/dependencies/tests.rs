use std::fs;
use std::path::Path;

use tempfile::TempDir;

use super::*;
use crate::scan_project;

#[test]
fn parses_pubspec_dependency_keys_from_source() {
    let dependencies = declared_dependencies_from_source(
        "name: app\n\
dependencies:\n  http: ^1.0.0\n  path: ^1.0.0\n\
dev_dependencies:\n  test: ^1.0.0\n",
        Path::new("pubspec.yaml"),
    );

    assert_eq!(
        dependencies
            .iter()
            .map(|dependency| (
                dependency.name.as_str(),
                dependency.section,
                dependency.location.line,
            ))
            .collect::<Vec<_>>(),
        vec![
            ("http", DependencySection::Dependencies, 3),
            ("path", DependencySection::Dependencies, 4),
            ("test", DependencySection::DevDependencies, 6),
        ]
    );
}

#[test]
fn reports_unused_and_unlisted_pub_dependencies() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  http: ^1.0.0\n  path: ^1.0.0\n\
dev_dependencies:\n  test: ^1.0.0\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:http/http.dart';\n\
import 'package:collection/collection.dart';\n\
void main() {}\n",
    )?;
    let project = scan_project(fixture.path())?;
    let packages = discover_packages(&project.root)?;
    assert_eq!(packages.len(), 1);
    assert_eq!(packages[0].name, "app");
    assert_eq!(packages[0].dependencies.len(), 3);

    let report = analyze_dependency_hygiene(&project)?;

    assert_eq!(
        report
            .unused_dependencies
            .iter()
            .map(|dependency| (
                dependency.dependency.as_str(),
                dependency.section,
                dependency.issue,
                dependency.location.line,
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "path",
                DependencySection::Dependencies,
                DependencyIssue::UnusedRuntimeDependency,
                4,
            ),
            (
                "test",
                DependencySection::DevDependencies,
                DependencyIssue::UnusedDevDependency,
                6,
            ),
        ]
    );
    assert_eq!(report.unlisted_dependencies.len(), 1);
    assert_eq!(report.unlisted_dependencies[0].dependency, "collection");
    assert_eq!(report.unlisted_dependencies[0].location.line, 2);

    Ok(())
}

#[test]
fn reports_dev_dependency_used_from_lib_as_production_usage()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dev_dependencies:\n  collection: ^1.0.0\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:collection/collection.dart';\nvoid main() {}\n",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_dependency_hygiene(&project)?;

    assert!(report.unlisted_dependencies.is_empty());
    assert_eq!(report.unused_dependencies.len(), 1);
    assert_eq!(report.unused_dependencies[0].dependency, "collection");
    assert_eq!(
        report.unused_dependencies[0].issue,
        DependencyIssue::DevDependencyInProduction
    );
    assert!(!report.unused_dependencies[0].safe_to_delete);

    Ok(())
}

#[test]
fn reports_dev_dependency_used_from_bin_as_production_usage()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dev_dependencies:\n  args: ^2.0.0\n",
    )?;
    write(
        &fixture,
        "bin/app.dart",
        "import 'package:args/args.dart';\nvoid main() {}\n",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_dependency_hygiene(&project)?;

    assert!(report.unlisted_dependencies.is_empty());
    assert_eq!(report.unused_dependencies.len(), 1);
    assert_eq!(
        report.unused_dependencies[0].issue,
        DependencyIssue::DevDependencyInProduction
    );

    Ok(())
}

#[test]
fn reports_dev_dependencies_used_from_dart_runtime_entrypoints()
-> Result<(), Box<dyn std::error::Error>> {
    for (path, dependency) in [
        ("web/main.dart", "web"),
        ("hook/build.dart", "hooks"),
        ("hook/link.dart", "native_toolchain_c"),
    ] {
        let fixture = tempfile::tempdir()?;
        write(
            &fixture,
            "pubspec.yaml",
            &format!("name: app\ndev_dependencies:\n  {dependency}: ^1.0.0\n"),
        )?;
        write(
            &fixture,
            path,
            &format!("import 'package:{dependency}/{dependency}.dart';\nvoid main() {{}}\n"),
        )?;
        let project = scan_project(fixture.path())?;

        let report = analyze_dependency_hygiene(&project)?;

        assert!(report.unlisted_dependencies.is_empty(), "{path}");
        assert_eq!(report.unused_dependencies.len(), 1, "{path}");
        assert_eq!(
            report.unused_dependencies[0].issue,
            DependencyIssue::DevDependencyInProduction,
            "{path}"
        );
    }

    Ok(())
}

#[test]
fn allows_test_imports_from_dev_dependencies() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dev_dependencies:\n  test: ^1.0.0\n",
    )?;
    write(
        &fixture,
        "test/app_test.dart",
        "import 'package:test/test.dart';\nvoid main() {}\n",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_dependency_hygiene(&project)?;

    assert!(report.unlisted_dependencies.is_empty());
    assert!(report.unused_dependencies.is_empty());

    Ok(())
}

#[test]
fn reports_runtime_dependency_used_only_from_tests() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  test: ^1.0.0\n",
    )?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(
        &fixture,
        "test/app_test.dart",
        "import 'package:test/test.dart';\nvoid main() {}\n",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_dependency_hygiene(&project)?;

    assert_eq!(report.unused_dependencies.len(), 1);
    assert_eq!(report.unused_dependencies[0].dependency, "test");
    assert_eq!(
        report.unused_dependencies[0].issue,
        DependencyIssue::TestOnlyDependency
    );
    assert_eq!(
        report.unused_dependencies[0].section,
        DependencySection::Dependencies
    );

    Ok(())
}

#[test]
fn reports_unused_dev_dependency() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dev_dependencies:\n  mockito: ^5.0.0\n",
    )?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let project = scan_project(fixture.path())?;

    let report = analyze_dependency_hygiene(&project)?;

    assert_eq!(report.unused_dependencies.len(), 1);
    assert_eq!(report.unused_dependencies[0].dependency, "mockito");
    assert_eq!(
        report.unused_dependencies[0].issue,
        DependencyIssue::UnusedDevDependency
    );
    assert_eq!(
        report.unused_dependencies[0].section,
        DependencySection::DevDependencies
    );

    Ok(())
}

#[test]
fn reports_lockfile_absent_dependency_override() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  http: ^1.0.0\n\
dependency_overrides:\n  stale: ^1.0.0\n",
    )?;
    write(
        &fixture,
        "pubspec.lock",
        "packages:\n  http:\n    dependency: \"direct main\"\n    description:\n      name: http\n    source: hosted\n    version: \"1.0.0\"\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:http/http.dart';\nvoid main() {}\n",
    )?;
    let project = scan_project(fixture.path())?;
    let packages = discover_packages(&project.root)?;
    assert_eq!(packages.len(), 1);
    assert_eq!(
        packages[0]
            .dependencies
            .iter()
            .map(|dependency| (dependency.name.as_str(), dependency.section))
            .collect::<Vec<_>>(),
        vec![
            ("http", DependencySection::Dependencies),
            ("stale", DependencySection::DependencyOverrides),
        ]
    );
    assert_eq!(
        packages[0].locked_packages.clone().unwrap_or_default(),
        ["http".to_owned()].into_iter().collect()
    );

    let report = analyze_dependency_hygiene(&project)?;

    assert_eq!(report.unused_dependencies.len(), 1);
    assert!(report.misconfigured_dependency_overrides.is_empty());
    assert_eq!(report.unused_dependencies[0].dependency, "stale");
    assert_eq!(
        report.unused_dependencies[0].issue,
        DependencyIssue::UnusedDependencyOverride
    );
    assert_eq!(
        report.unused_dependencies[0].section,
        DependencySection::DependencyOverrides
    );
    assert!(!report.unused_dependencies[0].safe_to_delete);
    assert_eq!(report.unused_dependencies[0].location.line, 5);

    Ok(())
}

#[test]
fn reports_misconfigured_dependency_overrides() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependency_overrides:\n  Bad-Name: ^1.0.0\n  stale:\n  patched: ^1.0.0\n",
    )?;
    write(&fixture, "pubspec.lock", "packages: {}\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let project = scan_project(fixture.path())?;

    let report = analyze_dependency_hygiene(&project)?;

    assert_eq!(report.unused_dependencies.len(), 1);
    assert_eq!(report.unused_dependencies[0].dependency, "patched");
    assert_eq!(report.misconfigured_dependency_overrides.len(), 2);
    assert_eq!(
        report.misconfigured_dependency_overrides[0].raw_key,
        "Bad-Name"
    );
    assert_eq!(
        report.misconfigured_dependency_overrides[0].reason,
        DependencyOverrideMisconfigReason::UnparsableKey
    );
    assert_eq!(
        report.misconfigured_dependency_overrides[1].raw_key,
        "stale"
    );
    assert_eq!(
        report.misconfigured_dependency_overrides[1].reason,
        DependencyOverrideMisconfigReason::EmptyValue
    );

    Ok(())
}

#[test]
fn skips_dependency_override_hygiene_without_lockfile() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependency_overrides:\n  patched: ^1.0.0\n",
    )?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let project = scan_project(fixture.path())?;

    let report = analyze_dependency_hygiene(&project)?;

    assert!(report.unused_dependencies.is_empty());
    assert!(report.unlisted_dependencies.is_empty());

    Ok(())
}

#[test]
fn reports_runtime_import_declared_only_as_override_as_unlisted()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependency_overrides:\n  collection: ^1.0.0\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:collection/collection.dart';\nvoid main() {}\n",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_dependency_hygiene(&project)?;

    assert_eq!(report.unlisted_dependencies.len(), 1);
    assert_eq!(report.unlisted_dependencies[0].dependency, "collection");
    assert_eq!(
        report.unlisted_dependencies[0].declared_section,
        Some(DependencySection::DependencyOverrides)
    );

    Ok(())
}

#[test]
fn pubspec_overrides_dependency_overrides_replace_pubspec_overrides()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependency_overrides:\n  stale: ^1.0.0\n",
    )?;
    write(
        &fixture,
        "pubspec_overrides.yaml",
        "dependency_overrides:\n  patched: ^1.0.0\n",
    )?;
    write(&fixture, "pubspec.lock", "packages: {}\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let project = scan_project(fixture.path())?;

    let report = analyze_dependency_hygiene(&project)?;

    assert_eq!(report.unused_dependencies.len(), 1);
    assert_eq!(report.unused_dependencies[0].dependency, "patched");
    assert!(
        report.unused_dependencies[0]
            .pubspec_path
            .ends_with("pubspec_overrides.yaml")
    );
    assert_eq!(
        report.unused_dependencies[0].issue,
        DependencyIssue::UnusedDependencyOverride
    );
    assert_eq!(report.unused_dependencies[0].location.line, 2);

    Ok(())
}

#[test]
fn pubspec_overrides_dependency_overrides_count_as_override_only_declarations()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "pubspec_overrides.yaml",
        "dependency_overrides:\n  collection: ^1.0.0\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:collection/collection.dart';\nvoid main() {}\n",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_dependency_hygiene(&project)?;

    assert_eq!(report.unlisted_dependencies.len(), 1);
    assert_eq!(report.unlisted_dependencies[0].dependency, "collection");
    assert_eq!(
        report.unlisted_dependencies[0].declared_section,
        Some(DependencySection::DependencyOverrides)
    );

    Ok(())
}

#[test]
fn location_ignores_nested_path_key_when_dependency_name_is_path()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  local:\n    path: local\n  path: ^1.9.0\n",
    )?;
    write(&fixture, "local/pubspec.yaml", "name: local\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let project = scan_project(fixture.path())?;

    let report = analyze_dependency_hygiene(&project)?;
    let Some(path_dependency) = report
        .unused_dependencies
        .iter()
        .find(|dependency| dependency.dependency == "path")
    else {
        panic!("path dependency finding");
    };

    assert_eq!(path_dependency.location.line, 5);
    assert!(path_dependency.safe_to_delete);

    Ok(())
}

#[test]
fn runtime_import_prevents_test_only_dependency() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  collection: ^1.0.0\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:collection/collection.dart';\nvoid main() {}\n",
    )?;
    write(
        &fixture,
        "test/app_test.dart",
        "import 'package:collection/collection.dart';\nvoid main() {}\n",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_dependency_hygiene(&project)?;

    assert!(report.unused_dependencies.is_empty());
    assert!(report.unlisted_dependencies.is_empty());

    Ok(())
}

#[test]
fn reports_unlisted_dependency_from_generated_flutterfire_options()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/firebase_options.dart",
        "import 'package:firebase_core/firebase_core.dart';\n\
class DefaultFirebaseOptions {\n\
  static const FirebaseOptions currentPlatform = FirebaseOptions(apiKey: 'key');\n\
}\n",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_dependency_hygiene(&project)?;

    assert_eq!(report.unlisted_dependencies.len(), 1);
    assert_eq!(report.unlisted_dependencies[0].dependency, "firebase_core");
    assert!(
        report.unlisted_dependencies[0]
            .path
            .ends_with("lib/firebase_options.dart")
    );
    assert_eq!(report.unlisted_dependencies[0].location.line, 1);

    Ok(())
}

#[test]
fn finds_declared_package_dependencies_for_trace() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  collection: ^1.18.0\n",
    )?;

    let declarations = declared_package_dependencies(fixture.path(), "collection")?;

    assert_eq!(declarations.len(), 1);
    assert_eq!(declarations[0].package, "app");
    assert_eq!(declarations[0].dependency, "collection");
    assert_eq!(declarations[0].section, DependencySection::Dependencies);
    assert_eq!(declarations[0].location.line, 3);
    assert!(declarations[0].pubspec_path.ends_with("pubspec.yaml"));

    Ok(())
}

#[test]
fn analyzes_workspace_packages_independently() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: root\nworkspace:\n  - packages/*\n",
    )?;
    write(
        &fixture,
        "packages/app/pubspec.yaml",
        "name: app\ndependencies:\n  shared: ^1.0.0\n",
    )?;
    write(&fixture, "packages/shared/pubspec.yaml", "name: shared\n")?;
    write(
        &fixture,
        "packages/app/lib/main.dart",
        "import 'package:shared/shared.dart';\nvoid main() {}\n",
    )?;
    write(
        &fixture,
        "packages/shared/lib/shared.dart",
        "class Shared {}\n",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_dependency_hygiene(&project)?;

    assert!(report.unlisted_dependencies.is_empty());
    assert!(
        report
            .unused_dependencies
            .iter()
            .all(|dependency| dependency.dependency != "shared")
    );

    Ok(())
}

#[test]
fn reports_dev_dependency_in_production_for_owning_workspace_package()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: root\nworkspace:\n  - packages/*\n",
    )?;
    write(
        &fixture,
        "packages/app/pubspec.yaml",
        "name: app\ndev_dependencies:\n  collection: ^1.0.0\n",
    )?;
    write(
        &fixture,
        "packages/app/lib/main.dart",
        "import 'package:collection/collection.dart';\nvoid main() {}\n",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_dependency_hygiene(&project)?;

    assert!(report.unlisted_dependencies.is_empty());
    assert_eq!(report.unused_dependencies.len(), 1);
    assert_eq!(report.unused_dependencies[0].package, "app");
    assert!(
        report.unused_dependencies[0]
            .pubspec_path
            .ends_with("packages/app/pubspec.yaml")
    );
    assert_eq!(
        report.unused_dependencies[0].issue,
        DependencyIssue::DevDependencyInProduction
    );

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
