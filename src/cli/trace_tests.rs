use std::fs;

use serde_json::Value;
use tempfile::TempDir;

use super::run_from;

#[test]
fn trace_file_command_emits_json_contract() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'barrel.dart';\nvoid main() { Api(); }\n",
    )?;
    write(&fixture, "lib/barrel.dart", "export 'src/api.dart';\n")?;
    write(&fixture, "lib/src/api.dart", "class Api {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "trace-file",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
            "--file",
            "lib/barrel.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "dart-decimate.trace.v1");
    assert_eq!(json["kind"], "trace-file");
    assert_eq!(json["command"], "trace-file");
    assert_eq!(json["path"], "lib/barrel.dart");
    assert_eq!(json["reachable"], true);
    assert_eq!(json["imported_by"][0]["from"], "lib/main.dart");
    assert_eq!(json["re_exports"][0]["to"], "lib/src/api.dart");

    Ok(())
}

#[test]
fn trace_file_command_reports_missing_file_without_failure()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "trace-file",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
            "--file",
            "lib/missing.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["found"], false);
    assert_eq!(json["reason"], "file is not in the module graph");

    Ok(())
}

#[test]
fn trace_symbol_command_emits_json_contract() -> Result<(), Box<dyn std::error::Error>> {
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
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "trace-symbol",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
            "--file",
            "lib/src/api.dart",
            "--symbol",
            "Api",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["command"], "trace-symbol");
    assert_eq!(json["path"], "lib/src/api.dart");
    assert_eq!(json["symbol"], "Api");
    assert_eq!(json["found"], true);
    assert_eq!(json["direct_references"][0]["path"], "lib/main.dart");
    assert_eq!(json["re_export_chains"][0][0], "lib/barrel.dart");
    assert_eq!(json["reason"], "symbol has reachable direct references");

    Ok(())
}

#[test]
fn trace_command_alias_emits_symbol_trace_contract() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/api.dart';\nvoid main() { Api(); }\n",
    )?;
    write(&fixture, "lib/src/api.dart", "class Api {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "trace",
            "--root",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
            "lib/src/api.dart:Api",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "dart-decimate.trace.v1");
    assert_eq!(json["kind"], "trace-symbol");
    assert_eq!(json["command"], "trace-symbol");
    assert_eq!(json["path"], "lib/src/api.dart");
    assert_eq!(json["symbol"], "Api");
    assert_eq!(json["found"], true);
    assert_eq!(json["direct_references"][0]["path"], "lib/main.dart");

    Ok(())
}

#[test]
fn trace_dependency_command_emits_json_contract() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  collection: ^1.18.0\n  path: ^1.9.0\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:collection/collection.dart' deferred as coll;\n\
export 'package:collection/equality.dart';\n\
void main() {}\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "trace-dependency",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--dependency",
            "collection",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "dart-decimate.trace.v1");
    assert_eq!(json["kind"], "trace-dependency");
    assert_eq!(json["command"], "trace-dependency");
    assert_eq!(json["dependency"], "collection");
    assert_eq!(json["found"], true);
    assert_eq!(json["declared"], true);
    assert_eq!(json["is_used"], true);
    assert_eq!(json["used_in_scripts"], false);
    assert_eq!(json["total_import_count"], 2);
    let Some(type_only_importers) = json["type_only_importers"].as_array() else {
        panic!("type_only_importers array");
    };
    assert_eq!(type_only_importers, &Vec::<Value>::new());
    assert_eq!(json["declared_in"][0]["pubspec_path"], "pubspec.yaml");
    assert_eq!(json["declared_in"][0]["section"], "dependencies");
    assert_eq!(json["declared_in"][0]["line"], 3);
    assert_eq!(json["importing_files"][0]["package"], "app");
    assert_eq!(json["importing_files"][0]["kind"], "import");
    assert_eq!(
        json["importing_files"][0]["specifier"],
        "package:collection/collection.dart"
    );
    assert_eq!(json["importing_files"][0]["prefix"], "coll");
    assert_eq!(json["importing_files"][0]["deferred"], true);
    assert_eq!(json["importing_files"][1]["kind"], "export");

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
