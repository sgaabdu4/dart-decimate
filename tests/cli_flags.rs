use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn flags_command_emits_json_contract() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_feature_flags(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "flags",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["schema_version"], "dart-decimate.report.v1");
    assert_eq!(json["command"], "flags");
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["feature_flags"], 5);
    assert_eq!(json["summary"]["feature_flag_occurrences"], 5);
    assert_eq!(json["summary"]["findings"], 5);
    assert_eq!(
        json["feature_flags"][0]["occurrences"][0]["path"],
        "lib/main.dart"
    );
    assert_eq!(json["findings"][0]["rule_id"], "dart-decimate/feature-flag");
    assert_eq!(json["findings"][0]["kind"], "feature-flag");
    assert_eq!(json["findings"][0]["safe_to_delete"], false);
    assert_eq!(
        json["findings"][0]["actions"][0]["action"],
        "review-feature-flag"
    );

    Ok(())
}

#[test]
fn check_command_includes_feature_flag_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "const beta = bool.fromEnvironment('FEATURE_BETA');\nvoid main() {}\n",
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
    assert_eq!(json["summary"]["feature_flags"], 1);
    assert_eq!(json["summary"]["feature_flag_occurrences"], 1);
    assert_eq!(json["feature_flags"][0]["name"], "FEATURE_BETA");
    assert!(json["findings"].as_array().is_some_and(|findings| {
        findings
            .iter()
            .any(|finding| finding["rule_id"] == "dart-decimate/feature-flag")
    }));

    Ok(())
}

#[test]
fn flags_top_limits_grouped_flags() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_feature_flags(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "flags",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--top",
            "2",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(flags) = json["feature_flags"].as_array() else {
        panic!("feature_flags array");
    };
    let Some(findings) = json["findings"].as_array() else {
        panic!("findings array");
    };
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["feature_flags"], 2);
    assert_eq!(json["summary"]["feature_flag_occurrences"], 5);
    assert_eq!(flags.len(), 2);
    assert_eq!(findings.len(), 2);

    Ok(())
}

#[test]
fn flags_command_passes_when_no_flags() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() { print('ok'); }\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "flags",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(flags) = json["feature_flags"].as_array() else {
        panic!("feature_flags array");
    };
    let Some(findings) = json["findings"].as_array() else {
        panic!("findings array");
    };
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["feature_flags"], 0);
    assert_eq!(json["summary"]["feature_flag_occurrences"], 0);
    assert_eq!(flags, &Vec::<Value>::new());
    assert_eq!(findings, &Vec::<Value>::new());

    Ok(())
}

#[test]
fn flags_findings_respect_inline_suppression() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "// dart-decimate-ignore-next-line feature-flag
const beta = bool.fromEnvironment('FEATURE_BETA');
",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "flags",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(flags) = json["feature_flags"].as_array() else {
        panic!("feature_flags array");
    };
    let Some(findings) = json["findings"].as_array() else {
        panic!("findings array");
    };
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["feature_flags"], 1);
    assert_eq!(json["summary"]["feature_flag_occurrences"], 1);
    assert_eq!(json["summary"]["findings"], 0);
    assert_eq!(flags.len(), 1);
    assert_eq!(findings, &Vec::<Value>::new());

    Ok(())
}

fn write_feature_flags(fixture: &TempDir) -> Result<(), std::io::Error> {
    write(
        fixture,
        "lib/main.dart",
        "const checkout = bool.fromEnvironment('FEATURE_CHECKOUT');
const variant = String.fromEnvironment('experiment_variant');

bool enabled(dynamic client) {
  if (Platform.environment['FEATURE_PAYMENT_FLOW'] == '1') return true;
  if (FirebaseRemoteConfig.instance.getBool('new_checkout')) return true;
  return client.boolVariation('premium_dashboard', false);
}
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
