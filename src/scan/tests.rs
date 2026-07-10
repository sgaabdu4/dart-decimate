use std::fs;

use tempfile::TempDir;

use super::*;

#[test]
fn scans_dart_files_in_parallel_and_skips_build_outputs() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'live.dart';\nvoid main() {}\n",
    )?;
    write(&fixture, "lib/live.dart", "class Live {}\n")?;
    write(&fixture, "build/generated.dart", "class Generated {}\n")?;

    let project = scan_project(fixture.path())?;

    assert_eq!(project.files.len(), 2);
    assert_eq!(project.graph.node_count(), 2);
    assert_eq!(project.graph.edge_count(), 1);

    Ok(())
}

#[test]
fn skips_rust_target_fixture_pubspecs() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(
        &fixture,
        "target/stale_fixture/pubspec.yaml",
        "name: stale_fixture\nworkspace:\n  - missing/*\n",
    )?;
    write(
        &fixture,
        "target/stale_fixture/lib/generated.dart",
        "class Generated {}\n",
    )?;

    let project = scan_project(fixture.path())?;

    assert_eq!(project.files.len(), 1);
    assert_eq!(project.files[0].path, fixture.path().join("lib/main.dart"));

    Ok(())
}

#[test]
fn scans_local_path_dependencies_for_graph_resolution() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "app/pubspec.yaml",
        "name: app\ndependencies:\n  shared:\n    path: ../shared\n",
    )?;
    write(
        &fixture,
        "app/lib/main.dart",
        "import 'package:shared/shared.dart';\nvoid main() {}\n",
    )?;
    write(&fixture, "shared/pubspec.yaml", "name: shared\n")?;
    write(&fixture, "shared/lib/shared.dart", "class Shared {}\n")?;

    let project = scan_project(fixture.path().join("app"))?;

    assert_eq!(project.files.len(), 2);
    assert_eq!(project.graph.edge_count(), 1);
    assert!(project.graph.unresolved().is_empty());

    Ok(())
}

#[test]
fn scans_local_package_config_roots_for_graph_resolution() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "app/.dart_tool/package_config.json",
        r#"{
  "configVersion": 2,
  "packages": [
    {"name": "app", "rootUri": "../", "packageUri": "lib/"},
    {"name": "shared", "rootUri": "../../shared", "packageUri": "lib/"}
  ]
}
"#,
    )?;
    write(
        &fixture,
        "app/lib/main.dart",
        "import 'package:shared/shared.dart';\nvoid main() {}\n",
    )?;
    write(&fixture, "shared/lib/shared.dart", "class Shared {}\n")?;

    let project = scan_project(fixture.path().join("app"))?;

    assert_eq!(project.files.len(), 2);
    assert_eq!(project.graph.edge_count(), 1);
    assert!(project.graph.unresolved().is_empty());

    Ok(())
}

