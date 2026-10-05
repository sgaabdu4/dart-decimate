use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn health_command_emits_json_contract() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_complex_source(&fixture, "lib/main.dart")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "health",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--max-cyclomatic",
            "3",
            "--max-cognitive",
            "3",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["schema_version"], "dart-decimate.report.v1");
    assert_eq!(json["command"], "health");
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["health_files"], 1);
    assert_eq!(json["summary"]["functions"], 2);
    assert_eq!(json["summary"]["complex_functions"], 1);
    assert!(
        json["summary"]["quality_score"]
            .as_u64()
            .is_some_and(|score| score < 100)
    );
    assert_eq!(json["summary"]["file_scores"], 0);
    assert_eq!(json["summary"]["max_cyclomatic_complexity"], 4);
    assert_eq!(json["summary"]["max_cognitive_complexity"], 4);
    assert_eq!(
        json["findings"][0]["rule_id"],
        "dart-decimate/high-complexity"
    );
    assert_eq!(json["findings"][0]["kind"], "high-complexity");
    assert_eq!(json["findings"][0]["path"], "lib/main.dart");
    assert_eq!(json["findings"][0]["line"], 3);
    assert_eq!(json["findings"][0]["safe_to_delete"], false);
    assert_eq!(
        json["findings"][0]["actions"][0]["action"],
        "refactor-function"
    );
    assert_eq!(json["complexity"][0]["symbol"], "route");
    assert_eq!(json["complexity"][0]["cyclomatic_complexity"], 4);
    assert_eq!(json["complexity"][0]["cognitive_complexity"], 4);
    assert_eq!(json["next_steps"][0]["id"], "complexity-breakdown");

    Ok(())
}

#[test]
fn check_command_includes_health_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_complex_source(&fixture, "lib/main.dart")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--max-cyclomatic",
            "3",
            "--max-cognitive",
            "3",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(findings) = json["findings"].as_array() else {
        panic!("findings array");
    };
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["complex_functions"], 1);
    assert!(
        findings
            .iter()
            .any(|finding| finding["rule_id"] == "dart-decimate/high-complexity")
    );

    Ok(())
}

#[test]
fn health_command_passes_when_thresholds_are_not_exceeded() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() { print('ok'); }\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "health",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["functions"], 1);
    assert_eq!(json["summary"]["quality_score"], 100);
    assert_eq!(json["summary"]["complex_functions"], 0);
    assert_eq!(json["summary"]["max_cyclomatic_complexity"], 1);
    assert_eq!(json["summary"]["max_cognitive_complexity"], 0);
    assert_eq!(json["summary"]["findings"], 0);
    let Some(complexity) = json["complexity"].as_array() else {
        panic!("complexity array");
    };
    assert_eq!(complexity, &Vec::<Value>::new());

    Ok(())
}

#[test]
fn health_breakdown_includes_decision_contributions() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_complex_source(&fixture, "lib/main.dart")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "health",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--max-cyclomatic",
            "3",
            "--max-cognitive",
            "3",
            "--complexity-breakdown",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(contributions) = json["complexity"][0]["contributions"].as_array() else {
        panic!("contributions array");
    };
    assert_eq!(code, 1);
    assert_eq!(contributions[0]["kind"], "if");
    assert!(
        contributions
            .iter()
            .any(|contribution| contribution["kind"] == "loop")
    );
    assert!(
        contributions
            .iter()
            .any(|contribution| contribution["nesting"] == 1)
    );
    assert_eq!(contributions.len(), 3);

    Ok(())
}

#[test]
fn health_ignores_generated_and_test_files() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write_complex_source(&fixture, "lib/model.g.dart")?;
    write_complex_source(&fixture, "lib/model.freezed.dart")?;
    write_complex_source(&fixture, "lib/routes.gr.dart")?;
    write_complex_source(&fixture, "lib/l10n.gen.dart")?;
    write_complex_source(&fixture, "lib/foo.mocks.dart")?;
    write_complex_source(&fixture, "test/home_test.dart")?;
    write_complex_source(&fixture, "integration_test/app_test.dart")?;
    write_complex_source(&fixture, "test_driver/app_test.dart")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "health",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--max-cyclomatic",
            "3",
            "--max-cognitive",
            "3",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["health_files"], 1);
    assert_eq!(json["summary"]["complex_functions"], 0);
    assert_eq!(json["summary"]["findings"], 0);

    Ok(())
}

