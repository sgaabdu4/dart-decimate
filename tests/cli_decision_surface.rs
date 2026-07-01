use std::fs;
use std::process::Command;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn decision_surface_reports_changed_public_api_and_coupling_decisions()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/app.dart", "export 'domain/service.dart';\n")?;
    write(
        &fixture,
        "lib/domain/service.dart",
        "import '../ui/widget.dart';\nclass Service {}\n",
    )?;
    write(&fixture, "lib/ui/widget.dart", "class WidgetApi {}\n")?;
    commit_all(&fixture)?;
    write(
        &fixture,
        "lib/domain/service.dart",
        "import '../ui/widget.dart';\nclass Service { void touched() {} }\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "decision-surface",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--base",
            "HEAD",
            "--max-decisions",
            "10",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "dart-decimate.decision-surface.v1");
    assert_eq!(json["kind"], "decision-surface");
    assert_eq!(json["summary"]["changed_files"], 1);
    assert!(has_category(&json, "coupling-boundary"));
    assert!(has_category(&json, "public-api-contract"));
    assert!(json["decisions"].as_array().is_some_and(|decisions| {
        decisions.iter().any(|decision| {
            decision["question"]
                .as_str()
                .is_some_and(|question| question.contains("lib/app.dart"))
        })
    }));

    Ok(())
}

#[test]
fn decision_surface_reports_changed_pubspec_dependency_decision()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    commit_all(&fixture)?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  http: ^1.0.0\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "decision-surface",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--base",
            "HEAD",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["decisions"], 1);
    assert!(has_category(&json, "dependency"));
    assert_eq!(json["decisions"][0]["path"], "pubspec.yaml");

    Ok(())
}

#[test]
fn decision_surface_treats_nested_non_src_libraries_as_public_api()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(&fixture, "lib/widgets/button.dart", "class Button {}\n")?;
    commit_all(&fixture)?;
    write(
        &fixture,
        "lib/widgets/button.dart",
        "class Button { void touched() {} }\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "decision-surface",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--base",
            "HEAD",
            "--max-decisions",
            "10",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert!(has_category(&json, "public-api-contract"));
    assert!(json["decisions"].as_array().is_some_and(|decisions| {
        decisions.iter().any(|decision| {
            decision["path"] == "lib/widgets/button.dart"
                && decision["question"]
                    .as_str()
                    .is_some_and(|question| question.contains("public library"))
        })
    }));

    Ok(())
}

#[test]
fn review_command_emits_decision_surface_contract() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = changed_dependency_fixture()?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "review",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--base",
            "HEAD",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "dart-decimate.decision-surface.v1");
    assert_eq!(json["kind"], "decision-surface");
    assert_eq!(json["command"], "review");
    assert_eq!(json["summary"]["decisions"], 1);
    assert!(has_category(&json, "dependency"));

    Ok(())
}

#[test]
fn audit_brief_emits_review_contract_and_never_fails() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = changed_dependency_fixture()?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "audit",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--base",
            "HEAD",
            "--brief",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "dart-decimate.decision-surface.v1");
    assert_eq!(json["kind"], "decision-surface");
    assert_eq!(json["command"], "audit --brief");
    assert_eq!(json["summary"]["decisions"], 1);
    assert!(has_category(&json, "dependency"));

    Ok(())
}

#[test]
fn audit_brief_emits_html_report() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = changed_dependency_fixture()?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "audit",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "html",
            "--base",
            "HEAD",
            "--brief",
        ],
        &mut output,
    )?;

    let html = String::from_utf8(output)?;
    assert_eq!(code, 0);
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("<h1>audit --brief decision surface</h1>"));
    assert!(html.contains("pubspec.yaml"));
    assert!(html.contains("dependency"));

    Ok(())
}

fn has_category(json: &Value, category: &str) -> bool {
    json["decisions"].as_array().is_some_and(|decisions| {
        decisions
            .iter()
            .any(|decision| decision["category"] == category)
    })
}

fn changed_dependency_fixture() -> Result<TempDir, Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    commit_all(&fixture)?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  http: ^1.0.0\n",
    )?;
    Ok(fixture)
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
    git(fixture, ["commit", "-m", "baseline", "-q"])?;
    Ok(())
}

fn git<const N: usize>(
    fixture: &TempDir,
    args: [&str; N],
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(fixture.path())
        .output()?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string().into());
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
