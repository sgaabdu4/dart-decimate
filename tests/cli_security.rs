use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn security_command_emits_json_contract() -> Result<(), Box<dyn std::error::Error>> {
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
            "--surface",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["schema_version"], "dart-decimate.report.v1");
    assert_eq!(json["command"], "security");
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["security_candidates"], 2);
    assert_eq!(json["summary"]["security_candidate_occurrences"], 2);
    assert_eq!(json["summary"]["attack_surface"], 2);
    assert_eq!(json["summary"]["findings"], 2);
    assert_eq!(
        json["security_candidates"][0]["occurrences"][0]["path"],
        "lib/main.dart"
    );
    assert_eq!(json["findings"][0]["kind"], "security-candidate");
    assert_eq!(json["findings"][0]["safe_to_delete"], false);
    assert_eq!(
        json["findings"][0]["actions"][0]["action"],
        "review-security-candidate"
    );
    let Some(candidates) = json["security_candidates"].as_array() else {
        panic!("security_candidates array");
    };
    let Some(secret) = candidates
        .iter()
        .find(|candidate| candidate["category"] == "hardcoded-secret")
    else {
        panic!("hardcoded-secret candidate");
    };
    assert_eq!(secret["finding_id"], secret["fingerprint"]);
    assert_eq!(secret["cwe"][0], "CWE-798");
    assert_eq!(secret["severity"], "error");
    assert_eq!(secret["candidate"]["source"], "source-code-literal");
    assert_eq!(secret["candidate"]["sink"], "secret-literal");
    assert_eq!(secret["candidate"]["boundary"], "source-control");
    assert_eq!(
        secret["evidence"]["occurrences"][0]["path"],
        "lib/main.dart"
    );
    assert_eq!(secret["trace"][0]["role"], "source");
    assert!(
        candidates
            .iter()
            .flat_map(|candidate| candidate["occurrences"].as_array().into_iter().flatten())
            .all(|occurrence| !occurrence["evidence"]
                .as_str()
                .unwrap_or_default()
                .contains("dart_decimate_fixture_value_1234567890"))
    );

    Ok(())
}

#[test]
fn security_config_categories_filter_candidates() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        ".dart-decimaterc",
        "[cli]\nformat = \"json\"\n\n[security]\nsurface = true\ncategories = [\"insecure-transport\"]\n",
    )?;
    write_security_candidates(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["security_candidates"], 1);
    assert_eq!(json["summary"]["security_candidate_occurrences"], 1);
    assert_eq!(json["summary"]["attack_surface"], 1);
    assert_eq!(
        json["security_candidates"][0]["rule_id"],
        "dart-decimate/security-insecure-transport"
    );

    Ok(())
}

#[test]
fn security_command_emits_sarif_contract() -> Result<(), Box<dyn std::error::Error>> {
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
            "sarif",
            "--entry",
            "lib/main.dart",
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
    assert!(results.iter().all(|result| {
        result["properties"]["findingId"] == result["partialFingerprints"]["decimateFingerprint"]
    }));
    assert!(results.iter().all(|result| {
        result["properties"]["safeToDelete"] == false
            && result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
                == "lib/main.dart"
    }));
    assert_sarif_location(results, "dart-decimate/security-hardcoded-secret", 1, 21);
    assert_sarif_location(results, "dart-decimate/security-insecure-transport", 2, 23);
    assert!(!String::from_utf8(output)?.contains("dart_decimate_fixture_value_1234567890"));

    Ok(())
}

#[test]
fn security_sarif_passes_when_no_candidates() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() { print('ok'); }\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "sarif",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(results) = json["runs"][0]["results"].as_array() else {
        panic!("sarif results array");
    };
    assert_eq!(code, 0);
    assert_eq!(json["version"], "2.1.0");
    assert_eq!(json["runs"][0]["tool"]["driver"]["name"], "dart-decimate");
    assert!(results.is_empty());

    Ok(())
}

