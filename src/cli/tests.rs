use std::fs;

use serde_json::Value;
use tempfile::TempDir;

use super::*;

#[test]
fn dead_code_command_emits_json_contract() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'live.dart';\nvoid main() {}\n",
    )?;
    write(&fixture, "lib/live.dart", "class Live {}\n")?;
    write(&fixture, "lib/dead.dart", "class Dead {}\n")?;
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
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["schema_version"], "dart-decimate.report.v1");
    assert_eq!(json["kind"], "dead-code");
    assert_eq!(json["command"], "dead-code");
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["dead_files"], 1);
    assert_eq!(json["findings"][0]["rule_id"], "dart-decimate/dead-file");
    assert_eq!(json["findings"][0]["path"], "lib/dead.dart");
    assert_eq!(json["findings"][0]["safe_to_delete"], true);
    assert_eq!(json["findings"][0]["actions"][0]["type"], "delete-file");
    assert_eq!(
        json["findings"][0]["actions"][0]["target_path"],
        "lib/dead.dart"
    );
    assert_eq!(
        json["findings"][0]["actions"][0]["command"],
        "dart-decimate inspect --format json --file lib/dead.dart"
    );
    assert_eq!(json["findings"][0]["actions"][0]["auto_fixable"], true);

    Ok(())
}

#[test]
fn cycles_command_reports_circular_dependencies() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/a.dart", "import 'b.dart';\nclass A {}\n")?;
    write(&fixture, "lib/b.dart", "import 'a.dart';\nclass B {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "cycles",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["command"], "cycles");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_eq!(
        json["findings"][0]["rule_id"],
        "dart-decimate/circular-dependency"
    );
    assert_eq!(json["findings"][0]["files"][0], "lib/a.dart");
    assert_eq!(json["findings"][0]["files"][1], "lib/b.dart");

    Ok(())
}

#[test]
fn cycles_command_reports_re_export_cycles_separately() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/a.dart", "export 'b.dart';\n")?;
    write(&fixture, "lib/b.dart", "export 'a.dart';\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "cycles",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(findings) = json["findings"].as_array() else {
        panic!("findings array");
    };
    let rule_ids = findings
        .iter()
        .map(|finding| finding["rule_id"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["re_export_cycles"], 1);
    assert!(rule_ids.contains(&"dart-decimate/circular-dependency"));
    assert!(rule_ids.contains(&"dart-decimate/re-export-cycle"));

    Ok(())
}

#[test]
fn dead_code_command_reports_unused_exports_with_safe_fix_action()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/live.dart';\nvoid main() { Used(); }\n",
    )?;
    write(
        &fixture,
        "lib/src/live.dart",
        "class Used {}\nclass Unused {}\n",
    )?;
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
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "dart-decimate/unused-export")
    }) else {
        panic!("unused export finding");
    };
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["unused_exports"], 1);
    assert_eq!(finding["path"], "lib/src/live.dart");
    assert_eq!(finding["line"], 2);
    assert_eq!(finding["safe_to_delete"], true);
    assert_eq!(finding["actions"][0]["action"], "remove-declaration");
    assert_eq!(finding["actions"][0]["type"], "remove-declaration");
    assert_eq!(finding["actions"][0]["target_path"], "lib/src/live.dart");
    assert_eq!(finding["actions"][0]["target_symbol"], "Unused");
    assert_eq!(finding["actions"][0]["target_end_line"], 2);
    assert_eq!(finding["actions"][0]["auto_fixable"], true);
    assert_eq!(finding["actions"][1]["action"], "trace-symbol");
    assert_eq!(finding["actions"][1]["type"], "trace-symbol");
    assert_eq!(finding["actions"][1]["target_path"], "lib/src/live.dart");
    assert_eq!(finding["actions"][1]["target_symbol"], "Unused");
    assert_eq!(
        finding["actions"][1]["command"],
        "dart-decimate inspect --format json --symbol lib/src/live.dart:Unused"
    );
    assert_eq!(
        finding["actions"][1]["suppression_comment"],
        "// dart-decimate-ignore-next-line unused-export"
    );
    assert_eq!(finding["actions"][1]["auto_fixable"], false);
    assert_eq!(json["next_steps"][0]["id"], "trace-unused-export");
    assert_eq!(
        json["next_steps"][0]["command"],
        "dart-decimate trace-symbol --format json --symbol lib/src/live.dart:Unused"
    );

    Ok(())
}

