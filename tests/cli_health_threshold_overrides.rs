use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn health_threshold_override_suppresses_matching_exact_function()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_override_config(
        &fixture,
        "maxCyclomatic = 4\nmaxCognitive = 4\nreason = \"legacy branch budget\"\n",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/legacy.dart",
        &format!(
            "{}\n{}",
            route_source("legacyRoute"),
            route_source("modernRoute")
        ),
    )?;

    let json = run_health_json(&fixture)?;

    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["complex_functions"], 1);
    assert_eq!(json["complexity"][0]["symbol"], "modernRoute");
    assert_eq!(json["threshold_overrides"][0]["active"], true);
    assert_eq!(json["threshold_overrides"][0]["stale"], false);
    assert_eq!(json["threshold_overrides"][0]["no_match"], false);
    assert_eq!(
        json["threshold_overrides"][0]["reason"],
        "legacy branch budget"
    );
    assert_eq!(
        json["threshold_overrides"][0]["matched_functions"][0],
        "lib/legacy.dart:legacyRoute"
    );

    Ok(())
}

#[test]
fn health_threshold_override_marks_finding_threshold_source()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_override_config(&fixture, "maxCyclomatic = 4\nmaxCognitive = 4\n")?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/legacy.dart",
        &very_complex_route_source("legacyRoute"),
    )?;

    let json = run_health_json(&fixture)?;

    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["complexity"][0]["symbol"], "legacyRoute");
    assert_eq!(json["complexity"][0]["threshold_source"], "override");
    assert_eq!(
        json["complexity"][0]["effective_thresholds"]["max_cyclomatic"],
        4
    );
    assert_eq!(
        json["complexity"][0]["effective_thresholds"]["max_cognitive"],
        4
    );
    assert_eq!(
        json["complexity"][0]["rule_id"],
        "dart-decimate/high-complexity"
    );

    Ok(())
}

#[test]
fn health_threshold_override_file_only_applies_to_all_functions_in_file()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".dart-decimaterc",
        "[cli]
format = \"json\"

[health]
max_cyclomatic = 3
max_cognitive = 3

[[health.thresholdOverrides]]
files = [\"lib/legacy.dart\"]
maxCyclomatic = 4
maxCognitive = 4
",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/legacy.dart",
        &format!(
            "{}\n{}",
            route_source("legacyRoute"),
            route_source("modernRoute")
        ),
    )?;

    let json = run_health_json(&fixture)?;

    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["complex_functions"], 0);
    assert_eq!(json["threshold_overrides"][0]["active"], true);
    assert_eq!(json["threshold_overrides"][0]["no_match"], false);

    Ok(())
}

#[test]
fn health_threshold_override_reports_stale_and_no_match() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".dart-decimaterc",
        "[cli]
format = \"json\"

[[health.thresholdOverrides]]
files = [\"lib/main.dart\"]
functions = [\"calm\"]
maxCyclomatic = 10

[[health.thresholdOverrides]]
files = [\"lib/missing.dart\"]
maxCyclomatic = 10
",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void calm() {}\n")?;

    let json = run_health_json(&fixture)?;

    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["threshold_overrides"][0]["stale"], true);
    assert_eq!(json["threshold_overrides"][0]["active"], false);
    assert_eq!(json["threshold_overrides"][0]["no_match"], false);
    assert_eq!(json["threshold_overrides"][1]["no_match"], true);
    assert_eq!(json["threshold_overrides"][1]["active"], false);
    assert_eq!(json["threshold_overrides"][1]["stale"], false);

    Ok(())
}

#[test]
fn health_threshold_override_applies_to_crap_threshold() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".dart-decimaterc",
        "[cli]
format = \"json\"

[health]
coverage_path = \"coverage/lcov.info\"
max_crap = 10

[[health.thresholdOverrides]]
files = [\"lib/main.dart\"]
functions = [\"uncovered\"]
maxCrap = 25
reason = \"legacy coverage debt\"
",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", &coverage_source())?;
    write_uncovered_lcov(&fixture)?;

    let json = run_health_json(&fixture)?;

    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["crap_functions"], 0);
    assert_eq!(json["summary"]["findings"], 0);
    assert!(!has_rule(&json, "dart-decimate/high-crap-score"));
    assert_eq!(json["threshold_overrides"][0]["active"], true);

    Ok(())
}

