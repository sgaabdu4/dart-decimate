use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::*;
use crate::{DartFile, DartImport};

#[test]
fn resolves_package_imports_using_ancestor_package_config_from_member_root()
-> Result<(), Box<dyn std::error::Error>> {
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
        "packages/app/pubspec.yaml",
        "name: app\nresolution: workspace\n",
    )?;
    write(
        &fixture,
        "packages/shared/pubspec.yaml",
        "name: shared\nresolution: workspace\n",
    )?;

    let root = fixture.path().join("packages/app");
    let main = dart_file(
        fixture.path().join("packages/app/lib/main.dart"),
        "package:shared/shared.dart",
    );
    let shared = DartFile {
        path: fixture.path().join("packages/shared/lib/shared.dart"),
        library: None,
        part_of: None,
        imports: Vec::new(),
        exports: Vec::new(),
        parts: Vec::new(),
        declarations: Vec::new(),
        members: Vec::new(),
        references: Vec::new(),
        signature_references: Vec::new(),
        routes: Vec::new(),
    };

    let graph = build_module_graph(&root, &[main, shared])?;

    assert_eq!(graph.edge_count(), 1);
    assert_eq!(
        strip_root(fixture.path(), &graph.dependencies()[0].to_path),
        PathBuf::from("packages/shared/lib/shared.dart")
    );
    assert!(graph.unresolved().is_empty());

    Ok(())
}

fn dart_file(path: PathBuf, import_uri: &str) -> DartFile {
    DartFile {
        path,
        library: None,
        part_of: None,
        imports: vec![DartImport {
            uri: import_uri.to_owned(),
            condition: None,
            prefix: None,
            deferred: false,
            combinators: Vec::new(),
            location: Location { line: 1, column: 0 },
        }],
        exports: Vec::new(),
        parts: Vec::new(),
        declarations: Vec::new(),
        members: Vec::new(),
        references: Vec::new(),
        signature_references: Vec::new(),
        routes: Vec::new(),
    }
}

fn strip_root(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