#[test]
fn health_coverage_flags_emit_json_contract() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_coverage_source(&fixture)?;
    write_uncovered_lcov(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "health",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--coverage",
            "coverage/lcov.info",
            "--coverage-gaps",
            "--max-crap",
            "10",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["coverage_files"], 1);
    assert_eq!(json["summary"]["coverage_gaps"], 1);
    assert_eq!(json["summary"]["crap_functions"], 1);
    assert_eq!(json["summary"]["complex_functions"], 1);
    assert_eq!(json["summary"]["max_crap_score"], 20);
    assert!(has_rule(&json, "dart-decimate/coverage-gap"));
    assert!(has_rule(&json, "dart-decimate/high-crap-score"));

    let Some(crap) = json["complexity"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|item| item["rule_id"] == "dart-decimate/high-crap-score")
    }) else {
        panic!("high-crap-score complexity entry");
    };
    assert_eq!(crap["symbol"], "uncovered");
    assert_eq!(crap["line_coverage_percent"], 0);
    assert_eq!(crap["covered_lines"], 0);
    assert_eq!(crap["executable_lines"], 4);
    assert_eq!(crap["crap_score"], 20);

    let Some(coverage_gap) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|item| item["rule_id"] == "dart-decimate/coverage-gap")
    }) else {
        panic!("coverage-gap finding");
    };
    assert_eq!(coverage_gap["kind"], "coverage-gap");
    assert_eq!(coverage_gap["path"], "lib/main.dart");
    assert_eq!(coverage_gap["safe_to_delete"], false);
    assert_eq!(coverage_gap["actions"][0]["action"], "add-test");
    assert!(coverage_gap["fingerprint"].as_str().is_some());

    Ok(())
}

#[test]
fn health_coverage_rules_can_disable_and_warn() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".dart-decimaterc",
        "[cli]
format = \"json\"

[health]
coverage_path = \"coverage/lcov.info\"
coverage_gaps = true
max_crap = 10

[rules]
coverage-gap = \"off\"
high-crap-score = \"warn\"
",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_coverage_source(&fixture)?;
    write_uncovered_lcov(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "health",
            fixture.path().to_str().unwrap_or("."),
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["coverage_gaps"], 0);
    assert_eq!(json["summary"]["crap_functions"], 1);
    assert_eq!(json["summary"]["findings"], 1);
    assert!(!has_rule(&json, "dart-decimate/coverage-gap"));
    assert_eq!(
        json["findings"][0]["rule_id"],
        "dart-decimate/high-crap-score"
    );
    assert_eq!(json["findings"][0]["severity"], "warning");

    Ok(())
}

#[test]
fn health_file_scores_are_inventory_only() -> Result<(), Box<dyn std::error::Error>> {
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
            "--file-scores",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["file_scores"], 2);
    assert_eq!(json["summary"]["findings"], 0);
    assert_eq!(json["file_scores"][0]["path"], "lib/complex.dart");
    assert!(
        json["file_scores"][0]["score"]
            .as_u64()
            .is_some_and(|score| score < 100)
    );
    assert_eq!(json["file_scores"][1]["path"], "lib/calm.dart");
    assert_eq!(json["hotspots"].as_array().map(Vec::len), Some(0));
    assert_eq!(
        json["refactoring_targets"].as_array().map(Vec::len),
        Some(0)
    );

    Ok(())
}

#[test]
fn health_hotspots_emit_actionable_findings() -> Result<(), Box<dyn std::error::Error>> {
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
            "--hotspots",
            "--min-score",
            "99",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["file_scores"], 2);
    assert_eq!(json["summary"]["hotspots"], 1);
    assert_eq!(json["hotspots"][0]["path"], "lib/complex.dart");
    assert_eq!(
        json["findings"][0]["rule_id"],
        "dart-decimate/health-hotspot"
    );
    assert_eq!(json["findings"][0]["kind"], "health-hotspot");
    assert_eq!(
        json["findings"][0]["actions"][0]["action"],
        "review-file-health"
    );
    assert!(json["findings"][0]["fingerprint"].as_str().is_some());

    Ok(())
}