#[test]
fn check_command_reports_duplicate_exports() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "export 'src/a.dart';\nexport 'src/b.dart';\n",
    )?;
    write(&fixture, "lib/src/a.dart", "class Api {}\n")?;
    write(&fixture, "lib/src/b.dart", "class Api {}\n")?;
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
    assert_eq!(json["summary"]["duplicate_exports"], 1);
    assert_eq!(
        json["findings"][0]["rule_id"],
        "dart-decimate/duplicate-export"
    );
    assert_eq!(json["findings"][0]["kind"], "duplicate-export");
    assert_eq!(json["findings"][0]["path"], "lib/package.dart");
    assert_eq!(json["findings"][0]["safe_to_delete"], false);
    assert_eq!(json["findings"][0]["files"][0], "lib/src/a.dart");
    assert_eq!(json["findings"][0]["files"][1], "lib/src/b.dart");
    assert_eq!(
        json["findings"][0]["actions"][0]["action"],
        "inspect-export-surface"
    );
    assert_eq!(json["findings"][0]["actions"][0]["auto_fixable"], false);

    Ok(())
}

#[test]
fn check_command_emits_html_report() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/a.dart", "import 'b.dart';\nclass A {}\n")?;
    write(&fixture, "lib/b.dart", "import 'a.dart';\nclass B {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "html",
        ],
        &mut output,
    )?;

    let html = String::from_utf8(output)?;
    assert_eq!(code, 1);
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("<h1>check report</h1>"));
    assert!(html.contains("Circular dependency"));
    assert!(html.contains("Why"));
    assert!(html.contains("Best"));
    assert!(html.contains("lib/a.dart"));

    Ok(())
}

#[test]
fn open_html_rejects_explicit_non_html_format() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let mut output = Vec::new();

    let result = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--open",
        ],
        &mut output,
    );
    let Err(error) = result else {
        panic!("json reports cannot be opened as HTML");
    };

    assert!(matches!(error, CliError::HtmlOpenRequiresHtml));
    assert!(output.is_empty());

    Ok(())
}

#[test]
fn check_command_reports_boundaries_and_unresolved_imports()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'domain/service.dart';\nvoid main() {}\n",
    )?;
    write(
        &fixture,
        "lib/domain/service.dart",
        "import '../ui/widget.dart';\nimport 'missing.dart';\nclass Service {}\n",
    )?;
    write(&fixture, "lib/ui/widget.dart", "class Widget {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--boundary",
            "lib/domain:lib/ui",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(findings) = json["findings"].as_array() else {
        panic!("findings array");
    };
    let rule_ids = findings
        .iter()
        .map(|finding| finding["rule_id"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(code, 1);
    assert!(rule_ids.contains(&"dart-decimate/boundary-violation"));
    assert!(rule_ids.contains(&"dart-decimate/unresolved-dependency"));
    assert_eq!(json["summary"]["boundary_violations"], 1);
    assert_eq!(json["summary"]["unresolved_dependencies"], 1);

    Ok(())
}

#[test]
fn dead_code_command_includes_pub_dependency_hygiene() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  http: ^1.0.0\n  path: ^1.0.0\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:http/http.dart';\n\
import 'package:collection/collection.dart';\n\
void main() {}\n",
    )?;
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
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(findings) = json["findings"].as_array() else {
        panic!("findings array");
    };
    let rule_ids = findings
        .iter()
        .map(|finding| finding["rule_id"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(code, 1);
    assert!(rule_ids.contains(&"dart-decimate/unused-dependency"));
    assert!(rule_ids.contains(&"dart-decimate/unlisted-dependency"));
    assert_eq!(json["summary"]["unused_dependencies"], 1);
    assert_eq!(json["summary"]["unlisted_dependencies"], 1);
    let Some(next_steps) = json["next_steps"].as_array() else {
        panic!("next_steps array");
    };
    assert!(next_steps.iter().any(|step| {
        step["id"] == "trace-unused-dependency"
            && step["command"] == "dart-decimate trace-dependency --format json --dependency path"
    }));

    Ok(())
}

#[test]
fn unlisted_dependency_json_edge_kind_preserves_export() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "export 'package:collection/collection.dart';\nvoid main() {}\n",
    )?;
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
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "dart-decimate/unlisted-dependency")
    }) else {
        panic!("unlisted dependency finding");
    };
    assert_eq!(code, 1);
    assert_eq!(finding["edge"]["kind"], "export");

    Ok(())
}

