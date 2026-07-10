mod common;

use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn audit_ignores_unchanged_existing_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'live.dart';\nvoid main() { Live(); }\n",
    )?;
    write(&fixture, "lib/live.dart", "class Live {}\n")?;
    write(&fixture, "lib/dead.dart", "class Dead {}\n")?;
    commit_all(&fixture)?;
    write(
        &fixture,
        "lib/live.dart",
        "class Live {\n  void touched() {}\n}\n",
    )?;
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
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["command"], "audit");
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["risk_score"], 0);
    assert_eq!(json["summary"]["risk_level"], "pass");
    assert_eq!(json["summary"]["attribution"]["introduced"]["findings"], 0);
    assert_eq!(
        json["summary"]["attribution"]["pre_existing"]["findings"],
        0
    );
    assert_eq!(json["summary"]["dead_files"], 0);
    assert_eq!(json["summary"]["findings"], 0);
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));
    assert!(json["next_steps"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn audit_reports_findings_on_changed_files() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    commit_all(&fixture)?;
    write(&fixture, "lib/new_dead.dart", "class NewDead {}\n")?;
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
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["command"], "audit");
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["risk_score"], 40);
    assert_eq!(json["summary"]["risk_level"], "fail");
    assert_eq!(json["summary"]["attribution"]["introduced"]["findings"], 1);
    assert_eq!(
        json["summary"]["attribution"]["introduced"]["error_findings"],
        1
    );
    assert_eq!(
        json["summary"]["attribution"]["pre_existing"]["findings"],
        0
    );
    assert_eq!(json["summary"]["dead_files"], 1);
    assert_eq!(json["summary"]["findings"], 1);
    assert_eq!(json["findings"][0]["rule_id"], "dart-decimate/dead-file");
    assert_eq!(json["findings"][0]["path"], "lib/new_dead.dart");

    let mut new_only_output = Vec::new();
    let new_only_code = run_from(
        [
            "dart-decimate",
            "audit",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--base",
            "HEAD",
            "--gate",
            "new-only",
            "--entry",
            "lib/main.dart",
        ],
        &mut new_only_output,
    )?;
    let new_only_json = serde_json::from_slice::<Value>(&new_only_output)?;
    assert_eq!(new_only_code, 1);
    assert_eq!(new_only_json["summary"]["risk_level"], "fail");

    Ok(())
}

#[test]
fn audit_attributes_existing_changed_file_findings_as_pre_existing()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(&fixture, "lib/dead.dart", "class Dead {}\n")?;
    commit_all(&fixture)?;
    write(&fixture, "lib/dead.dart", "// touched\nclass Dead {}\n")?;
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
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["risk_score"], 12);
    assert_eq!(json["summary"]["risk_level"], "warn");
    assert_eq!(json["summary"]["attribution"]["introduced"]["findings"], 0);
    assert_eq!(
        json["summary"]["attribution"]["pre_existing"]["findings"],
        1
    );
    assert_eq!(
        json["summary"]["attribution"]["pre_existing"]["error_findings"],
        1
    );
    assert_eq!(
        json["summary"]["attribution"]["pre_existing"]["safe_to_delete"],
        1
    );
    assert_eq!(json["findings"][0]["path"], "lib/dead.dart");

    let mut new_only_output = Vec::new();
    let new_only_code = run_from(
        [
            "dart-decimate",
            "audit",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--base",
            "HEAD",
            "--gate",
            "new-only",
            "--entry",
            "lib/main.dart",
        ],
        &mut new_only_output,
    )?;
    let new_only_json = serde_json::from_slice::<Value>(&new_only_output)?;
    assert_eq!(new_only_code, 0);
    assert_eq!(new_only_json["summary"]["risk_level"], "warn");
    assert_eq!(
        new_only_json["summary"]["attribution"]["introduced"]["findings"],
        0
    );

    Ok(())
}