#[test]
fn config_schema_includes_health_threshold_overrides() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(
        ["dart-decimate", "config-schema", "--format", "json"],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(
        json["properties"]["health"]["properties"]["thresholdOverrides"]["type"],
        "array"
    );
    assert_eq!(
        json["properties"]["health"]["properties"]["thresholdOverrides"]["items"]["required"][0],
        "files"
    );
    assert_eq!(
        json["properties"]["health"]["properties"]["thresholdOverrides"]["items"]["properties"]["reason"]
            ["type"][1],
        "null"
    );
    assert_eq!(
        json["properties"]["health"]["properties"]["maxUnitSize"]["minimum"],
        1
    );
    assert_eq!(
        json["properties"]["health"]["properties"]["thresholdOverrides"]["items"]["properties"]["maxUnitSize"]
            ["minimum"],
        1
    );

    Ok(())
}

#[test]
fn max_unit_size_config_and_override_aliases_apply() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".dart-decimaterc",
        "[cli]
format = \"json\"

[health]
maxUnitSize = 60

[[health.thresholdOverrides]]
files = [\"lib/main.dart\"]
functions = [\"legacyRoute\"]
maxUnitSize = 120
reason = \"framework adapter\"
",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        &large_function_source("legacyRoute", 61),
    )?;

    let json = run_health_json(&fixture)?;

    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["large_functions"], 0);
    assert!(
        json["large_functions"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
    assert_eq!(json["threshold_overrides"][0]["max_unit_size"], 120);
    assert_eq!(json["threshold_overrides"][0]["active"], true);
    assert_eq!(
        json["threshold_overrides"][0]["matched_functions"][0],
        "lib/main.dart:legacyRoute"
    );

    Ok(())
}

#[test]
fn report_schema_includes_threshold_override_contract() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(
        ["dart-decimate", "report-schema", "--format", "json"],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert!(array_contains(&json["required"], "threshold_overrides"));
    assert_eq!(json["properties"]["threshold_overrides"]["type"], "array");
    assert_eq!(
        json["$defs"]["threshold_override"]["properties"]["active"]["type"],
        "boolean"
    );
    assert!(array_contains(&json["required"], "large_functions"));
    assert_eq!(json["properties"]["large_functions"]["type"], "array");
    assert_eq!(
        json["$defs"]["large_function"]["properties"]["line_count"]["minimum"],
        1
    );

    Ok(())
}

#[test]
fn malformed_health_threshold_override_reports_config_error()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".dart-decimaterc",
        "[[health.thresholdOverrides]]
files = []
maxCyclomatic = 4
",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let mut output = Vec::new();

    let error = match run_from(
        [
            "dart-decimate",
            "health",
            fixture.path().to_str().unwrap_or("."),
        ],
        &mut output,
    ) {
        Ok(code) => panic!("expected config error, got exit code {code}"),
        Err(error) => error,
    };

    assert!(format!("{error}").contains("health.thresholdOverrides"));
    assert!(output.is_empty());

    Ok(())
}

fn run_health_json(fixture: &TempDir) -> Result<Value, Box<dyn std::error::Error>> {
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
    assert_eq!(code, i32::from(json["verdict"] == "fail"));
    Ok(json)
}

fn write_override_config(fixture: &TempDir, thresholds: &str) -> Result<(), std::io::Error> {
    write(
        fixture,
        ".dart-decimaterc",
        &format!(
            "[cli]
format = \"json\"

[health]
max_cyclomatic = 3
max_cognitive = 3

[[health.thresholdOverrides]]
files = [\"lib/legacy.dart\"]
functions = [\"legacyRoute\"]
{thresholds}
"
        ),
    )
}

fn route_source(name: &str) -> String {
    format!(
        "String {name}(List<int> items) {{
  if (items.isEmpty) return 'none';
  for (final item in items) {{
    if (item.isEven) return 'even';
  }}
  return 'odd';
}}
"
    )
}

fn very_complex_route_source(name: &str) -> String {
    format!(
        "String {name}(List<int> items) {{
  if (items.length > 10) return 'many';
  if (items.isEmpty) return 'none';
  for (final item in items) {{
    if (item.isEven) return 'even';
  }}
  return 'odd';
}}
"
    )
}

fn large_function_source(name: &str, lines: usize) -> String {
    let mut source = vec![format!("void {name}() {{")];
    source.extend(
        (0..lines.saturating_sub(2)).map(|index| format!("  final value{index} = {index};")),
    );
    source.push("}".to_owned());
    source.join("\n")
}

fn coverage_source() -> String {
    "void uncovered(List<int> items) {
  if (items.isEmpty) return;
  for (final item in items) {
    if (item.isEven) return;
  }
}
"
    .to_owned()
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

fn array_contains(value: &Value, expected: &str) -> bool {
    value
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item == expected))
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