#[test]
fn check_command_uses_path_dependencies_without_reporting_their_findings()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "app/pubspec.yaml",
        "name: app\n\
dependencies:\n  shared:\n    path: ../shared\n",
    )?;
    write(
        &fixture,
        "app/lib/main.dart",
        "import 'package:shared/shared.dart';\nvoid main() {}\n",
    )?;
    write(&fixture, "shared/pubspec.yaml", "name: shared\n")?;
    write(
        &fixture,
        "shared/lib/shared.dart",
        "import 'loop.dart';\nimport 'missing.dart';\nclass Shared {}\n",
    )?;
    write(
        &fixture,
        "shared/lib/loop.dart",
        "import 'shared.dart';\nclass Loop {}\n",
    )?;
    write(&fixture, "shared/lib/dead.dart", "class SharedDead {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().join("app").to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["unresolved_dependencies"], 0);
    assert_eq!(json["summary"]["dead_files"], 0);
    assert_eq!(json["summary"]["cycles"], 0);
    assert_eq!(json["summary"]["findings"], 0);

    Ok(())
}

#[test]
fn check_command_treats_tests_as_default_entry_points() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(&fixture, "test/all_tests.dart", "void main() {}\n")?;
    write(&fixture, "test/home_test.dart", "void main() {}\n")?;
    write(
        &fixture,
        "integration_test/app_test.dart",
        "void main() {}\n",
    )?;
    write(
        &fixture,
        "test_driver/integration_test.dart",
        "void main() {}\n",
    )?;
    write(&fixture, "tool/grind.dart", "void main() {}\n")?;
    write(&fixture, "pigeon/schema.dart", "class Schema {}\n")?;
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
    assert_eq!(json["summary"]["dead_files"], 0);
    assert_eq!(json["summary"]["findings"], 0);

    Ok(())
}

#[test]
fn check_command_treats_public_library_files_as_default_entry_points()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "import 'src/live.dart';\nclass Package {}\n",
    )?;
    write(&fixture, "lib/src/live.dart", "class Live {}\n")?;
    write(&fixture, "lib/src/dead.dart", "class Dead {}\n")?;
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
    let Some(findings) = json["findings"].as_array() else {
        panic!("findings array");
    };
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["dead_files"], 1);
    assert_eq!(findings[0]["path"], "lib/src/dead.dart");

    Ok(())
}

#[test]
fn output_alias_commands_run_check_formats() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/a.dart", "import 'b.dart';\nclass A {}\n")?;
    write(&fixture, "lib/b.dart", "import 'a.dart';\nclass B {}\n")?;
    let root = fixture.path().to_str().unwrap_or(".");

    let mut output = Vec::new();
    let code = run_from(["dart-decimate", "json", root], &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["command"], "check");
    assert_eq!(json["summary"]["cycles"], 1);

    output.clear();
    let code = run_from(["dart-decimate", "html", root, "--stdout"], &mut output)?;
    let html = String::from_utf8(std::mem::take(&mut output))?;
    assert_eq!(code, 1);
    assert!(html.starts_with("<!doctype html>"));
    assert!(html.contains("Circular dependency"));

    let code = run_from(["dart-decimate", "human", root], &mut output)?;
    let human = String::from_utf8(output)?;
    assert_eq!(code, 1);
    assert!(human.contains("Dart Decimate check"));
    assert!(human.contains("Why"));

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
