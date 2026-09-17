use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn file_scope_reports_only_named_file_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_basic_project(&fixture)?;
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
            "--file",
            "lib/dead.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["files"], 1);
    assert_eq!(json["summary"]["dead_files"], 1);
    assert_eq!(json["findings"][0]["path"], "lib/dead.dart");

    Ok(())
}

#[test]
fn file_scope_suppresses_unselected_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_basic_project(&fixture)?;
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
            "--file",
            "lib/live.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["files"], 1);
    assert_eq!(json["summary"]["findings"], 0);
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn file_scope_suppresses_project_wide_dependency_findings() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  unused: ^1.0.0\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:missing/missing.dart';\nvoid main() {}\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
            "--file",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let rule_ids = json["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|finding| finding["rule_id"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(code, 1);
    assert!(rule_ids.contains(&"dart-decimate/unlisted-dependency"));
    assert!(!rule_ids.contains(&"dart-decimate/unused-dependency"));
    assert_eq!(json["summary"]["unused_dependencies"], 0);
    assert_eq!(json["summary"]["unlisted_dependencies"], 1);

    Ok(())
}

#[test]
fn file_scope_prunes_clone_group_instances() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    let source = "void shared() {\n  final items = [1, 2, 3];\n  final active = items.where((item) => item > 1);\n  print(active.length);\n}\n";
    write(&fixture, "lib/a.dart", source)?;
    write(&fixture, "lib/b.dart", source)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "dupes",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
            "--file",
            "lib/a.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["code_duplications"], 1);
    assert_eq!(
        json["clone_groups"][0]["instances"][0]["path"],
        "lib/a.dart"
    );
    assert_eq!(
        json["clone_groups"][0]["instances"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );

    Ok(())
}

#[test]
fn file_scope_ignores_duplicate_threshold_outside_scope() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    let duplicate = "void shared() {\n  final items = [1, 2, 3];\n  final active = items.where((item) => item > 1);\n  print(active.length);\n}\n";
    write(&fixture, "lib/a.dart", duplicate)?;
    write(&fixture, "lib/b.dart", duplicate)?;
    write(&fixture, "lib/c.dart", "void unique() {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "dupes",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
            "--file",
            "lib/c.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["findings"], 0);
    assert_eq!(json["summary"]["code_duplications"], 0);
    assert_eq!(json["summary"]["duplicated_lines"], 0);
    assert_eq!(json["summary"]["duplication_threshold_exceeded"], false);
    assert!(json["clone_groups"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn list_file_scope_filters_files_and_keeps_owner_workspace()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_basic_project(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "list",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--file",
            "lib/live.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["files"], 1);
    assert_eq!(json["files"][0]["path"], "lib/live.dart");
    assert_eq!(json["summary"]["workspaces"], 1);
    assert_eq!(json["workspaces"][0]["name"], "app");

    Ok(())
}

#[test]
fn fix_file_scope_plans_only_selected_dead_file() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_basic_project(&fixture)?;
    write(&fixture, "lib/other_dead.dart", "class OtherDead {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "fix",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
            "--file",
            "lib/dead.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["planned"], 1);
    assert_eq!(json["fixes"][0]["path"], "lib/dead.dart");

    Ok(())
}

#[test]
fn health_file_scope_filters_inventory_arrays() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_score_sources(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "health",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--file",
            "lib/complex.dart",
            "--file-scores",
            "--hotspots",
            "--targets",
            "--min-score",
            "99",
            "--max-cyclomatic",
            "3",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["files"], 1);
    assert_eq!(json["summary"]["file_scores"], 1);
    assert_eq!(json["summary"]["hotspots"], 1);
    assert_eq!(json["summary"]["refactoring_targets"], 1);
    assert_eq!(json["file_scores"][0]["path"], "lib/complex.dart");
    assert_eq!(json["hotspots"][0]["path"], "lib/complex.dart");
    assert_eq!(json["refactoring_targets"][0]["path"], "lib/complex.dart");
    assert!(json["file_scores"].as_array().is_some_and(|scores| {
        scores
            .iter()
            .all(|score| score["path"] == "lib/complex.dart")
    }));

    Ok(())
}

#[test]
fn security_file_scope_filters_candidates_and_surface() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "final uri = Uri.parse('http://api.example.com/login');\n",
    )?;
    write(
        &fixture,
        "lib/other.dart",
        "const accessToken = 'dart_decimate_fixture_value_1234567890';\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--surface",
            "--file",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["security_candidates"], 1);
    assert_eq!(json["summary"]["security_candidate_occurrences"], 1);
    assert_eq!(json["summary"]["attack_surface"], 1);
    assert_eq!(
        json["security_candidates"][0]["occurrences"][0]["path"],
        "lib/main.dart"
    );
    assert_eq!(json["attack_surface"][0]["path"], "lib/main.dart");

    Ok(())
}

fn write_basic_project(fixture: &TempDir) -> Result<(), std::io::Error> {
    write(fixture, "pubspec.yaml", "name: app\n")?;
    write(
        fixture,
        "lib/main.dart",
        "import 'live.dart';\nvoid main() { live(); }\n",
    )?;
    write(fixture, "lib/live.dart", "void live() {}\n")?;
    write(fixture, "lib/dead.dart", "class Dead {}\n")
}

fn write_score_sources(fixture: &TempDir) -> Result<(), std::io::Error> {
    write(
        fixture,
        "lib/complex.dart",
        "void risky(int value) {
  if (value == 1) return;
  if (value == 2) return;
  if (value == 3) return;
  if (value == 4) return;
  if (value == 5) return;
}
",
    )?;
    write(fixture, "lib/calm.dart", "void calm() {}\n")
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
