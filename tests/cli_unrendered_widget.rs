use std::fs;

use dart_decimate::cli::run_from;
use serde_json::{Value, json};
use tempfile::TempDir;

#[test]
fn check_reports_unrendered_widget_class() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = unrendered_widget_fixture()?;
    let mut output = Vec::new();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["unrendered_widgets"], 1);

    let finding = unrendered_widget_finding(&json);
    assert_eq!(finding["kind"], "unrendered-widget");
    assert_eq!(finding["severity"], "warning");
    assert_eq!(finding["path"], "lib/widgets.dart");
    assert_eq!(finding["line"], 3);
    assert_eq!(finding["safe_to_delete"], false);
    assert_eq!(finding["files"], json!([]));
    assert_eq!(finding["edge"], Value::Null);
    assert_eq!(finding["actions"][0]["action"], "trace-widget-reachability");
    assert_eq!(finding["actions"][0]["auto_fixable"], false);
    assert_eq!(finding["actions"][0]["target_symbol"], "DeadCard");
    assert_eq!(
        finding["actions"][0]["suppression_comment"],
        "// dart-decimate-ignore-next-line unrendered-widget"
    );
    for class_name in [
        "UsedCard",
        "LegacyCard",
        "NamedCard",
        "PrefixedCard",
        "GeneratedDead",
        "TestDead",
        "DeadScreen",
    ] {
        assert_no_unrendered_widget_for(&json, class_name);
    }

    Ok(())
}

#[test]
fn unrendered_widget_rule_can_error_or_turn_off() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = unrendered_widget_fixture()?;
    write(
        &fixture,
        ".dart-decimaterc.json",
        r#"{ "rules": { "unused-export": "off", "dead-file": "off", "unrendered-widget": "error" } }"#,
    )?;
    let mut output = Vec::new();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(unrendered_widget_finding(&json)["severity"], "error");

    write(
        &fixture,
        ".dart-decimaterc.json",
        r#"{ "rules": { "unused-export": "off", "dead-file": "off", "unused-component": "off" } }"#,
    )?;
    output.clear();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["unrendered_widgets"], 0);
    assert_no_unrendered_widget_for(&json, "DeadCard");

    Ok(())
}

#[test]
fn check_skips_public_exported_package_widgets() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package_app\n")?;
    write(
        &fixture,
        ".dart-decimaterc.json",
        r#"{ "rules": { "dead-file": "off", "unused-export": "off" } }"#,
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package_app.dart';\nvoid main() {}\n",
    )?;
    write(
        &fixture,
        "lib/package_app.dart",
        "export 'src/package_button.dart';\n",
    )?;
    write(
        &fixture,
        "lib/src/package_button.dart",
        r"class PackageButton extends StatelessWidget {
  const PackageButton({super.key});
  Widget build(BuildContext context) => const SizedBox();
}
",
    )?;
    let mut output = Vec::new();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["unrendered_widgets"], 0);
    assert_no_unrendered_widget_for(&json, "PackageButton");

    Ok(())
}

#[test]
fn check_counts_widgets_rendered_from_generic_builder_callbacks()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        ".dart-decimaterc.json",
        r#"{ "rules": { "unused-export": "off", "dead-file": "off" } }"#,
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'routine_concept_flow.dart';\nvoid main() { const RoutineConceptFlow(); }\n",
    )?;
    write(
        &fixture,
        "lib/routine_concept_flow.dart",
        r"import 'routine_concept_card.dart';

class RoutineConceptItem {
  const RoutineConceptItem(this.title);
  final String title;
}

class RoutineConceptFlow extends StatelessWidget {
  const RoutineConceptFlow({super.key});

  Widget build(BuildContext context) => BentoList<RoutineConceptItem>(
    items: const [RoutineConceptItem('one')],
    itemBuilder: (context, item, _) => RoutineConceptCard(title: item.title),
  );
}
",
    )?;
    write(
        &fixture,
        "lib/routine_concept_card.dart",
        r"class RoutineConceptCard extends StatelessWidget {
  const RoutineConceptCard({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) => Text(title);
}

class UnusedConceptCard extends StatelessWidget {
  const UnusedConceptCard({super.key});
  Widget build(BuildContext context) => const SizedBox();
}
",
    )?;
    let mut output = Vec::new();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;

    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&output));
    assert_no_unrendered_widget_for(&json, "RoutineConceptCard");
    assert_unrendered_widget_for(&json, "UnusedConceptCard");

    Ok(())
}

fn unrendered_widget_fixture() -> Result<TempDir, std::io::Error> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )?;
    write(
        &fixture,
        ".dart-decimaterc.json",
        r#"{ "rules": { "unused-export": "off", "dead-file": "off" } }"#,
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"import 'widgets.dart';
import 'prefixed.dart' as ui;
import 'generated_dead.g.dart';

void main() {
  const UsedCard();
  new LegacyCard();
  NamedCard.primary();
  ui.PrefixedCard();
}
",
    )?;
    write(
        &fixture,
        "lib/widgets.dart",
        r"import 'package:flutter/material.dart';

class DeadCard extends StatelessWidget {
  const DeadCard({super.key});
  Widget build(BuildContext context) => const SizedBox();
}

class UsedCard extends StatelessWidget {
  const UsedCard({super.key});
  Widget build(BuildContext context) => const SizedBox();
}

class LegacyCard extends StatelessWidget {
  LegacyCard({super.key});
  Widget build(BuildContext context) => const SizedBox();
}

class NamedCard extends StatelessWidget {
  const NamedCard.primary({super.key});
  Widget build(BuildContext context) => const SizedBox();
}
",
    )?;
    write(
        &fixture,
        "lib/prefixed.dart",
        r"import 'package:flutter/material.dart';

class PrefixedCard extends StatelessWidget {
  const PrefixedCard({super.key});
  Widget build(BuildContext context) => const SizedBox();
}
",
    )?;
    write(
        &fixture,
        "lib/generated_dead.g.dart",
        r"import 'package:flutter/material.dart';

class GeneratedDead extends StatelessWidget {
  const GeneratedDead({super.key});
  Widget build(BuildContext context) => const SizedBox();
}
",
    )?;
    write(
        &fixture,
        "test/widget_test.dart",
        r"class TestDead extends StatelessWidget {
  const TestDead({super.key});
  Widget build(BuildContext context) => const SizedBox();
}
",
    )?;
    write(
        &fixture,
        "lib/dead_screen.dart",
        r"class DeadScreen extends StatelessWidget {
  const DeadScreen({super.key});
  Widget build(BuildContext context) => const SizedBox();
}
",
    )?;
    Ok(fixture)
}

fn run_check(fixture: &TempDir, output: &mut Vec<u8>) -> Result<i32, Box<dyn std::error::Error>> {
    Ok(run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
        output,
    )?)
}

fn unrendered_widget_finding(json: &Value) -> &Value {
    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "dart-decimate/unrendered-widget")
    }) else {
        panic!("unrendered widget finding");
    };
    finding
}

fn assert_no_unrendered_widget_for(json: &Value, class_name: &str) {
    assert!(json["findings"].as_array().is_some_and(|findings| {
        findings.iter().all(|finding| {
            finding["rule_id"] != "dart-decimate/unrendered-widget"
                || finding["actions"][0]["target_symbol"] != class_name
        })
    }));
}

fn assert_unrendered_widget_for(json: &Value, class_name: &str) {
    assert!(json["findings"].as_array().is_some_and(|findings| {
        findings.iter().any(|finding| {
            finding["rule_id"] == "dart-decimate/unrendered-widget"
                && finding["actions"][0]["target_symbol"] == class_name
        })
    }));
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
