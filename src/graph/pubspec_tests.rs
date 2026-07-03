use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::*;
use crate::{DartFile, DartImport, Location};

#[test]
fn pubspec_overrides_dependency_overrides_drive_package_resolution()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write(
        "pubspec.yaml",
        "name: app\ndependencies:\n  shared:\n    path: old_shared\n",
    )?;
    fixture.write(
        "pubspec_overrides.yaml",
        "dependency_overrides:\n  shared:\n    path: patched_shared\n",
    )?;

    let main = fixture.file("lib/main.dart", vec![import("package:shared/shared.dart")]);
    fixture.write("old_shared/pubspec.yaml", "name: shared\n")?;
    let stale = fixture.file("old_shared/lib/shared.dart", vec![]);
    fixture.write("patched_shared/pubspec.yaml", "name: shared\n")?;
    let patched = fixture.file("patched_shared/lib/shared.dart", vec![]);

    let graph = build_module_graph(fixture.root(), &[main, stale, patched])?;

    assert_eq!(graph.edge_count(), 1);
    assert_eq!(
        strip_root(fixture.root(), &graph.dependencies()[0].to_path),
        "patched_shared/lib/shared.dart"
    );

    Ok(())
}

#[test]
fn pubspec_overrides_workspace_replaces_pubspec_workspace() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = Fixture::new()?;
    fixture.write(
        "pubspec.yaml",
        "name: root\nworkspace:\n  - old_packages/*\n",
    )?;
    fixture.write("pubspec_overrides.yaml", "workspace:\n  - packages/*\n")?;
    fixture.write("packages/shared/pubspec.yaml", "name: shared\n")?;

    let main = fixture.file("lib/main.dart", vec![import("package:shared/shared.dart")]);
    let shared = fixture.file("packages/shared/lib/shared.dart", vec![]);

    let graph = build_module_graph(fixture.root(), &[main, shared])?;

    assert_eq!(graph.package_names(), vec!["root", "shared"]);
    assert!(graph.unresolved().is_empty());

    Ok(())
}

fn import(uri: &str) -> DartImport {
    DartImport {
        uri: uri.to_owned(),
        condition: None,
        prefix: None,
        deferred: false,
        combinators: Vec::new(),
        location: Location { line: 1, column: 0 },
    }
}

fn strip_root(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

struct Fixture {
    temp: TempDir,
}

impl Fixture {
    fn new() -> Result<Self, std::io::Error> {
        tempfile::tempdir().map(|temp| Self { temp })
    }

    fn root(&self) -> &Path {
        self.temp.path()
    }

    fn path(&self, path: &str) -> PathBuf {
        normalize_path(&self.root().join(path))
    }

    fn write(&self, path: &str, source: &str) -> Result<(), std::io::Error> {
        let path = self.path(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, source)
    }

    fn file(&self, path: &str, imports: Vec<DartImport>) -> DartFile {
        DartFile {
            path: self.path(path),
            library: None,
            part_of: None,
            imports,
            exports: vec![],
            parts: vec![],
            declarations: vec![],
            members: vec![],
            references: vec![],
            signature_references: vec![],
            routes: vec![],
        }
    }
}
