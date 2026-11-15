use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn strict_check_fails_one_name_declared_in_two_library_files()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "import 'src/a.dart';\nimport 'src/b.dart' as b;\n\nvoid run() {\n  Label();\n  b.Label();\n}\n",
    )?;
    write(&fixture, "lib/src/a.dart", "class Label {}\n")?;
    write(&fixture, "lib/src/b.dart", "class Label {}\n")?;

    let (code, json) = strict_check(&fixture)?;

    assert_eq!(code, 1);
    assert_eq!(json["summary"]["duplicate_exports"], 1);
    let findings = duplicate_findings(&json);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["files"][0], "lib/src/a.dart");
    assert_eq!(findings[0]["files"][1], "lib/src/b.dart");

    Ok(())
}

#[test]
fn strict_check_keeps_platform_alternatives_that_share_a_name()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "import 'src/stub.dart' if (dart.library.io) 'src/io.dart';\n\nvoid run() {\n  createClient();\n}\n",
    )?;
    write(
        &fixture,
        "lib/src/stub.dart",
        "Object createClient() => 0;\n",
    )?;
    write(&fixture, "lib/src/io.dart", "Object createClient() => 1;\n")?;

    let (code, json) = strict_check(&fixture)?;

    assert_eq!(code, 0);
    assert_eq!(json["summary"]["duplicate_exports"], 0);

    Ok(())
}

#[test]
fn strict_check_keeps_private_names_and_files_outside_lib() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "import 'src/a.dart';\nimport 'src/b.dart' as b;\n\nvoid run() {\n  first();\n  b.second();\n}\n",
    )?;
    write(
        &fixture,
        "lib/src/a.dart",
        "void first() => _helper();\n\nvoid _helper() {}\n",
    )?;
    write(
        &fixture,
        "lib/src/b.dart",
        "void second() => _helper();\n\nvoid _helper() {}\n",
    )?;
    write(&fixture, "bin/tool.dart", "void main() {}\n")?;
    write(&fixture, "bin/other.dart", "void main() {}\n")?;

    let (code, json) = strict_check(&fixture)?;

    assert_eq!(code, 0);
    assert_eq!(json["summary"]["duplicate_exports"], 0);

    Ok(())
}

fn duplicate_findings(json: &Value) -> Vec<&Value> {
    json["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|finding| finding["rule_id"] == "dart-decimate/duplicate-export")
        .collect()
}

fn strict_check(fixture: &TempDir) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--threshold",
            "0",
            "--strict",
            "--format",
            "json",
        ],
        &mut output,
    )?;
    Ok((code, serde_json::from_slice::<Value>(&output)?))
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
