use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn dead_code_unused_exports_filter_limits_visible_findings()
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
    write(&fixture, "lib/src/dead.dart", "class Dead {}\n")?;

    let (code, json) = run_json([
        "dart-decimate",
        "dead-code",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--unused-exports",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["summary"]["findings"], 1);
    assert_eq!(json["summary"]["dead_files"], 0);
    assert_eq!(json["summary"]["unused_exports"], 1);
    assert_eq!(json["findings"][0]["kind"], "unused-export");
    assert_eq!(json["findings"][0]["path"], "lib/src/live.dart");
    assert!(json["next_steps"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn dead_code_unused_deps_filter_groups_dependency_variants()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  path: ^1.0.0\n  args: ^2.0.0\n\
dev_dependencies:\n  lints: ^5.0.0\n  collection: ^1.0.0\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:args/args.dart';\n\
import 'package:collection/collection.dart';\n\
void main() {}\n",
    )?;
    write(
        &fixture,
        "test/app_test.dart",
        "import 'package:args/args.dart';\nvoid main() {}\n",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "dead-code",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--unused-deps",
    ])?;

    let kinds = finding_kinds(&json);
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["findings"], 3);
    assert_eq!(json["summary"]["unused_dependencies"], 3);
    assert!(kinds.contains(&"unused-dependency"));
    assert!(kinds.contains(&"unused-dev-dependency"));
    assert!(kinds.contains(&"dev-dependency-in-production"));
    assert!(kinds.iter().all(|kind| {
        matches!(
            *kind,
            "unused-dependency"
                | "unused-dev-dependency"
                | "dev-dependency-in-production"
                | "test-only-dependency"
        )
    }));

    Ok(())
}

#[test]
fn check_unresolved_import_filter_preserves_only_unresolved_findings()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'missing.dart';\nconst beta = bool.fromEnvironment('FEATURE_BETA');\nvoid main() {}\n",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--unresolved-imports",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["summary"]["findings"], 1);
    assert_eq!(json["summary"]["unresolved_dependencies"], 1);
    assert_eq!(json["summary"]["feature_flags"], 0);
    assert_eq!(json["findings"][0]["kind"], "unresolved-dependency");

    Ok(())
}

#[test]
fn check_private_type_leaks_filter_hides_other_findings() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package.dart';\nvoid main() { Api(); }\n",
    )?;
    write(
        &fixture,
        "lib/package.dart",
        "class Api extends _Hidden {}\nclass _Hidden {}\nclass Unused {}\n",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--private-type-leaks",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["summary"]["findings"], 1);
    assert_eq!(json["summary"]["private_type_leaks"], 1);
    assert_eq!(json["summary"]["unused_exports"], 0);
    assert_eq!(json["findings"][0]["kind"], "private-type-leak");

    Ok(())
}

#[test]
fn check_circular_deps_filter_preserves_only_cycles() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'a.dart';\nconst beta = bool.fromEnvironment('FEATURE_BETA');\nvoid main() { a(); }\n",
    )?;
    write(
        &fixture,
        "lib/a.dart",
        "import 'b.dart';\nvoid a() { b(); }\n",
    )?;
    write(
        &fixture,
        "lib/b.dart",
        "import 'a.dart';\nvoid b() { a(); }\n",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--circular-deps",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["summary"]["findings"], 1);
    assert_eq!(json["summary"]["cycles"], 1);
    assert_eq!(json["summary"]["feature_flags"], 0);
    assert_eq!(json["findings"][0]["kind"], "circular-dependency");

    Ok(())
}

#[test]
fn check_boundary_violations_filter_groups_boundary_family()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/domain/order.dart",
        "import '../ui/order_card.dart';\nvoid order() { render(); }\n",
    )?;
    write(&fixture, "lib/ui/order_card.dart", "void render() {}\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'domain/order.dart';\nconst beta = bool.fromEnvironment('FEATURE_BETA');\nvoid main() { order(); }\n",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--boundary",
        "lib/domain:lib/ui",
        "--boundary-violations",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["summary"]["findings"], 1);
    assert_eq!(json["summary"]["boundary_violations"], 1);
    assert_eq!(json["summary"]["feature_flags"], 0);
    assert_eq!(json["findings"][0]["kind"], "boundary-violation");

    Ok(())
}

#[test]
fn check_policy_violations_filter_uses_existing_policy_flag()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'missing.dart';\nimport 'dart:io';\nvoid main() { Process.runSync('sh', []); }\n",
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
        "--entry",
        "lib/main.dart",
        "--policy-pack",
        "policy.jsonc",
        "--policy-violations",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["summary"]["findings"], 1);
    assert_eq!(json["summary"]["policy_violations"], 1);
    assert_eq!(json["summary"]["unresolved_dependencies"], 0);
    assert_eq!(json["findings"][0]["kind"], "policy-violation");

    Ok(())
}

#[test]
fn dead_code_rejects_check_only_issue_filters() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;

    let mut output = Vec::new();
    let error = run_from(
        [
            "dart-decimate",
            "dead-code",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
            "--circular-deps",
        ],
        &mut output,
    )
    .err()
    .map(|error| error.to_string());

    assert!(matches!(
        error.as_deref(),
        Some(message) if message.contains("unexpected argument '--circular-deps'")
    ));

    Ok(())
}

#[test]
fn dead_code_combines_multiple_filter_flags() -> Result<(), Box<dyn std::error::Error>> {
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
        "class Used {}\ntypedef Alias = String;\nclass Unused {}\n",
    )?;
    write(&fixture, "lib/src/dead.dart", "class Dead {}\n")?;

    let (code, json) = run_json([
        "dart-decimate",
        "dead-code",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--unused-types",
        "--unused-files",
    ])?;

    let kinds = finding_kinds(&json);
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["findings"], 2);
    assert_eq!(json["summary"]["dead_files"], 1);
    assert_eq!(json["summary"]["unused_types"], 1);
    assert!(kinds.contains(&"dead-file"));
    assert!(kinds.contains(&"unused-type"));
    assert!(!kinds.contains(&"unused-export"));

    Ok(())
}

#[test]
fn schema_lists_dead_code_issue_filter_flags() -> Result<(), Box<dyn std::error::Error>> {
    let (_, json) = run_json(["dart-decimate", "schema", "--format", "json"])?;
    assert_manifest_flags(
        &json,
        "dead-code",
        &["--unused-files", "--unused-exports", "--unused-deps"],
    );
    assert_manifest_flags(
        &json,
        "check",
        &[
            "--unresolved-imports",
            "--stale-suppressions",
            "--circular-deps",
            "--re-export-cycles",
            "--boundary-violations",
        ],
    );

    Ok(())
}

fn run_json<const N: usize>(args: [&str; N]) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(args, &mut output)?;
    Ok((code, serde_json::from_slice::<Value>(&output)?))
}

fn finding_kinds(json: &Value) -> Vec<&str> {
    json["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|finding| finding["kind"].as_str())
        .collect()
}

fn assert_manifest_flags(json: &Value, command_name: &str, expected: &[&str]) {
    let command = json["commands"]
        .as_array()
        .and_then(|commands| {
            commands
                .iter()
                .find(|command| command["name"] == command_name)
        })
        .unwrap_or_else(|| panic!("missing manifest command {command_name}"));
    for flag in expected {
        assert!(
            command["flags"]
                .as_array()
                .is_some_and(|flags| flags.iter().any(|candidate| candidate == flag)),
            "{command_name} missing {flag}"
        );
    }
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
