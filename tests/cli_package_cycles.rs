use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn strict_check_fails_two_workspace_packages_that_depend_on_each_other()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_workspace(&fixture, "  a:\n    path: ../a\n")?;

    let (code, json) = strict_check(&fixture)?;

    assert_eq!(code, 1);
    assert_eq!(json["summary"]["package_cycles"], 1);
    assert_eq!(json["summary"]["cycles"], 0);
    let findings = package_cycle_findings(&json);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["kind"], "package-cycle");
    assert_eq!(findings[0]["files"][0], "packages/a/pubspec.yaml");
    assert_eq!(findings[0]["files"][1], "packages/b/pubspec.yaml");

    Ok(())
}

#[test]
fn strict_check_keeps_workspace_packages_with_one_way_dependencies()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_workspace(&fixture, "")?;

    let (code, json) = strict_check(&fixture)?;

    assert_eq!(code, 0);
    assert_eq!(json["summary"]["package_cycles"], 0);

    Ok(())
}

fn write_workspace(fixture: &TempDir, b_dependencies: &str) -> Result<(), std::io::Error> {
    write(
        fixture,
        "pubspec.yaml",
        "name: root\nenvironment:\n  sdk: ^3.6.0\nworkspace:\n  - packages/a\n  - packages/b\n",
    )?;
    write(
        fixture,
        "packages/a/pubspec.yaml",
        "name: a\nresolution: workspace\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  b:\n    path: ../b\n",
    )?;
    write(
        fixture,
        "packages/b/pubspec.yaml",
        &format!(
            "name: b\nresolution: workspace\nenvironment:\n  sdk: ^3.6.0\ndependencies:\n  c:\n    path: ../c\n{b_dependencies}"
        ),
    )?;
    write(
        fixture,
        "packages/c/pubspec.yaml",
        "name: c\nresolution: workspace\nenvironment:\n  sdk: ^3.6.0\n",
    )?;
    write(fixture, "packages/c/lib/c.dart", "String fc() => 'c';\n")?;
    write(
        fixture,
        "packages/a/lib/a.dart",
        "import 'package:b/b.dart';\n\nString fa() => 'a${fb()}';\n",
    )?;
    write(
        fixture,
        "packages/b/lib/b.dart",
        &if b_dependencies.is_empty() {
            "import 'package:c/c.dart';\n\nString fb() => fc();\n".to_owned()
        } else {
            "import 'package:a/other.dart';\nimport 'package:c/c.dart';\n\nString fb() => other() + fc();\n"
                .to_owned()
        },
    )?;
    write(
        fixture,
        "packages/a/lib/other.dart",
        "String other() => 'o';\n",
    )?;
    write(
        fixture,
        "packages/a/bin/main.dart",
        "import 'package:a/a.dart';\n\nvoid main() {\n  print(fa());\n}\n",
    )?;
    Ok(())
}

fn package_cycle_findings(json: &Value) -> Vec<&Value> {
    json["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|finding| finding["rule_id"] == "dart-decimate/package-cycle")
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