#[test]
fn security_sarif_omits_suppressed_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "// dart-decimate-ignore-next-line security-candidate
const accessToken = 'dart_decimate_fixture_value_1234567890';
",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "sarif",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(results) = json["runs"][0]["results"].as_array() else {
        panic!("sarif results array");
    };
    assert_eq!(code, 0);
    assert!(results.is_empty());
    assert!(!String::from_utf8(output)?.contains("dart_decimate_fixture_value_1234567890"));

    Ok(())
}

#[test]
fn security_sarif_file_writes_code_scanning_output() -> Result<(), Box<dyn std::error::Error>> {
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
            "--sarif-file",
            "security.sarif.json",
        ],
        &mut output,
    )?;

    let stdout = serde_json::from_slice::<Value>(&output)?;
    let sarif_path = fixture.path().join("security.sarif.json");
    let sarif_text = fs::read_to_string(sarif_path)?;
    let sarif = serde_json::from_str::<Value>(&sarif_text)?;
    let Some(results) = sarif["runs"][0]["results"].as_array() else {
        panic!("sarif results array");
    };
    assert_eq!(code, 1);
    assert_eq!(stdout["schema_version"], "dart-decimate.report.v1");
    assert_eq!(stdout["command"], "security");
    assert_eq!(sarif["version"], "2.1.0");
    assert_eq!(results.len(), 2);
    assert!(!sarif_text.contains("dart_decimate_fixture_value_1234567890"));

    Ok(())
}

#[test]
fn security_gate_new_filters_json_to_added_lines() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_security_candidates(&fixture)?;
    write_security_diff(&fixture, 2)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--surface",
            "--gate",
            "new",
            "--diff-file",
            "security.diff",
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
    assert_eq!(code, 8);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["security_candidates"], 1);
    assert_eq!(json["summary"]["security_candidate_occurrences"], 1);
    assert_eq!(json["summary"]["attack_surface"], 1);
    assert_eq!(json["summary"]["findings"], 1);
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0]["rule_id"],
        "dart-decimate/security-insecure-transport"
    );
    assert_eq!(candidates[0]["occurrences"][0]["line"], 2);
    assert_eq!(
        findings[0]["rule_id"],
        "dart-decimate/security-insecure-transport"
    );
    assert_eq!(findings[0]["line"], 2);

    Ok(())
}

#[test]
fn security_gate_new_filters_sarif_to_added_lines() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_security_candidates(&fixture)?;
    write_security_diff(&fixture, 1)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "sarif",
            "--gate",
            "new",
            "--diff-file",
            "security.diff",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(results) = json["runs"][0]["results"].as_array() else {
        panic!("sarif results array");
    };
    assert_eq!(code, 8);
    assert_eq!(results.len(), 1);
    assert_sarif_location(results, "dart-decimate/security-hardcoded-secret", 1, 21);
    assert!(
        results
            .iter()
            .all(|result| { result["ruleId"] != "dart-decimate/security-insecure-transport" })
    );

    Ok(())
}

#[test]
fn security_gate_new_passes_when_diff_has_no_candidates() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_security_candidates(&fixture)?;
    write_security_diff(&fixture, 3)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--gate",
            "new",
            "--diff-file",
            "security.diff",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["security_candidates"], 0);
    assert_eq!(json["summary"]["security_candidate_occurrences"], 0);
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
fn security_gate_new_requires_diff_file() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_security_candidates(&fixture)?;
    let mut output = Vec::new();

    let error = match run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--gate",
            "new",
        ],
        &mut output,
    ) {
        Ok(code) => panic!("security --gate new returned code {code}"),
        Err(error) => error,
    };

    assert_eq!(
        error.to_string(),
        "security --gate new or newly-reachable requires --diff-file PATH, --diff-stdin, --changed-since REF, or --compare REF"
    );
    assert!(output.is_empty());

    Ok(())
}

