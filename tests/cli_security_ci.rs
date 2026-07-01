use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn security_ci_emits_sarif_and_fails_on_candidates() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_security_candidates(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--ci",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(results) = json["runs"][0]["results"].as_array() else {
        panic!("sarif results array");
    };
    assert_eq!(code, 1);
    assert_eq!(json["version"], "2.1.0");
    assert_eq!(json["runs"][0]["tool"]["driver"]["name"], "dart-decimate");
    assert_eq!(
        json["runs"][0]["properties"]["schemaVersion"],
        "dart-decimate.report.v1"
    );
    assert_eq!(json["runs"][0]["properties"]["command"], "security");
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|result| result["level"] == "error"));
    assert!(!String::from_utf8(output)?.contains("dart_decimate_fixture_value_1234567890"));

    Ok(())
}

#[test]
fn security_json_fail_on_issues_fails_on_candidates() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_security_candidates(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--fail-on-issues",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(candidates) = json["security_candidates"].as_array() else {
        panic!("security_candidates array");
    };
    let Some(findings) = json["findings"].as_array() else {
        panic!("findings array");
    };
    assert_eq!(code, 1);
    assert_eq!(json["schema_version"], "dart-decimate.report.v1");
    assert_eq!(json["command"], "security");
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["security_candidates"], 2);
    assert_eq!(json["summary"]["security_candidate_occurrences"], 2);
    assert_eq!(json["summary"]["findings"], 2);
    assert_eq!(candidates.len(), 2);
    assert_eq!(findings.len(), 2);

    Ok(())
}

#[test]
fn summary_omits_arrays_keeps_failure_code() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_security_candidates(&fixture)?;
    let mut full_output = Vec::new();
    let mut summary_output = Vec::new();

    let full_code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--surface",
        ],
        &mut full_output,
    )?;
    let summary_code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--summary",
            "--surface",
        ],
        &mut summary_output,
    )?;

    let json = serde_json::from_slice::<Value>(&summary_output)?;
    assert_eq!(summary_code, full_code);
    assert_eq!(summary_code, 1);
    assert_eq!(json["schema_version"], "dart-decimate.report.v1");
    assert_eq!(json["command"], "security");
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["security_candidates"], 2);
    assert_eq!(json["summary"]["security_candidate_occurrences"], 2);
    assert_eq!(json["summary"]["attack_surface"], 2);
    assert_eq!(json["summary"]["findings"], 2);
    assert!(
        json["security_candidates"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));
    assert!(json["attack_surface"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn security_summary_human_omits_details_without_pass_copy() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_security_candidates(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "human",
            "--summary",
            "--surface",
        ],
        &mut output,
    )?;

    let text = String::from_utf8(output)?;
    assert_eq!(code, 1);
    assert!(text.contains("Dart Decimate security: FAIL"));
    assert!(text.contains("Findings: 2"));
    assert!(text.contains("2 findings were omitted from this summary output."));
    assert!(!text.contains("No findings. The selected Dart graph checks passed."));

    Ok(())
}

#[test]
fn security_summary_html_omits_details_without_pass_copy() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_security_candidates(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "html",
            "--summary",
            "--surface",
        ],
        &mut output,
    )?;

    let html = String::from_utf8(output)?;
    assert_eq!(code, 1);
    assert!(html.contains("<h1>security report</h1>"));
    assert!(html.contains("2 findings were omitted from this summary output."));
    assert!(!html.contains("No findings. The selected Dart graph checks passed."));

    Ok(())
}

#[test]
fn security_summary_does_not_change_passing_exit_code() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() { print('ok'); }\n")?;
    let mut full_output = Vec::new();
    let mut summary_output = Vec::new();

    let full_code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut full_output,
    )?;
    let summary_code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--summary",
        ],
        &mut summary_output,
    )?;

    let json = serde_json::from_slice::<Value>(&summary_output)?;
    assert_eq!(summary_code, full_code);
    assert_eq!(summary_code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["security_candidates"], 0);
    assert_eq!(json["summary"]["findings"], 0);
    assert!(
        json["security_candidates"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn security_ci_fails_on_warn_level_candidates() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, ".dart-decimaterc", "[rules]\nall = \"warn\"\n")?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_security_candidates(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--ci",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(results) = json["runs"][0]["results"].as_array() else {
        panic!("sarif results array");
    };
    assert_eq!(code, 1);
    assert_eq!(json["runs"][0]["properties"]["verdict"], "pass");
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|result| result["level"] == "warning"));

    Ok(())
}

#[test]
fn security_fail_on_issues_fails_on_warn_level_candidates() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, ".dart-decimaterc", "[rules]\nall = \"warn\"\n")?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_security_candidates(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--fail-on-issues",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["findings"], 2);
    assert_eq!(json["findings"][0]["severity"], "warning");

    Ok(())
}

#[test]
fn security_process_execution_rule_off_removes_candidate_and_surface()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".dart-decimaterc",
        "[rules]\nsecurity-process-execution = \"off\"\n",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'dart:io';\nFuture<void> main(String command) => Process.run(command, ['status']);\n",
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
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["security_candidates"], 0);
    assert_eq!(json["summary"]["attack_surface"], 0);
    assert!(
        json["security_candidates"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );
    assert!(json["attack_surface"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

fn write_security_candidates(fixture: &TempDir) -> Result<(), std::io::Error> {
    write(
        fixture,
        "lib/main.dart",
        "const accessToken = 'dart_decimate_fixture_value_1234567890';
final uri = Uri.parse('http://api.example.com/login');
",
    )
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
