use std::fs;

use tempfile::TempDir;

use super::*;
use crate::{find_dead_code, scan_project};

#[test]
fn traces_file_dependencies_importers_exports_and_declarations()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'barrel.dart';\nvoid main() { Api(); }\n",
    )?;
    write(
        &fixture,
        "lib/barrel.dart",
        "export 'src/api.dart';\nclass Barrel {}\n",
    )?;
    write(&fixture, "lib/src/api.dart", "class Api {}\n")?;
    let project = scan_project(fixture.path())?;
    let dead_code = find_dead_code(&project.graph, ["lib/main.dart"]);

    let report = trace_file(&project, &dead_code, "lib/barrel.dart");

    assert!(report.found);
    assert!(report.reachable);
    assert!(!report.entry_point);
    assert_eq!(report.declarations[0].name, "Barrel");
    assert_eq!(report.imported_by[0].from, "lib/main.dart");
    assert_eq!(report.re_exports[0].to, "lib/src/api.dart");
    assert!(report.imports_from.is_empty());

    Ok(())
}

#[test]
fn traces_symbol_references_and_re_export_chains() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'barrel.dart';\nvoid main() { Api(); }\n",
    )?;
    write(&fixture, "lib/barrel.dart", "export 'src/api.dart';\n")?;
    write(
        &fixture,
        "lib/src/api.dart",
        "class Api {}\nclass Unused {}\n",
    )?;
    let project = scan_project(fixture.path())?;
    let dead_code = find_dead_code(&project.graph, ["lib/main.dart"]);

    let report = trace_symbol(&project, &dead_code, "lib/src/api.dart", "Api");

    assert!(report.found);
    assert!(report.reachable_file);
    assert_eq!(
        report
            .declaration
            .as_ref()
            .map(|declaration| declaration.name.as_str()),
        Some("Api")
    );
    assert_eq!(report.direct_references.len(), 1);
    assert_eq!(report.direct_references[0].path, "lib/main.dart");
    assert!(report.direct_references[0].reachable);
    assert_eq!(
        report.re_export_chains,
        vec![vec![
            "lib/barrel.dart".to_owned(),
            "lib/src/api.dart".to_owned()
        ]]
    );
    assert_eq!(report.reason, "symbol has reachable direct references");

    Ok(())
}

#[test]
fn traces_unused_symbol_without_marking_it_deleted() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/api.dart';\nvoid main() { Used(); }\n",
    )?;
    write(
        &fixture,
        "lib/src/api.dart",
        "class Used {}\nclass Unused {}\n",
    )?;
    let project = scan_project(fixture.path())?;
    let dead_code = find_dead_code(&project.graph, ["lib/main.dart"]);

    let report = trace_symbol(&project, &dead_code, "lib/src/api.dart", "Unused");

    assert!(report.found);
    assert!(report.reachable_file);
    assert!(report.direct_references.is_empty());
    assert_eq!(report.reason, "symbol has no reachable direct references");

    Ok(())
}

#[test]
fn trace_symbol_re_export_chains_respect_show_hide() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'barrel.dart';\nvoid main() {}\n",
    )?;
    write(
        &fixture,
        "lib/barrel.dart",
        "export 'src/api.dart' show PublicApi hide HiddenApi;\n",
    )?;
    write(
        &fixture,
        "lib/src/api.dart",
        "class PublicApi {}\nclass HiddenApi {}\n",
    )?;
    let project = scan_project(fixture.path())?;
    let dead_code = find_dead_code(&project.graph, ["lib/main.dart"]);

    let public = trace_symbol(&project, &dead_code, "lib/src/api.dart", "PublicApi");
    let hidden = trace_symbol(&project, &dead_code, "lib/src/api.dart", "HiddenApi");

    assert_eq!(public.re_export_chains.len(), 1);
    assert!(hidden.re_export_chains.is_empty());

    Ok(())
}

#[test]
fn traces_pub_dependency_declarations_and_importers() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  collection: ^1.18.0\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:collection/collection.dart' deferred as coll;\n\
export 'package:collection/equality.dart';\n\
void main() {}\n",
    )?;
    let project = scan_project(fixture.path())?;

    let report = trace_dependency(&project, "collection")?;

    assert!(report.found);
    assert!(report.declared);
    assert!(report.is_used);
    assert!(!report.used_in_scripts);
    assert_eq!(report.total_import_count, 2);
    assert!(report.type_only_importers.is_empty());
    assert_eq!(report.declared_in[0].pubspec_path, "pubspec.yaml");
    assert_eq!(
        report.declared_in[0].section,
        DependencySection::Dependencies
    );
    assert_eq!(report.importing_files[0].package.as_deref(), Some("app"));
    assert_eq!(report.importing_files[0].kind, "import");
    assert_eq!(report.importing_files[0].line, 1);
    assert_eq!(report.importing_files[0].prefix.as_deref(), Some("coll"));
    assert!(report.importing_files[0].deferred);
    assert_eq!(report.importing_files[1].kind, "export");

    Ok(())
}

#[test]
fn traces_missing_pub_dependency_without_failing() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let project = scan_project(fixture.path())?;

    let report = trace_dependency(&project, "missing_package")?;

    assert!(!report.found);
    assert!(!report.declared);
    assert!(!report.is_used);
    assert_eq!(report.total_import_count, 0);
    assert!(report.declared_in.is_empty());
    assert!(report.importing_files.is_empty());

    Ok(())
}

#[test]
fn traces_missing_files_without_failing() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let project = scan_project(fixture.path())?;
    let dead_code = find_dead_code(&project.graph, ["lib/main.dart"]);

    let report = trace_file(&project, &dead_code, "lib/missing.dart");

    assert!(!report.found);
    assert!(!report.reachable);
    assert_eq!(report.reason, "file is not in the module graph");

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
