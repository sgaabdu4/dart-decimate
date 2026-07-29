use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn check_reports_boundary_call_violations() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    write(
        &fixture,
        "lib/ui/page.dart",
        "void render() { SystemChrome.setPreferredOrientations([]); }\n",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--boundary-call",
        "lib/ui:SystemChrome.*",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["summary"]["boundary_call_violations"], 1);
    let finding = &json["findings"][0];
    assert_eq!(finding["rule_id"], "dart-decimate/boundary-violation");
    assert_eq!(finding["kind"], "boundary-call-violation");
    assert_eq!(finding["path"], "lib/ui/page.dart");
    assert_eq!(
        finding["actions"][0]["suppression_comment"],
        "// dart-decimate-ignore-next-line boundary-call-violation"
    );

    Ok(())
}

#[test]
fn policy_pack_reports_banned_imports_and_calls_as_warnings()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'dart:io';\nvoid main() { Process.killPid(1); }\n",
    )?;
    write_policy_pack(&fixture)?;

    let (code, json) = run_json([
        "dart-decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--policy-pack",
        "policy.jsonc",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["policy_violations"], 2);
    assert_eq!(json["summary"]["findings"], 2);
    assert!(json["findings"].as_array().is_some_and(|findings| {
        findings.iter().all(|finding| {
            finding["kind"] == "policy-violation" && finding["severity"] == "warning"
        })
    }));
    assert!(json["findings"].as_array().is_some_and(|findings| {
        findings
            .iter()
            .any(|finding| finding["rule_id"] == "dart-decimate/policy/mobile/no-dart-io")
    }));

    Ok(())
}

#[test]
fn policy_violations_can_be_promoted_and_suppressed() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    write(
        &fixture,
        "lib/main.dart",
        "// dart-decimate-ignore-next-line dart-decimate/policy/mobile/no-dart-io\nimport 'dart:io';\nvoid main() { Process.killPid(1); }\n",
    )?;
    write_policy_pack(&fixture)?;
    write(
        &fixture,
        ".dart-decimaterc",
        "\
format = \"json\"
rulePacks = [\"policy.jsonc\"]

[rules]
policy-violation = \"error\"
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["policy_violations"], 1);
    assert_eq!(
        json["findings"][0]["rule_id"],
        "dart-decimate/policy/mobile/no-process"
    );
    assert_eq!(json["findings"][0]["severity"], "error");

    Ok(())
}

#[test]
fn policy_pack_rule_severity_can_fail_check() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    write(
        &fixture,
        "lib/main.dart",
        "void main() { Process.killPid(1); }\n",
    )?;
    write(
        &fixture,
        "policy.jsonc",
        r#"{
  "name": "mobile",
  "rules": [
    {
      "id": "no-process",
      "type": "banned-call",
      "pattern": "Process.*",
      "severity": "error"
    }
  ]
}
"#,
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--policy-pack",
        "policy.jsonc",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["findings"][0]["severity"], "error");

    Ok(())
}

#[test]
fn boundary_call_family_rule_can_disable_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    write(
        &fixture,
        "lib/ui/page.dart",
        "void render() { SystemChrome.setPreferredOrientations([]); }\n",
    )?;
    write(
        &fixture,
        ".dart-decimaterc",
        "\
format = \"json\"
boundaryCalls = [\"lib/ui:SystemChrome.*\"]

[rules]
boundary-violation = \"off\"
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["summary"]["boundary_call_violations"], 0);
    assert_eq!(json["summary"]["findings"], 0);

    Ok(())
}

#[test]
fn rule_pack_schema_command_emits_json_schema() -> Result<(), Box<dyn std::error::Error>> {
    let (code, json) = run_json(["dart-decimate", "rule-pack-schema", "--format", "json"])?;

    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "dart-decimate.rule-pack.v1");
    assert_eq!(
        json["$defs"]["rule"]["properties"]["type"]["enum"][0],
        "banned-import"
    );

    Ok(())
}

fn write_policy_pack(fixture: &TempDir) -> Result<(), std::io::Error> {
    write(
        fixture,
        "policy.jsonc",
        r#"{
  "name": "mobile",
  "rules": [
    {
      "id": "no-dart-io",
      "type": "banned-import",
      "pattern": "dart:io",
      "message": "Do not import dart:io from app code"
    },
    {
      "id": "no-process",
      "type": "banned-call",
      "pattern": "Process.*"
    }
  ]
}
"#,
    )
}

fn fixture() -> Result<TempDir, std::io::Error> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    Ok(fixture)
}

fn run_json<I, S>(args: I) -> Result<(i32, Value), Box<dyn std::error::Error>>
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> + Clone,
{
    let mut output = Vec::new();
    let code = run_from(args, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    Ok((code, json))
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
