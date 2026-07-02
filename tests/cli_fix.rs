use std::collections::BTreeSet;
use std::fs;
use std::process::Command;

use dart_decimate::cli::run_from;
use dart_decimate::output::{Finding, FindingAction, FindingKind, Severity};
use dart_decimate::{FixMode, fix_findings};
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn fix_command_dry_run_reports_safe_plan_without_writing() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = fix_fixture()?;

    let (code, json) = run_json([
        "dart-decimate",
        "fix",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "dart-decimate.fix.v1");
    assert_eq!(json["kind"], "fix");
    assert_eq!(json["mode"], "dry-run");
    assert_eq!(json["summary"]["planned"], 2);
    assert_eq!(json["summary"]["applied"], 0);
    assert_eq!(json["summary"]["skipped"], 0);
    assert_eq!(
        actions(&json),
        BTreeSet::from(["delete-file", "remove-suppression"])
    );
    assert!(fixture.path().join("lib/dead.dart").exists());
    assert!(
        fs::read_to_string(fixture.path().join("lib/main.dart"))?
            .contains("dart-decimate-ignore-next-line")
    );

    Ok(())
}

#[test]
fn fix_command_accepts_fallow_dry_run_alias() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = fix_fixture()?;

    let (code, json) = run_json([
        "dart-decimate",
        "fix",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--dry-run",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["mode"], "dry-run");
    assert_eq!(json["summary"]["planned"], 2);
    assert!(fixture.path().join("lib/dead.dart").exists());

    Ok(())
}

#[test]
fn fix_command_applies_confirmed_safe_changes() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = fix_fixture()?;

    let (code, json) = run_json([
        "dart-decimate",
        "fix",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--apply",
        "--confirm",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["mode"], "apply");
    assert_eq!(json["summary"]["planned"], 2);
    assert_eq!(json["summary"]["applied"], 2);
    assert_eq!(json["summary"]["skipped"], 0);
    assert!(!fixture.path().join("lib/dead.dart").exists());
    assert_eq!(
        fs::read_to_string(fixture.path().join("lib/main.dart"))?,
        "void main() {}\n"
    );

    let (check_code, check_json) = run_json([
        "dart-decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
    ])?;
    assert_eq!(check_code, 0);
    assert_eq!(check_json["summary"]["findings"], 0);

    Ok(())
}

#[test]
fn fix_command_applies_with_fallow_yes_alias() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = fix_fixture()?;

    let (code, json) = run_json([
        "dart-decimate",
        "fix",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--yes",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["mode"], "apply");
    assert_eq!(json["summary"]["planned"], 2);
    assert_eq!(json["summary"]["applied"], 2);
    assert!(!fixture.path().join("lib/dead.dart").exists());

    Ok(())
}

#[test]
fn fix_command_applies_compare_scope_to_changed_files() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(&fixture, "lib/old_dead.dart", "void oldDead() {}\n")?;
    commit_all(&fixture)?;
    write(&fixture, "lib/new_dead.dart", "void newDead() {}\n")?;

    let (code, json) = run_json([
        "dart-decimate",
        "fix",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--compare",
        "HEAD",
        "--apply",
        "--confirm",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["summary"]["planned"], 1);
    assert_eq!(json["summary"]["applied"], 1);
    assert!(fixture.path().join("lib/old_dead.dart").exists());
    assert!(!fixture.path().join("lib/new_dead.dart").exists());

    Ok(())
}

#[test]
fn fix_command_filters_by_action() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = fix_fixture()?;

    let (code, json) = run_json([
        "dart-decimate",
        "fix",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--action",
        "remove-suppression",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["summary"]["planned"], 1);
    assert_eq!(actions(&json), BTreeSet::from(["remove-suppression"]));

    Ok(())
}

#[test]
fn fix_findings_removes_safe_unused_pub_dependency_entry() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  collection: ^1.18.0\n  http: ^1.2.0\n",
    )?;
    let finding = unused_pub_dependency(
        FindingKind::UnusedDependency,
        "collection",
        "dependencies",
        3,
        true,
    );

    let dry_run = fix_findings(
        fixture.path(),
        std::slice::from_ref(&finding),
        &BTreeSet::new(),
        FixMode::DryRun,
    );
    assert_eq!(dry_run.summary.planned, 1);
    assert_eq!(dry_run.summary.applied, 0);
    assert_eq!(dry_run.summary.skipped, 0);
    assert!(fs::read_to_string(fixture.path().join("pubspec.yaml"))?.contains("collection"));

    let apply = fix_findings(
        fixture.path(),
        std::slice::from_ref(&finding),
        &BTreeSet::new(),
        FixMode::Apply,
    );
    assert_eq!(apply.summary.planned, 1);
    assert_eq!(apply.summary.applied, 1);
    assert_eq!(apply.summary.skipped, 0);
    assert_eq!(
        fs::read_to_string(fixture.path().join("pubspec.yaml"))?,
        "name: app\n\
dependencies:\n  http: ^1.2.0\n"
    );

    Ok(())
}

