use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn check_reports_stale_inline_suppression() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "// dart-decimate-ignore-next-line dead-file\nvoid main() {}\n",
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

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["findings"], 1);
    assert_eq!(
        json["findings"][0]["rule_id"],
        "dart-decimate/stale-suppression"
    );
    assert_eq!(json["findings"][0]["kind"], "stale-suppression");
    assert_eq!(json["findings"][0]["path"], "lib/main.dart");
    assert_eq!(json["findings"][0]["line"], 1);
    assert_eq!(json["findings"][0]["safe_to_delete"], true);
    assert_eq!(
        json["findings"][0]["actions"][0]["action"],
        "remove-suppression"
    );
    assert_eq!(
        json["findings"][0]["actions"][0]["type"],
        "remove-suppression"
    );
    assert_eq!(
        json["findings"][0]["actions"][0]["target_path"],
        "lib/main.dart"
    );
    assert_eq!(
        json["findings"][0]["actions"][0]["suppression_comment"],
        "// dart-decimate-ignore-next-line dead-file"
    );
    assert_eq!(json["findings"][0]["actions"][0]["auto_fixable"], true);

    Ok(())
}

#[test]
fn used_inline_suppression_is_not_reported_as_stale() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "// dart-decimate-ignore-next-line feature-flag\nconst beta = bool.fromEnvironment('FEATURE_BETA');\n",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "flags",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["findings"], 0);
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn unused_member_findings_respect_fallow_suppression() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/live.dart';\nvoid main() { runLive(); }\n",
    )?;
    write(
        &fixture,
        "lib/src/live.dart",
        "\
enum Mode {
  on,
  // dart-decimate-ignore-next-line unused-enum-member
  off,
}
void runLive() { print(Mode.on); }
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "dead-code",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["unused_enum_members"], 0);
    assert_eq!(json["summary"]["findings"], 0);

    Ok(())
}

#[test]
fn misplaced_suppression_names_the_line_its_finding_lands_on()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/live.dart';\nvoid main() { runLive(); }\n",
    )?;
    write(
        &fixture,
        "lib/src/live.dart",
        "\
enum Mode {
  on,
  // dart-decimate-ignore-next-line unused-enum-member
  // the directive sits one line too high to cover the member
  off,
}
void runLive() { print(Mode.on); }
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "dead-code",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
    ])?;

    assert_eq!(code, 1);
    let stale = json["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|finding| finding["kind"] == "stale-suppression")
        .ok_or("expected a stale-suppression finding")?;
    assert_eq!(stale["line"], 3);
    assert_eq!(
        stale["message"],
        "Suppression covers line 4, but the finding it names is reported on line 5: \
// dart-decimate-ignore-next-line unused-enum-member"
    );

    Ok(())
}

#[test]
fn equidistant_candidates_resolve_to_the_lower_line() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/live.dart';\nvoid main() { runLive(); }\n",
    )?;
    write(
        &fixture,
        "lib/src/live.dart",
        "\
enum Mode {
  alpha,
  // filler comment
  // dart-decimate-ignore-next-line unused-enum-member
  // padding line
  beta,
  live,
}
void runLive() { print(Mode.live); }
",
    )?;

    let (_, json) = run_json([
        "dart-decimate",
        "dead-code",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
    ])?;

    // Lines 2 and 6 are both two lines from the directive on line 4.
    let stale = json["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|finding| finding["kind"] == "stale-suppression")
        .ok_or("expected a stale-suppression finding")?;
    assert_eq!(
        stale["message"],
        "Suppression covers line 5, but the finding it names is reported on line 2: \
// dart-decimate-ignore-next-line unused-enum-member"
    );

    Ok(())
}

#[test]
fn stale_suppression_rule_can_be_disabled() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        ".dart-decimaterc.json",
        "{\"rules\":{\"stale-suppression\":\"off\"}}\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "// dart-decimate-ignore-next-line dead-file\nvoid main() {}\n",
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
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["findings"], 0);
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn missing_suppression_reason_reports_when_rule_enabled() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        ".dart-decimaterc.json",
        "{\"rules\":{\"missing-suppression-reason\":\"warn\"}}\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "// dart-decimate-ignore-next-line feature-flag\nconst beta = bool.fromEnvironment('FEATURE_BETA');\n",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "flags",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["missing_suppression_reasons"], 1);
    assert_eq!(
        json["findings"][0]["rule_id"],
        "dart-decimate/missing-suppression-reason"
    );
    assert_eq!(json["findings"][0]["kind"], "missing-suppression-reason");
    assert_eq!(json["findings"][0]["severity"], "warning");
    assert_eq!(json["findings"][0]["safe_to_delete"], false);

    Ok(())
}

#[test]
fn documented_suppression_reason_is_accepted() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        ".dart-decimaterc.json",
        "{\"rules\":{\"missing-suppression-reason\":\"error\"}}\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "// dart-decimate-ignore-next-line feature-flag -- platform rollout flag\nconst beta = bool.fromEnvironment('FEATURE_BETA');\n",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "flags",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["missing_suppression_reasons"], 0);
    assert_eq!(json["summary"]["findings"], 0);

    Ok(())
}

fn run_json<const N: usize>(args: [&str; N]) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(args, &mut output)?;
    Ok((code, serde_json::from_slice::<Value>(&output)?))
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
