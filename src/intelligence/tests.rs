use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::*;
use crate::{DartExport, DartFile, DartImport, GraphError, build_module_graph};

#[test]
fn flags_files_unreachable_from_entry_points() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    let graph = fixture.graph(&[
        fixture.file("lib/main.dart", &["src/a.dart"]),
        fixture.file("lib/src/a.dart", &["b.dart"]),
        fixture.file("lib/src/b.dart", &[]),
        fixture.file("lib/src/dead.dart", &[]),
    ])?;

    let report = find_dead_code(&graph, ["lib/main.dart", "lib/missing.dart"]);

    assert_eq!(
        paths(fixture.root(), report.reachable_files),
        vec!["lib/main.dart", "lib/src/a.dart", "lib/src/b.dart",]
    );
    assert_eq!(
        paths(fixture.root(), report.missing_entry_points),
        vec!["lib/missing.dart",]
    );
    assert_eq!(
        report
            .dead_files
            .into_iter()
            .map(|dead| (strip_root(fixture.root(), &dead.path), dead.safe_to_delete))
            .collect::<Vec<_>>(),
        vec![("lib/src/dead.dart".to_owned(), true)]
    );

    Ok(())
}

#[test]
fn detects_tarjan_cycles_with_deterministic_file_order() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    let graph = fixture.graph(&[
        fixture.file("lib/a.dart", &["b.dart"]),
        fixture.file("lib/b.dart", &["c.dart"]),
        fixture.file("lib/c.dart", &["a.dart"]),
        fixture.file("lib/acyclic.dart", &[]),
    ])?;

    let cycles = detect_cycles(&graph);

    assert_eq!(cycles.len(), 1);
    assert_eq!(
        paths(fixture.root(), cycles[0].files.clone()),
        vec!["lib/a.dart", "lib/b.dart", "lib/c.dart",]
    );

    Ok(())
}

#[test]
fn detects_self_loop_cycles() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    let graph = fixture.graph(&[fixture.file("lib/self.dart", &["self.dart"])])?;

    let cycles = detect_cycles(&graph);

    assert_eq!(cycles.len(), 1);
    assert_eq!(
        paths(fixture.root(), cycles[0].files.clone()),
        vec!["lib/self.dart",]
    );

    Ok(())
}

#[test]
fn detects_re_export_cycles_without_import_cycles() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    let graph = fixture.graph(&[
        fixture.file_with_exports("lib/a.dart", &["b.dart"]),
        fixture.file_with_exports("lib/b.dart", &["a.dart"]),
        fixture.file("lib/c.dart", &["d.dart"]),
        fixture.file("lib/d.dart", &[]),
    ])?;

    let cycles = detect_re_export_cycles(&graph);

    assert_eq!(cycles.len(), 1);
    assert_eq!(
        cycles[0]
            .files
            .iter()
            .map(|path| strip_root(fixture.root(), path))
            .collect::<Vec<_>>(),
        vec!["lib/a.dart", "lib/b.dart"]
    );

    Ok(())
}

#[test]
fn flags_architecture_boundary_violations() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    let graph = fixture.graph(&[
        fixture.file("lib/domain/service.dart", &["../ui/widget.dart"]),
        fixture.file("lib/domain/model.dart", &[]),
        fixture.file("lib/ui/widget.dart", &[]),
        fixture.file("lib/data/repository.dart", &["../domain/model.dart"]),
    ])?;

    let violations =
        check_architecture_boundaries(&graph, &[BoundaryRule::new("lib/domain", "lib/ui")]);

    assert_eq!(violations.len(), 1);
    assert_eq!(
        (
            strip_root(fixture.root(), &violations[0].from_path),
            strip_root(fixture.root(), &violations[0].to_path),
            violations[0].specifier.as_str(),
        ),
        (
            "lib/domain/service.dart".to_owned(),
            "lib/ui/widget.dart".to_owned(),
            "../ui/widget.dart",
        )
    );

    Ok(())
}

fn paths(root: &Path, paths: Vec<PathBuf>) -> Vec<String> {
    paths
        .into_iter()
        .map(|path| strip_root(root, &path))
        .collect()
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
        crate::graph::normalize_path(&self.root().join(path))
    }

    fn write(&self, path: &str, source: &str) -> Result<(), std::io::Error> {
        let path = self.path(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, source)
    }

    fn file(&self, path: &str, imports: &[&str]) -> DartFile {
        DartFile {
            path: self.path(path),
            library: None,
            part_of: None,
            imports: imports.iter().map(|uri| import(uri)).collect(),
            exports: vec![],
            parts: vec![],
            declarations: vec![],
            members: vec![],
            references: vec![],
            signature_references: vec![],
            class_supertypes: vec![],
            routes: vec![],
        }
    }

    fn file_with_exports(&self, path: &str, exports: &[&str]) -> DartFile {
        DartFile {
            path: self.path(path),
            library: None,
            part_of: None,
            imports: vec![],
            exports: exports.iter().map(|uri| export(uri)).collect(),
            parts: vec![],
            declarations: vec![],
            members: vec![],
            references: vec![],
            signature_references: vec![],
            class_supertypes: vec![],
            routes: vec![],
        }
    }

    fn graph(&self, files: &[DartFile]) -> Result<ModuleGraph, GraphError> {
        build_module_graph(self.root(), files)
    }
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

fn export(uri: &str) -> DartExport {
    DartExport {
        uri: uri.to_owned(),
        condition: None,
        combinators: Vec::new(),
        location: Location { line: 1, column: 0 },
    }
}