#[test]
fn fix_findings_removes_safe_unused_dev_dependency_entry() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dev_dependencies:\n  mockito: ^5.0.0\n  lints: ^5.0.0\n",
    )?;
    let finding = unused_pub_dependency(
        FindingKind::UnusedDevDependency,
        "mockito",
        "dev_dependencies",
        3,
        true,
    );

    let apply = fix_findings(
        fixture.path(),
        std::slice::from_ref(&finding),
        &BTreeSet::new(),
        FixMode::Apply,
    );

    assert_eq!(apply.summary.planned, 1);
    assert_eq!(apply.summary.applied, 1);
    assert_eq!(apply.summary.skipped, 0);
    assert_eq!(
        fs::read_to_string(fixture.path().join("pubspec.yaml"))?,
        "name: app\n\
dev_dependencies:\n  lints: ^5.0.0\n"
    );

    Ok(())
}

#[test]
fn fix_findings_skips_unused_pub_dependency_without_safe_to_delete()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  collection: ^1.18.0\n",
    )?;
    let finding = unused_pub_dependency(
        FindingKind::UnusedDependency,
        "collection",
        "dependencies",
        3,
        false,
    );

    let report = fix_findings(
        fixture.path(),
        std::slice::from_ref(&finding),
        &BTreeSet::new(),
        FixMode::Apply,
    );

    assert_eq!(report.summary.planned, 0);
    assert_eq!(report.summary.applied, 0);
    assert_eq!(report.summary.skipped, 1);
    assert_eq!(
        report.skipped[0].reason,
        "unused dependency finding is not marked safe_to_delete"
    );
    assert!(fs::read_to_string(fixture.path().join("pubspec.yaml"))?.contains("collection"));

    Ok(())
}

#[test]
fn fix_findings_does_not_treat_review_dependency_action_as_mutation()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  collection: ^1.18.0\n",
    )?;
    let mut finding = unused_pub_dependency(
        FindingKind::UnusedDependency,
        "collection",
        "dependencies",
        3,
        true,
    );
    finding.actions[0].action = "review-pubspec-dependency".to_owned();

    let report = fix_findings(
        fixture.path(),
        std::slice::from_ref(&finding),
        &BTreeSet::new(),
        FixMode::Apply,
    );

    assert_eq!(report.summary.planned, 0);
    assert_eq!(report.summary.applied, 0);
    assert_eq!(report.summary.skipped, 1);
    assert_eq!(
        report.skipped[0].reason,
        "unsupported safe fix action review-pubspec-dependency"
    );
    assert!(fs::read_to_string(fixture.path().join("pubspec.yaml"))?.contains("collection"));

    Ok(())
}