#[test]
fn security_gate_newly_reachable_requires_diff_file() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_security_candidates(&fixture)?;
    let mut output = Vec::new();

    let error = match run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--gate",
            "newly-reachable",
        ],
        &mut output,
    ) {
        Ok(code) => panic!("security --gate newly-reachable returned code {code}"),
        Err(error) => error,
    };

    assert_eq!(
        error.to_string(),
        "security --gate new or newly-reachable requires --diff-file PATH, --diff-stdin, --changed-since REF, or --compare REF"
    );
    assert!(output.is_empty());

    Ok(())
}

#[test]
fn check_command_supports_sarif_output() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(&fixture, "lib/src/dead.dart", "class Dead {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "sarif",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(results) = json["runs"][0]["results"].as_array() else {
        panic!("sarif results array");
    };
    assert_eq!(code, 1);
    assert_eq!(json["version"], "2.1.0");
    assert_eq!(json["runs"][0]["properties"]["command"], "check");
    assert!(
        results
            .iter()
            .any(|result| result["ruleId"] == "dart-decimate/dead-file")
    );

    Ok(())
}

#[test]
fn check_command_includes_security_candidates() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "final uri = Uri.parse('http://api.example.com/login');\nvoid main() {}\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["command"], "check");
    assert_eq!(json["summary"]["security_candidates"], 1);
    assert_eq!(json["summary"]["security_candidate_occurrences"], 1);
    assert_eq!(json["summary"]["attack_surface"], 0);
    assert_eq!(
        json["security_candidates"][0]["rule_id"],
        "dart-decimate/security-insecure-transport"
    );
    assert!(json["findings"].as_array().is_some_and(|findings| {
        findings
            .iter()
            .any(|finding| finding["kind"] == "security-candidate")
    }));

    Ok(())
}

#[test]
fn security_top_limits_grouped_candidates() -> Result<(), Box<dyn std::error::Error>> {
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
            "--top",
            "1",
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
    assert_eq!(json["summary"]["security_candidates"], 1);
    assert_eq!(json["summary"]["security_candidate_occurrences"], 2);
    assert_eq!(candidates.len(), 1);
    assert_eq!(findings.len(), 1);

    Ok(())
}

#[test]
fn security_command_passes_when_no_candidates() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() { print('ok'); }\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
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
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["security_candidates"], 0);
    assert_eq!(json["summary"]["security_candidate_occurrences"], 0);
    assert!(candidates.is_empty());
    assert!(findings.is_empty());

    Ok(())
}

#[test]
fn security_findings_respect_inline_suppression() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "// dart-decimate-ignore-next-line security-candidate
const accessToken = 'dart_decimate_fixture_value_1234567890';
",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
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
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["security_candidates"], 1);
    assert_eq!(json["summary"]["findings"], 0);
    assert_eq!(candidates.len(), 1);
    assert!(findings.is_empty());

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

fn write_security_diff(fixture: &TempDir, added_line: usize) -> Result<(), std::io::Error> {
    let prefix = " context\n".repeat(added_line.saturating_sub(1));
    let added = match added_line {
        1 => "+const accessToken = 'dart_decimate_fixture_value_1234567890';\n",
        2 => "+final uri = Uri.parse('http://api.example.com/login');\n",
        _ => "+void helper() {}\n",
    };
    write(
        fixture,
        "security.diff",
        &format!(
            "diff --git a/lib/main.dart b/lib/main.dart
--- a/lib/main.dart
+++ b/lib/main.dart
@@ -1,2 +1,3 @@
{prefix}{added}"
        ),
    )
}

fn assert_sarif_location(results: &[Value], rule_id: &str, line: usize, column: usize) {
    let Some(result) = results.iter().find(|result| result["ruleId"] == rule_id) else {
        panic!("missing SARIF result for {rule_id}");
    };
    assert_eq!(
        result["locations"][0]["physicalLocation"]["region"]["startLine"],
        line
    );
    assert_eq!(
        result["locations"][0]["physicalLocation"]["region"]["startColumn"],
        column
    );
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
