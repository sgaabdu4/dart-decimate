use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn bare_command_defaults_to_check_and_preserves_flags() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["command"], "check");

    Ok(())
}

#[test]
fn bare_strict_command_fails_on_warning_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    let duplicate = "void shared() {\n  final values = [1, 2, 3];\n  final active = values.where((value) => value > 1);\n  print(active.length);\n}\n";
    write(&fixture, "lib/a.dart", duplicate)?;
    write(&fixture, "lib/b.dart", duplicate)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
            "--threshold",
            "100",
            "--strict",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["command"], "check");
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["code_duplications"], 1);
    assert_eq!(json["findings"][0]["severity"], "warning");

    Ok(())
}

#[test]
fn bare_command_defaults_to_check_with_root_flag() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "--root",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["command"], "check");
    assert_eq!(json["summary"]["files"], 1);

    Ok(())
}

#[test]
fn leading_format_flag_defaults_to_check() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "--format",
            "json",
            "--root",
            fixture.path().to_str().unwrap_or("."),
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["command"], "check");
    assert_eq!(json["summary"]["files"], 1);

    Ok(())
}

#[test]
fn json_shortcut_runs_check_with_json_output() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "json",
            fixture.path().to_str().unwrap_or("."),
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["command"], "check");
    assert_eq!(json["summary"]["files"], 1);

    Ok(())
}

#[test]
fn json_shortcut_delimiter_runs_check_with_json_output() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "json",
            "--",
            fixture.path().to_str().unwrap_or("."),
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["command"], "check");
    assert_eq!(json["summary"]["files"], 1);

    Ok(())
}

#[test]
fn human_shortcut_runs_check_with_human_output() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "human",
            fixture.path().to_str().unwrap_or("."),
        ],
        &mut output,
    )?;

    let text = String::from_utf8(output)?;
    assert_eq!(code, 0);
    assert!(text.contains("Dart Decimate check: PASS"));

    Ok(())
}

#[test]
fn html_shortcut_stdout_runs_check_with_html_output() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "html",
            fixture.path().to_str().unwrap_or("."),
            "--stdout",
        ],
        &mut output,
    )?;

    let html = String::from_utf8(output)?;
    assert_eq!(code, 0);
    assert!(html.contains("<!doctype html>"));
    assert!(html.contains("<h1>check report</h1>"));

    Ok(())
}

#[test]
fn explicit_command_accepts_root_flag() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            "--root",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["command"], "check");
    assert_eq!(json["summary"]["files"], 1);

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
