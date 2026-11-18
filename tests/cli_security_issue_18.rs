use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn check_command_points_representative_security_findings_to_surface()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", &token_declarations())?;
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
    assert_eq!(json["summary"]["security_candidates"], 1);
    assert_eq!(json["summary"]["security_candidate_occurrences"], 2);
    assert_eq!(json["summary"]["findings"], 1);
    assert!(json["next_steps"].as_array().is_some_and(|steps| {
        steps.iter().any(|step| {
            step["id"] == "review-security-surface"
                && step["command"] == "dart-decimate security . --format json --surface"
        })
    }));

    Ok(())
}

#[test]
fn check_command_omits_security_surface_step_when_rule_disables_visible_findings()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".dart-decimaterc",
        "[rules]\nsecurity-hardcoded-secret = \"off\"\n",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", &token_declarations())?;
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
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["security_candidates"], 0);
    assert_eq!(json["summary"]["findings"], 0);
    assert!(json["next_steps"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn check_command_omits_security_surface_step_when_rules_remove_grouped_candidate()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".dart-decimaterc",
        "[rules]\nsecurity-hardcoded-secret = \"off\"\n",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_grouped_and_singleton_security_candidates(&fixture)?;
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
    assert_eq!(json["summary"]["security_candidates"], 1);
    assert_eq!(json["summary"]["security_candidate_occurrences"], 1);
    assert_eq!(json["summary"]["findings"], 1);
    assert!(!json["next_steps"].as_array().is_some_and(|steps| {
        steps
            .iter()
            .any(|step| step["id"] == "review-security-surface")
    }));

    Ok(())
}

#[test]
fn check_command_omits_security_surface_step_when_baseline_suppresses_visible_findings()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", &token_declarations())?;
    let baseline_path = fixture.path().join("baseline.json");
    let baseline_arg = baseline_path.display().to_string();
    let mut save_output = Vec::new();

    let save_code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--save-baseline",
            baseline_arg.as_str(),
        ],
        &mut save_output,
    )?;
    let mut output = Vec::new();
    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--baseline",
            baseline_arg.as_str(),
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(save_code, 1);
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["security_candidates"], 0);
    assert_eq!(json["summary"]["findings"], 0);
    assert!(json["next_steps"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn check_command_omits_security_surface_step_when_baseline_removes_grouped_candidate()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_grouped_and_singleton_security_candidates(&fixture)?;
    let baseline_path = fixture.path().join("baseline.json");
    let baseline_arg = baseline_path.display().to_string();
    let mut save_output = Vec::new();

    let save_code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--save-baseline",
            baseline_arg.as_str(),
        ],
        &mut save_output,
    )?;
    let mut baseline = serde_json::from_slice::<Value>(&fs::read(&baseline_path)?)?;
    let findings = baseline["findings"]
        .as_array_mut()
        .ok_or_else(|| std::io::Error::other("baseline findings array"))?;
    findings.retain(|finding| finding["rule_id"] == "dart-decimate/security-hardcoded-secret");
    fs::write(&baseline_path, serde_json::to_vec(&baseline)?)?;

    let mut output = Vec::new();
    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--baseline",
            baseline_arg.as_str(),
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(save_code, 1);
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["security_candidates"], 1);
    assert_eq!(json["summary"]["security_candidate_occurrences"], 1);
    assert_eq!(json["summary"]["findings"], 1);
    assert!(!json["next_steps"].as_array().is_some_and(|steps| {
        steps
            .iter()
            .any(|step| step["id"] == "review-security-surface")
    }));

    Ok(())
}

fn token_declarations() -> String {
    format!(
        "const accessToken = '{}';\nconst refreshToken = '{}';\n",
        "0123456789abcdef".repeat(2),
        "fedcba9876543210".repeat(2)
    )
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}

fn write_grouped_and_singleton_security_candidates(
    fixture: &TempDir,
) -> Result<(), std::io::Error> {
    write(
        fixture,
        "lib/main.dart",
        &format!(
            "{}final uri = Uri.parse('http://api.example.com/login');\n",
            token_declarations()
        ),
    )
}
