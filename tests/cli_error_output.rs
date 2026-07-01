use std::fs;
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

#[test]
fn binary_emits_json_error_for_missing_entry_points() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/src/hidden.dart", "class Hidden {}\n")?;

    let output = Command::new(env!("CARGO_BIN_EXE_dart-decimate"))
        .args([
            "dead-code",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--summary",
            "--production",
        ])
        .output()?;

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let json = serde_json::from_slice::<Value>(&output.stdout)?;
    assert_eq!(json["error"], true);
    assert_eq!(json["exit_code"], 2);
    assert!(
        json["message"]
            .as_str()
            .is_some_and(|message| message.contains("no entry points provided"))
    );

    Ok(())
}

#[test]
fn binary_emits_json_error_for_missing_runtime_coverage() -> Result<(), Box<dyn std::error::Error>>
{
    let output = Command::new(env!("CARGO_BIN_EXE_dart-decimate"))
        .args(["coverage", "analyze", "--format", "json"])
        .output()?;

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let json = serde_json::from_slice::<Value>(&output.stdout)?;
    assert_eq!(json["error"], true);
    assert_eq!(
        json["message"],
        "coverage analyze requires --runtime-coverage PATH"
    );
    assert_eq!(json["exit_code"], 2);

    Ok(())
}

#[test]
fn binary_emits_json_error_for_malformed_config() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".dart-decimaterc",
        "[health]\nmax_cyclomatic = \"low\"\n",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;

    let output = Command::new(env!("CARGO_BIN_EXE_dart-decimate"))
        .args([
            "health",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ])
        .output()?;

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let json = serde_json::from_slice::<Value>(&output.stdout)?;
    assert_eq!(json["error"], true);
    assert_eq!(json["exit_code"], 2);
    assert!(json["message"].as_str().is_some_and(|message| {
        message.contains(".dart-decimaterc") && message.contains("max_cyclomatic")
    }));

    Ok(())
}

#[test]
fn binary_emits_json_error_for_clap_errors() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_dart-decimate"))
        .args(["dead-code", "--format", "json", "--unknown"])
        .output()?;

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let json = serde_json::from_slice::<Value>(&output.stdout)?;
    assert_eq!(json["error"], true);
    assert_eq!(json["exit_code"], 2);
    assert!(
        json["message"]
            .as_str()
            .is_some_and(|message| message.contains("unexpected argument '--unknown'"))
    );

    Ok(())
}

#[test]
fn binary_emits_json_error_for_json_shortcut_clap_errors() -> Result<(), Box<dyn std::error::Error>>
{
    let output = Command::new(env!("CARGO_BIN_EXE_dart-decimate"))
        .args(["json", "--bad-flag"])
        .output()?;

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let json = serde_json::from_slice::<Value>(&output.stdout)?;
    assert_eq!(json["error"], true);
    assert_eq!(json["exit_code"], 2);
    assert!(
        json["message"]
            .as_str()
            .is_some_and(|message| message.contains("unexpected argument '--bad-flag'"))
    );

    Ok(())
}

#[test]
fn json_shortcut_help_with_format_stays_human() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_dart-decimate"))
        .args(["json", "--format", "json", "--help"])
        .output()?;

    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert!(serde_json::from_slice::<Value>(&output.stdout).is_err());
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("Shortcut for check with JSON output"));
    assert!(stdout.contains("Usage: dart-decimate json [ROOT]"));

    Ok(())
}

#[test]
fn json_shortcut_missing_format_value_is_json_error() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_dart-decimate"))
        .args(["json", ".", "--format"])
        .output()?;

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let json = serde_json::from_slice::<Value>(&output.stdout)?;
    assert_eq!(json["error"], true);
    assert_eq!(json["exit_code"], 2);
    assert!(
        json["message"]
            .as_str()
            .is_some_and(|message| message.contains("--format") && message.contains("required"))
    );

    Ok(())
}

#[test]
fn json_shortcut_invalid_format_value_is_json_error() -> Result<(), Box<dyn std::error::Error>> {
    let cases: &[&[&str]] = &[
        &["json", ".", "--format", "xml"],
        &["json", ".", "--format=xml"],
        &["json", ".", "--file", "lib/main.dart", "--format", "xml"],
        &["json", ".", "--file", "lib/main.dart", "--format=xml"],
    ];

    for args in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_dart-decimate"))
            .args(*args)
            .output()?;

        assert_eq!(output.status.code(), Some(2));
        assert!(output.stderr.is_empty());
        let json = serde_json::from_slice::<Value>(&output.stdout)?;
        assert_eq!(json["error"], true);
        assert_eq!(json["exit_code"], 2);
        assert!(json["message"].as_str().is_some_and(|message| {
            message.contains("invalid value") && message.contains("xml")
        }));
    }

    Ok(())
}

#[test]
fn json_shortcut_missing_format_with_check_flags_is_json_error()
-> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_dart-decimate"))
        .args(["json", ".", "--file", "lib/main.dart", "--format"])
        .output()?;

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let json = serde_json::from_slice::<Value>(&output.stdout)?;
    assert_eq!(json["error"], true);
    assert_eq!(json["exit_code"], 2);
    assert!(
        json["message"]
            .as_str()
            .is_some_and(|message| message.contains("--format") && message.contains("required"))
    );

    Ok(())
}

#[test]
fn output_shortcut_invalid_format_with_check_flags_reports_format_error()
-> Result<(), Box<dyn std::error::Error>> {
    for alias in ["human", "html"] {
        let output = Command::new(env!("CARGO_BIN_EXE_dart-decimate"))
            .args([alias, ".", "--file", "lib/main.dart", "--format", "xml"])
            .output()?;

        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr)?;
        assert!(stderr.contains("invalid value"));
        assert!(stderr.contains("xml"));
        assert!(!stderr.contains("unexpected argument '--file'"));
    }

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
