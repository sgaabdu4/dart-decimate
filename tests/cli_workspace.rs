mod common;

use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn workspace_scope_matches_package_name() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_workspace(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "dead-code",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "packages/app/lib/main.dart",
            "--workspace",
            "app",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["dead_files"], 1);
    assert_eq!(json["findings"][0]["path"], "packages/app/lib/dead.dart");
    assert!(findings_only_under(&json, "packages/app"));

    Ok(())
}

#[test]
fn workspace_scope_matches_path_glob_and_exclude() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_workspace(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "dead-code",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "packages/app/lib/main.dart",
            "--workspace",
            "packages/*,!packages/shared",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["dead_files"], 1);
    assert!(findings_only_under(&json, "packages/app"));

    Ok(())
}

#[test]
fn workspace_scope_keeps_pubspec_dependency_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_workspace(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "packages/app/lib/main.dart",
            "--workspace",
            "app",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let paths = finding_paths(&json);
    assert_eq!(code, 1);
    assert!(paths.contains(&"packages/app/pubspec.yaml".to_owned()));
    assert!(!paths.contains(&"packages/shared/pubspec.yaml".to_owned()));

    Ok(())
}

#[test]
fn list_workspace_scope_filters_file_and_package_metadata() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write_workspace(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "list",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--workspace",
            "shared",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["files"], 2);
    assert_eq!(json["summary"]["workspaces"], 1);
    assert_eq!(json["workspaces"][0]["name"], "shared");
    assert!(
        json["files"]
            .as_array()
            .is_some_and(|files| files.iter().all(|file| file["path"]
                .as_str()
                .is_some_and(|path| path.starts_with("packages/shared/"))))
    );

    Ok(())
}

#[test]
fn workspace_scope_errors_when_no_package_matches() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_workspace(&fixture)?;
    let mut output = Vec::new();

    let Err(error) = run_from(
        [
            "dart-decimate",
            "dead-code",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "packages/app/lib/main.dart",
            "--workspace",
            "missing",
        ],
        &mut output,
    ) else {
        panic!("missing workspace should fail");
    };

    assert!(error.to_string().contains("no local pub packages matched"));
    Ok(())
}

#[test]
fn changed_workspaces_scope_selects_changed_package() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write_workspace(&fixture)?;
    commit_all(&fixture)?;
    write(
        &fixture,
        "packages/shared/lib/shared.dart",
        "class Shared { void touched() {} }\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "dead-code",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "packages/app/lib/main.dart",
            "--changed-workspaces",
            "HEAD",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["files"], 2);
    assert!(findings_only_under(&json, "packages/shared"));
    assert!(
        finding_paths(&json)
            .iter()
            .any(|path| path == "packages/shared/lib/unused.dart")
    );

    Ok(())
}

#[test]
fn changed_workspaces_empty_when_no_package_changed() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write_workspace(&fixture)?;
    commit_all(&fixture)?;
    write(&fixture, "README.md", "changed\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "dead-code",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "packages/app/lib/main.dart",
            "--changed-workspaces",
            "HEAD",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["files"], 0);
    assert_eq!(json["summary"]["findings"], 0);
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn changed_workspaces_conflicts_with_workspace() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_workspace(&fixture)?;
    let mut output = Vec::new();

    let error = match run_from(
        [
            "dart-decimate",
            "dead-code",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "packages/app/lib/main.dart",
            "--workspace",
            "app",
            "--changed-workspaces",
            "HEAD",
        ],
        &mut output,
    ) {
        Ok(code) => panic!("conflicting scope flags should fail, got exit code {code}"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("cannot be used with"));
    Ok(())
}

#[test]
fn changed_workspaces_errors_for_invalid_base() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write_workspace(&fixture)?;
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
            "packages/app/lib/main.dart",
            "--changed-workspaces",
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

fn write_workspace(fixture: &TempDir) -> Result<(), std::io::Error> {
    write(
        fixture,
        "pubspec.yaml",
        "name: root
workspace:
  - packages/*
",
    )?;
    write(
        fixture,
        "packages/app/pubspec.yaml",
        "name: app
dependencies:
  shared:
    path: ../shared
  unused_app: ^1.0.0
",
    )?;
    write(
        fixture,
        "packages/shared/pubspec.yaml",
        "name: shared
dependencies:
  unused_shared: ^1.0.0
",
    )?;
    write(
        fixture,
        "packages/app/lib/main.dart",
        "import 'package:shared/shared.dart';\nvoid main() { Shared(); }\n",
    )?;
    write(fixture, "packages/app/lib/dead.dart", "class AppDead {}\n")?;
    write(
        fixture,
        "packages/shared/lib/shared.dart",
        "class Shared {}\n",
    )?;
    write(
        fixture,
        "packages/shared/lib/unused.dart",
        "class SharedDead {}\n",
    )
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
    let output = common::isolated_git_command()
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

fn findings_only_under(json: &Value, prefix: &str) -> bool {
    finding_paths(json)
        .iter()
        .all(|path| path.starts_with(prefix))
}

fn finding_paths(json: &Value) -> Vec<String> {
    json["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|finding| finding["path"].as_str())
        .map(ToOwned::to_owned)
        .collect()
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
