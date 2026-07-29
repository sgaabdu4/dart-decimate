use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn private_type_leak_is_opt_in_and_agent_actionable() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = private_leak_fixture()?;

    let (default_code, default_json) = run_json([
        "dart-decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/package.dart",
    ])?;
    assert_eq!(default_code, 0);
    assert_eq!(default_json["summary"]["private_type_leaks"], 0);

    let (code, json) = run_json([
        "dart-decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/package.dart",
        "--private-type-leaks",
    ])?;

    let finding = &json["findings"][0];
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["private_type_leaks"], 1);
    assert_eq!(finding["rule_id"], "dart-decimate/private-type-leak");
    assert_eq!(finding["kind"], "private-type-leak");
    assert_eq!(finding["path"], "lib/package.dart");
    assert_eq!(finding["safe_to_delete"], false);
    assert_eq!(finding["actions"][0]["action"], "review-public-api");
    assert_eq!(finding["actions"][0]["target_symbol"], "Api");
    assert_eq!(
        finding["actions"][0]["suppression_comment"],
        "// dart-decimate-ignore-next-line private-type-leak"
    );
    assert_eq!(json["summary"]["semantic_evidence"], 1);
    assert_eq!(json["summary"]["type_couplings"], 1);
    assert_eq!(json["semantic"]["completeness"], "partial");
    assert_eq!(json["semantic"]["evidence"][0]["decision"], "confirmed");
    assert_eq!(
        json["semantic"]["evidence"][0]["subject"]["library_uri"],
        "lib/package.dart"
    );
    assert_eq!(
        json["semantic"]["type_couplings"][0]["target"]["name"],
        "_Hidden"
    );

    Ok(())
}

#[test]
fn private_type_leak_can_be_enabled_by_config_rule() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = private_leak_fixture()?;
    write(
        &fixture,
        ".dart-decimaterc.json",
        "{\"rules\":{\"private-type-leak\":\"warn\"}}\n",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/package.dart",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["summary"]["private_type_leaks"], 1);
    assert_eq!(json["findings"][0]["severity"], "warning");

    Ok(())
}

#[test]
fn private_type_leak_respects_inline_suppression() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "\
// dart-decimate-ignore-next-line private-type-leak
class Api extends _Hidden {}
class _Hidden {}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/package.dart",
        "--private-type-leaks",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["summary"]["private_type_leaks"], 0);
    assert_eq!(json["summary"]["findings"], 0);

    Ok(())
}

#[test]
fn semantic_completeness_reports_dynamic_access_without_inventing_a_finding()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "void main() {\n  dynamic service;\n  service.call();\n}\n",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["summary"]["findings"], 0);
    assert_eq!(json["semantic"]["completeness"], "partial");
    assert!(
        json["semantic"]["reasons"]
            .as_array()
            .is_some_and(|reasons| reasons.iter().any(|reason| reason == "dynamic-access"))
    );

    Ok(())
}

fn private_leak_fixture() -> Result<TempDir, std::io::Error> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "class Api extends _Hidden {}\nclass _Hidden {}\n",
    )?;
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