#[test]
fn audit_dead_code_baseline_suppresses_known_changed_findings()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    commit_all(&fixture)?;
    write(&fixture, "lib/new_dead.dart", "class NewDead {}\n")?;
    let baseline = fixture.path().join("dead-code-baseline.json");
    let baseline_arg = baseline.display().to_string();
    let mut baseline_output = Vec::new();

    let baseline_code = run_from(
        [
            "dart-decimate",
            "dead-code",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
            "--save-baseline",
            baseline_arg.as_str(),
        ],
        &mut baseline_output,
    )?;
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
            "--entry",
            "lib/main.dart",
            "--dead-code-baseline",
            baseline_arg.as_str(),
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(baseline_code, 1);
    assert_eq!(code, 0);
    assert_eq!(json["command"], "audit");
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["findings"], 0);
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn audit_keeps_related_findings_when_changed_file_participates()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'a.dart';\nvoid main() {}\n",
    )?;
    write(&fixture, "lib/a.dart", "import 'b.dart';\nclass A {}\n")?;
    write(&fixture, "lib/b.dart", "import 'a.dart';\nclass B {}\n")?;
    commit_all(&fixture)?;
    write(
        &fixture,
        "lib/a.dart",
        "import 'b.dart';\nclass A {\n  void touched() {}\n}\n",
    )?;
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
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["cycles"], 1);
    assert_eq!(json["summary"]["risk_score"], 10);
    assert_eq!(json["summary"]["risk_level"], "warn");
    assert_eq!(json["summary"]["attribution"]["introduced"]["findings"], 0);
    assert_eq!(
        json["summary"]["attribution"]["pre_existing"]["findings"],
        1
    );
    assert_eq!(
        json["findings"][0]["rule_id"],
        "dart-decimate/circular-dependency"
    );
    assert_eq!(json["findings"][0]["files"][0], "lib/a.dart");
    assert_eq!(json["findings"][0]["files"][1], "lib/b.dart");

    Ok(())
}

#[test]
fn audit_expands_scope_to_one_hop_related_files() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'a.dart';\nvoid main() {}\n",
    )?;
    write(&fixture, "lib/a.dart", "import 'b.dart';\nclass A {}\n")?;
    write(
        &fixture,
        "lib/b.dart",
        "int related(int value) {\n  if (value > 0) return 1;\n  return 0;\n}\n",
    )?;
    commit_all(&fixture)?;
    write(
        &fixture,
        "lib/a.dart",
        "import 'b.dart';\nclass A {\n  void touched() {}\n}\n",
    )?;
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
            "--max-cyclomatic",
            "1",
            "--max-cognitive",
            "99",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["complex_functions"], 1);
    assert_eq!(json["summary"]["risk_score"], 10);
    assert_eq!(json["summary"]["risk_level"], "warn");
    assert_eq!(json["summary"]["attribution"]["introduced"]["findings"], 0);
    assert_eq!(
        json["summary"]["attribution"]["pre_existing"]["findings"],
        1
    );
    assert_eq!(
        json["summary"]["attribution"]["pre_existing"]["error_findings"],
        1
    );
    assert_eq!(
        json["findings"][0]["rule_id"],
        "dart-decimate/high-cyclomatic-complexity"
    );
    assert_eq!(json["findings"][0]["path"], "lib/b.dart");

    let mut new_only_output = Vec::new();
    let new_only_code = run_from(
        [
            "dart-decimate",
            "audit",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--base",
            "HEAD",
            "--gate",
            "new-only",
            "--max-cyclomatic",
            "1",
            "--max-cognitive",
            "99",
        ],
        &mut new_only_output,
    )?;
    let new_only_json = serde_json::from_slice::<Value>(&new_only_output)?;
    assert_eq!(new_only_code, 0);
    assert_eq!(new_only_json["verdict"], "fail");
    assert_eq!(new_only_json["summary"]["risk_level"], "warn");
    assert_eq!(
        new_only_json["summary"]["attribution"]["pre_existing"]["findings"],
        1
    );

    Ok(())
}

#[test]
fn audit_expands_scope_through_augment_edges() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(
        &fixture,
        "lib/base.dart",
        "int related(int value) {\n  if (value > 0) return 1;\n  return 0;\n}\n",
    )?;
    commit_all(&fixture)?;
    write(
        &fixture,
        "lib/base_augment.dart",
        "library augment 'base.dart';\n",
    )?;
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
            "--max-cyclomatic",
            "1",
            "--max-cognitive",
            "99",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["complex_functions"], 1);
    assert_eq!(json["findings"][0]["path"], "lib/base.dart");

    Ok(())
}

#[test]
fn audit_errors_for_invalid_base() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    commit_all(&fixture)?;
    let mut output = Vec::new();

    let error = match run_from(
        [
            "dart-decimate",
            "audit",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--base",
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

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
