use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::*;
use crate::{DartFile, DartImport, Location};

#[test]
fn directive_uri_paths_are_percent_decoded_for_graph_resolution()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\nworkspace:\n  - packages/*\n")?;
    fixture.write("packages/shared/pubspec.yaml", "name: shared\n")?;

    let main = fixture.file(
        "lib/main.dart",
        vec![
            import("src/api%20impl.dart"),
            import("package:shared/src/package%20api.dart"),
        ],
    );
    let local = fixture.file("lib/src/api impl.dart", vec![]);
    let shared = fixture.file("packages/shared/lib/src/package api.dart", vec![]);

    let graph = build_module_graph(fixture.root(), &[main, local, shared])?;

    assert!(graph.unresolved().is_empty());
    assert_eq!(
        graph
            .dependencies()
            .into_iter()
            .map(|dependency| strip_root(fixture.root(), &dependency.to_path))
            .collect::<Vec<_>>(),
        vec![
            "lib/src/api impl.dart".to_owned(),
            "packages/shared/lib/src/package api.dart".to_owned(),
        ]
    );

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