#[test]
fn health_targets_imply_hotspots_and_rank_targets() -> Result<(), Box<dyn std::error::Error>> {
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
    assert_eq!(json["summary"]["file_scores"], 2);
    assert_eq!(json["summary"]["hotspots"], 1);
    assert_eq!(json["summary"]["refactoring_targets"], 1);
    assert_eq!(json["refactoring_targets"][0]["path"], "lib/complex.dart");
    assert!(
        json["refactoring_targets"][0]["priority"]
            .as_u64()
            .is_some_and(|priority| priority > 0)
    );
    assert!(has_rule(&json, "dart-decimate/refactoring-target"));
    let Some(target) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "dart-decimate/refactoring-target")
    }) else {
        panic!("refactoring-target finding");
    };
    assert_eq!(target["kind"], "refactoring-target");
    assert_eq!(target["actions"][0]["action"], "refactor-target");

    Ok(())
}

#[test]
fn health_ownership_attaches_codeowners_metadata() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        ".github/CODEOWNERS",
        "[Core]\n/lib/complex.dart @team/core owner@example.com\nlib/*.dart @team/lib\n/lib/complex.dart @team/override\n",
    )?;
    write_score_sources(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "health",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--ownership",
            "--min-score",
            "99",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["file_scores"], 2);
    assert_eq!(json["summary"]["hotspots"], 1);
    assert_eq!(json["file_scores"][0]["path"], "lib/complex.dart");
    assert_eq!(json["file_scores"][0]["owners"][0], "@team/override");
    assert_eq!(json["file_scores"][0]["owner_source"], ".github/CODEOWNERS");
    assert_eq!(json["file_scores"][0]["owner_section"], "Core");
    assert_eq!(json["hotspots"][0]["owners"][0], "@team/override");
    assert_eq!(json["file_scores"][1]["owners"][0], "@team/lib");

    Ok(())
}

#[test]
fn health_ownership_can_be_enabled_from_config() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".dart-decimaterc",
        "[cli]\nformat = \"json\"\n\n[health]\nownership = true\nmin_score = 99\n",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "CODEOWNERS", "lib/complex.dart @team/health\n")?;
    write_score_sources(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "health",
            fixture.path().to_str().unwrap_or("."),
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["file_scores"][0]["owners"][0], "@team/health");
    assert_eq!(json["file_scores"][0]["owner_source"], "CODEOWNERS");
    assert_eq!(json["hotspots"][0]["owners"][0], "@team/health");

    Ok(())
}

#[test]
fn health_hotspot_and_target_rules_can_disable_and_warn() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".dart-decimaterc",
        "[cli]
format = \"json\"

[health]
targets = true
min_score = 99
max_cyclomatic = 3

[rules]
high-cyclomatic-complexity = \"off\"
health-hotspot = \"off\"
refactoring-target = \"warn\"
",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_score_sources(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "health",
            fixture.path().to_str().unwrap_or("."),
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["hotspots"], 0);
    assert_eq!(json["summary"]["refactoring_targets"], 1);
    assert_eq!(json["summary"]["findings"], 1);
    assert_eq!(
        json["findings"][0]["rule_id"],
        "dart-decimate/refactoring-target"
    );
    assert_eq!(json["findings"][0]["severity"], "warning");
    assert_eq!(json["hotspots"].as_array().map(Vec::len), Some(0));

    Ok(())
}

fn write_complex_source(fixture: &TempDir, path: &str) -> Result<(), std::io::Error> {
    write(
        fixture,
        path,
        "void calm() {}

String route(List<int> items) {
  if (items.isEmpty) return 'none';
  for (final item in items) {
    if (item.isEven) return 'even';
  }
  return 'odd';
}
",
    )
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
  if (value == 6) return;
  if (value == 7) return;
  if (value == 8) return;
  if (value == 9) return;
}
",
    )?;
    write(fixture, "lib/calm.dart", "void calm() {}\n")
}

fn write_coverage_source(fixture: &TempDir) -> Result<(), std::io::Error> {
    write(
        fixture,
        "lib/main.dart",
        "void uncovered(List<int> items) {
  if (items.isEmpty) return;
  for (final item in items) {
    if (item.isEven) return;
  }
}
",
    )
}

fn write_uncovered_lcov(fixture: &TempDir) -> Result<(), std::io::Error> {
    write(
        fixture,
        "coverage/lcov.info",
        "SF:lib/main.dart
DA:2,0
DA:3,0
DA:4,0
DA:5,0
end_of_record
",
    )
}

fn has_rule(json: &Value, rule_id: &str) -> bool {
    json["findings"]
        .as_array()
        .is_some_and(|findings| findings.iter().any(|finding| finding["rule_id"] == rule_id))
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
