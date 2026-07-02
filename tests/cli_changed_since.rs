use std::fs;
use std::process::Command;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn changed_since_scopes_dead_code_to_changed_files() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write_project(&fixture)?;
    write(&fixture, "lib/old_dead.dart", "class OldDead {}\n")?;
    commit_all(&fixture)?;
    write(&fixture, "lib/new_dead.dart", "class NewDead {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "dead-code",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
            "--changed-since",
            "HEAD",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let paths = finding_paths(&json);
    assert_eq!(code, 1);
    assert_eq!(paths, vec!["lib/new_dead.dart"]);
    assert_eq!(json["summary"]["dead_files"], 1);

    Ok(())
}

#[test]
fn compare_alias_scopes_dead_code_to_changed_files() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write_project(&fixture)?;
    commit_all(&fixture)?;
    write(&fixture, "lib/new_dead.dart", "class NewDead {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "html",
            fixture.path().to_str().unwrap_or("."),
            "--stdout",
            "--entry",
            "lib/main.dart",
            "--compare",
            "HEAD",
        ],
        &mut output,
    )?;

    let html = String::from_utf8(output)?;
    assert_eq!(code, 1);
    assert!(html.contains("lib/new_dead.dart"));

    Ok(())
}

#[test]
fn changed_since_errors_for_invalid_base() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write_project(&fixture)?;
    commit_all(&fixture)?;
    let mut output = Vec::new();

    let error = match run_from(
        [
            "dart-decimate",
            "dead-code",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
            "--changed-since",
            "missing-ref",
        ],
        &mut output,
    ) {
        Ok(code) => panic!("invalid git base should fail, got exit code {code}"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("missing-ref"));
    Ok(())
}

#[test]
fn compare_suggests_similar_branch_for_invalid_base() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write_project(&fixture)?;
    commit_all(&fixture)?;
    git(&fixture, ["update-ref", "refs/remotes/origin/main", "HEAD"])?;
    let mut output = Vec::new();

    let error = match run_from(
        [
            "dart-decimate",
            "dead-code",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
            "--compare",
            "orign/main",
        ],
        &mut output,
    ) {
        Ok(code) => panic!("invalid git base should fail, got exit code {code}"),
        Err(error) => error,
    };

    let message = error.to_string();
    assert!(message.contains("orign/main"));
    assert!(message.contains("Did you mean origin/main?"));
    Ok(())
}

#[test]
fn changed_since_is_listed_for_report_commands() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(["dart-decimate", "schema", "--format", "json"], &mut output)?;

    let json = serde_json::from_slice::<Value>(&output)?;
    for name in [
        "check",
        "audit",
        "dead-code",
        "cycles",
        "dupes",
        "health",
        "flags",
        "security",
        "fix",
    ] {
        let flags = command_flags(&json, name)?;
        assert!(
            flags.contains(&"--changed-since"),
            "{name} should publish --changed-since"
        );
        assert!(
            flags.contains(&"--compare"),
            "{name} should publish --compare"
        );
    }
    assert_eq!(code, 0);
    Ok(())
}

fn write_project(fixture: &TempDir) -> Result<(), std::io::Error> {
    write(fixture, "pubspec.yaml", "name: app\n")?;
    write(
        fixture,
        "lib/main.dart",
        "import 'live.dart';\nvoid main() { live(); }\n",
    )?;
    write(fixture, "lib/live.dart", "void live() {}\n")
}

fn command_flags<'json>(
    json: &'json Value,
    name: &str,
) -> Result<Vec<&'json str>, Box<dyn std::error::Error>> {
    let commands = json["commands"].as_array().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "commands should be an array",
        )
    })?;
    let command = commands
        .iter()
        .find(|command| command["name"] == name)
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("{name} command should be listed"),
            )
        })?;
    Ok(command["flags"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect())
}

fn git_fixture() -> Result<TempDir, Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    git(&fixture, ["init", "-q"])?;
    git(
        &fixture,
        ["config", "user.email", "dart-decimate@example.com"],
    )?;
    git(&fixture, ["config", "user.name", "Dart Decimate Tests"])?;
    Ok(fixture)
}

fn commit_all(fixture: &TempDir) -> Result<(), Box<dyn std::error::Error>> {
    git(fixture, ["add", "."])?;
    git(fixture, ["commit", "-m", "initial", "-q"])
}

fn git<const N: usize>(
    fixture: &TempDir,
    args: [&str; N],
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(fixture.path())
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )
    .into())
}

fn finding_paths(json: &Value) -> Vec<&str> {
    json["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|finding| finding["path"].as_str())
        .collect()
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