#[test]
fn scans_workspace_package_config_from_member_root() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".dart_tool/package_config.json",
        r#"{
  "configVersion": 2,
  "packages": [
    {"name": "app", "rootUri": "../packages/app", "packageUri": "lib/"},
    {"name": "shared", "rootUri": "../packages/shared", "packageUri": "lib/"}
  ]
}
"#,
    )?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: workspace\nworkspace:\n  - packages/*\n",
    )?;
    write(
        &fixture,
        "packages/app/pubspec.yaml",
        "name: app\nresolution: workspace\ndependencies:\n  shared: ^1.0.0\n",
    )?;
    write(
        &fixture,
        "packages/app/lib/main.dart",
        "import 'package:shared/shared.dart';\nvoid main() {}\n",
    )?;
    write(
        &fixture,
        "packages/shared/pubspec.yaml",
        "name: shared\nresolution: workspace\n",
    )?;
    write(
        &fixture,
        "packages/shared/lib/shared.dart",
        "class Shared {}\n",
    )?;

    let project = scan_project(fixture.path().join("packages/app"))?;

    assert_eq!(project.files.len(), 2);
    assert_eq!(project.graph.edge_count(), 1);
    assert!(project.graph.unresolved().is_empty());

    Ok(())
}

#[test]
fn does_not_scan_pub_cache_package_config_roots() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "app/.dart_tool/package_config.json",
        r#"{
  "configVersion": 2,
  "packages": [
    {"name": "app", "rootUri": "../", "packageUri": "lib/"},
    {"name": "hosted", "rootUri": "../../.pub-cache/hosted/hosted-1.0.0", "packageUri": "lib/"}
  ]
}
"#,
    )?;
    write(
        &fixture,
        "app/lib/main.dart",
        "import 'package:hosted/hosted.dart';\nvoid main() {}\n",
    )?;
    write(
        &fixture,
        ".pub-cache/hosted/hosted-1.0.0/lib/hosted.dart",
        "class Hosted {}\n",
    )?;

    let project = scan_project(fixture.path().join("app"))?;

    assert_eq!(project.files.len(), 1);
    assert_eq!(project.graph.edge_count(), 0);
    assert!(project.graph.unresolved().is_empty());

    Ok(())
}

#[test]
fn scans_pub_workspace_member_files() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\nworkspace:\n  - packages/*\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:shared/shared.dart';\nvoid main() {}\n",
    )?;
    write(&fixture, "packages/shared/pubspec.yaml", "name: shared\n")?;
    write(
        &fixture,
        "packages/shared/lib/shared.dart",
        "class Shared {}\n",
    )?;

    let project = scan_project(fixture.path())?;

    assert_eq!(project.files.len(), 2);
    assert_eq!(project.graph.edge_count(), 1);
    assert!(project.graph.unresolved().is_empty());

    Ok(())
}

#[test]
fn repository_scan_stops_parent_gitignore_discovery_at_git_root()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, ".gitignore", "*.dart\n")?;
    write(&fixture, "repo/.git/HEAD", "ref: refs/heads/main\n")?;
    write(&fixture, "repo/.gitignore", "ignored.dart\n")?;
    write(&fixture, "repo/pubspec.yaml", "name: app\n")?;
    write(&fixture, "repo/lib/main.dart", "void main() {}\n")?;
    write(&fixture, "repo/lib/ignored.dart", "class Ignored {}\n")?;

    let project = scan_project(fixture.path().join("repo"))?;

    assert_eq!(project.files.len(), 1);
    assert_eq!(
        project.files[0].path,
        fixture.path().join("repo/lib/main.dart")
    );

    Ok(())
}

#[test]
fn non_repository_scan_uses_root_gitignore_without_parent_rules()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, ".gitignore", "*.dart\n")?;
    write(&fixture, "app/.gitignore", "ignored.dart\n")?;
    write(&fixture, "app/pubspec.yaml", "name: app\n")?;
    write(&fixture, "app/lib/main.dart", "void main() {}\n")?;
    write(&fixture, "app/lib/ignored.dart", "class Ignored {}\n")?;

    let project = scan_project(fixture.path().join("app"))?;

    assert_eq!(project.files.len(), 1);
    assert_eq!(
        project.files[0].path,
        fixture.path().join("app/lib/main.dart")
    );

    Ok(())
}

#[test]
fn ignored_external_package_root_is_not_scanned() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, ".git/HEAD", "ref: refs/heads/main\n")?;
    write(&fixture, ".gitignore", "shared/\n")?;
    write(
        &fixture,
        "app/pubspec.yaml",
        "name: app\ndependencies:\n  shared:\n    path: ../shared\n",
    )?;
    write(&fixture, "app/lib/main.dart", "void main() {}\n")?;
    write(&fixture, "shared/pubspec.yaml", "name: shared\n")?;
    write(
        &fixture,
        "shared/lib/generated.dart",
        "class Generated {}\n",
    )?;

    let project = scan_project(fixture.path().join("app"))?;

    assert_eq!(project.files.len(), 1);
    assert_eq!(
        project.files[0].path,
        fixture.path().join("app/lib/main.dart")
    );
    Ok(())
}

#[test]
fn scans_all_conditional_import_targets() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'io.dart' if (dart.library.html) 'web.dart';\nvoid main() {}\n",
    )?;
    write(&fixture, "lib/io.dart", "class Io {}\n")?;
    write(&fixture, "lib/web.dart", "class Web {}\n")?;

    let project = scan_project(fixture.path())?;

    assert_eq!(project.graph.edge_count(), 2);
    assert!(project.graph.unresolved().is_empty());

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