#[test]
fn fix_findings_skips_nested_or_commented_pub_dependency_entries()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  local:\n    path: ../local\n  commented: ^1.0.0 # keep\n",
    )?;
    let nested = unused_pub_dependency(
        FindingKind::UnusedDependency,
        "local",
        "dependencies",
        3,
        true,
    );
    let commented = unused_pub_dependency(
        FindingKind::UnusedDependency,
        "commented",
        "dependencies",
        5,
        true,
    );

    let report = fix_findings(
        fixture.path(),
        &[nested, commented],
        &BTreeSet::new(),
        FixMode::Apply,
    );

    assert_eq!(report.summary.planned, 0);
    assert_eq!(report.summary.applied, 0);
    assert_eq!(report.summary.skipped, 2);
    let reasons = report
        .skipped
        .iter()
        .map(|skip| skip.reason.as_str())
        .collect::<BTreeSet<_>>();
    assert!(reasons.contains("nested dependency entries are not auto-fixable"));
    assert!(reasons.contains("dependency entries with comments are not auto-fixable"));
    assert!(fs::read_to_string(fixture.path().join("pubspec.yaml"))?.contains("path: ../local"));

    Ok(())
}

#[test]
fn fix_command_applies_safe_unused_pub_dependency_from_check_output()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  collection: ^1.18.0\n",
    )?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;

    let (code, json) = run_json([
        "dart-decimate",
        "fix",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--apply",
        "--confirm",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["summary"]["planned"], 1);
    assert_eq!(json["summary"]["applied"], 1);
    assert_eq!(json["summary"]["skipped"], 0);
    assert_eq!(
        fs::read_to_string(fixture.path().join("pubspec.yaml"))?,
        "name: app\n\
dependencies:\n"
    );

    Ok(())
}

#[test]
fn fix_command_applies_safe_one_line_unused_symbol_from_check_output()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/live.dart';\nvoid main() {}\n",
    )?;
    write(&fixture, "lib/src/live.dart", "class Unused {}\n")?;

    let (code, json) = run_json([
        "dart-decimate",
        "fix",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--apply",
        "--confirm",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["summary"]["planned"], 1);
    assert_eq!(json["summary"]["applied"], 1);
    assert_eq!(json["summary"]["skipped"], 0);
    assert_eq!(actions(&json), BTreeSet::from(["remove-declaration"]));
    assert_eq!(
        fs::read_to_string(fixture.path().join("lib/src/live.dart"))?,
        ""
    );

    Ok(())
}

fn fix_fixture() -> Result<TempDir, std::io::Error> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "// dart-decimate-ignore-next-line dead-file\nvoid main() {}\n",
    )?;
    write(&fixture, "lib/dead.dart", "void dead() {}\n")?;
    Ok(fixture)
}

fn unused_pub_dependency(
    kind: FindingKind,
    dependency: &str,
    config_key: &str,
    line: usize,
    safe_to_delete: bool,
) -> Finding {
    let rule_id = if kind == FindingKind::UnusedDevDependency {
        "dart-decimate/unused-dev-dependency"
    } else {
        "dart-decimate/unused-dependency"
    };
    Finding {
        rule_id: rule_id.to_owned(),
        fingerprint: None,
        kind,
        severity: Severity::Error,
        message: format!("app declares unused pub dependency {dependency}"),
        path: "pubspec.yaml".to_owned(),
        line,
        column: 2,
        safe_to_delete,
        files: Vec::new(),
        edge: None,
        actions: vec![
            FindingAction::new(
                "remove-pubspec-dependency",
                "Remove this simple unused pubspec dependency",
                true,
            )
            .with_target_path("pubspec.yaml")
            .with_target_dependency(dependency)
            .with_config_key(config_key),
        ],
    }
}

fn actions(json: &Value) -> BTreeSet<&str> {
    json["fixes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|fix| fix["action"].as_str())
        .collect()
}

fn run_json<const N: usize>(args: [&str; N]) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(args, &mut output)?;
    Ok((code, serde_json::from_slice::<Value>(&output)?))
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

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
