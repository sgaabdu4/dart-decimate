use std::fs;
use std::path::PathBuf;

use tempfile::TempDir;

use super::*;
use crate::scan_project;

#[test]
fn reports_library_files_not_covered_by_boundary_zones() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/domain/model.dart", "class Model {}\n")?;
    write(&fixture, "lib/ui/page.dart", "class Page {}\n")?;
    write(
        &fixture,
        "lib/data/repository.dart",
        "class Repository {}\n",
    )?;
    let project = scan_project(fixture.path())?;

    let gaps =
        detect_boundary_coverage(&project, &[BoundaryRule::new("lib/domain", "lib/ui")], &[]);

    assert_eq!(
        gaps.iter()
            .filter_map(|gap| gap.path.strip_prefix(fixture.path()).ok())
            .map(PathBuf::from)
            .collect::<Vec<_>>(),
        vec![PathBuf::from("lib/data/repository.dart")]
    );

    Ok(())
}

#[test]
fn skips_generated_and_non_library_files() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/domain/model.dart", "class Model {}\n")?;
    write(&fixture, "lib/ui/page.dart", "class Page {}\n")?;
    write(&fixture, "lib/orphan.g.dart", "class Generated {}\n")?;
    write(&fixture, "test/orphan_test.dart", "void main() {}\n")?;
    let project = scan_project(fixture.path())?;

    let gaps =
        detect_boundary_coverage(&project, &[BoundaryRule::new("lib/domain", "lib/ui")], &[]);

    assert_eq!(gaps, []);

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
