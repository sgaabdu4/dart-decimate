use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::*;
use crate::{DartLibrary, DartPart, DartPartOf};

#[test]
fn builds_edges_for_library_augment_directives() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    let base = fixture.file("lib/base.dart");
    let mut augment = fixture.file("lib/base_augment.dart");
    augment.library = Some(library_augment("base.dart"));

    let graph = build_module_graph(fixture.root(), &[base, augment])?;
    let dependencies = graph.dependencies();

    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].kind, DependencyKind::Augment);
    assert_eq!(
        strip_root(fixture.root(), &dependencies[0].from_path),
        "lib/base_augment.dart"
    );
    assert_eq!(
        strip_root(fixture.root(), &dependencies[0].to_path),
        "lib/base.dart"
    );
    assert_eq!(graph.unresolved(), []);

    Ok(())
}

#[test]
fn missing_library_augment_target_is_unresolved() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    let mut augment = fixture.file("lib/base_augment.dart");
    augment.library = Some(library_augment("base.dart"));

    let graph = build_module_graph(fixture.root(), &[augment])?;

    assert_eq!(graph.unresolved().len(), 1);
    assert_eq!(graph.unresolved()[0].kind, DependencyKind::Augment);
    assert_eq!(graph.unresolved()[0].specifier, "base.dart");
    assert_eq!(
        strip_root(fixture.root(), &graph.unresolved()[0].attempted_path),
        "lib/base.dart"
    );

    Ok(())
}

#[test]
fn duplicate_part_ownership_is_reported_once() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    let mut first = fixture.file("lib/a.dart");
    first.library = Some(library_name("app.model"));
    first.parts = vec![part("src/shared_part.dart")];
    let mut second = fixture.file("lib/b.dart");
    second.library = Some(library_name("app.model"));
    second.parts = vec![part("src/shared_part.dart")];
    let mut shared = fixture.file("lib/src/shared_part.dart");
    shared.part_of = Some(part_of_name("app.model"));

    let graph = build_module_graph(fixture.root(), &[first, second, shared])?;

    assert_eq!(graph.edge_count(), 1);
    assert_eq!(graph.invalid_part_relationships().len(), 1);
    assert_eq!(
        graph.invalid_part_relationships()[0].reason,
        InvalidPartReason::DuplicatePartOwner {
            existing_library_path: fixture.path("lib/a.dart")
        }
    );

    Ok(())
}

#[test]
fn duplicate_part_ownership_wins_over_uri_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    let mut first = fixture.file("lib/a.dart");
    first.parts = vec![part("src/model.g.dart")];
    let mut second = fixture.file("lib/b.dart");
    second.parts = vec![part("src/model.g.dart")];
    let mut generated = fixture.file("lib/src/model.g.dart");
    generated.part_of = Some(part_of_uri("../a.dart"));

    let graph = build_module_graph(fixture.root(), &[first, second, generated])?;

    assert_eq!(graph.edge_count(), 1);
    assert_eq!(graph.invalid_part_relationships().len(), 1);
    assert_eq!(
        graph.invalid_part_relationships()[0].reason,
        InvalidPartReason::DuplicatePartOwner {
            existing_library_path: fixture.path("lib/a.dart")
        }
    );

    Ok(())
}

#[test]
fn duplicate_part_directives_in_same_library_are_deduped() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    let mut library = fixture.file("lib/model.dart");
    library.library = Some(library_name("app.model"));
    library.parts = vec![part("src/model.g.dart"), part("src/model.g.dart")];
    let mut generated = fixture.file("lib/src/model.g.dart");
    generated.part_of = Some(part_of_name("app.model"));

    let graph = build_module_graph(fixture.root(), &[library, generated])?;

    assert_eq!(graph.edge_count(), 1);
    assert_eq!(graph.invalid_part_relationships(), []);

    Ok(())
}

fn library_augment(uri: &str) -> DartLibrary {
    DartLibrary {
        name: None,
        augment_uri: Some(uri.to_owned()),
        location: Location { line: 1, column: 0 },
    }
}

fn library_name(name: &str) -> DartLibrary {
    DartLibrary {
        name: Some(name.to_owned()),
        augment_uri: None,
        location: Location { line: 1, column: 0 },
    }
}

fn part(uri: &str) -> DartPart {
    DartPart {
        uri: uri.to_owned(),
        location: Location { line: 1, column: 0 },
    }
}

fn part_of_name(name: &str) -> DartPartOf {
    DartPartOf {
        name: Some(name.to_owned()),
        uri: None,
        location: Location { line: 1, column: 0 },
    }
}

fn part_of_uri(uri: &str) -> DartPartOf {
    DartPartOf {
        name: None,
        uri: Some(uri.to_owned()),
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

    fn file(&self, path: &str) -> DartFile {
        DartFile {
            path: self.path(path),
            library: None,
            part_of: None,
            imports: vec![],
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
