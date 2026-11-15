use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn strict_check_fails_an_unused_public_class_member() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/cleaner.dart';\n\nvoid main() {\n  print(Cleaner().clean(' x '));\n}\n",
    )?;
    write(
        &fixture,
        "lib/src/cleaner.dart",
        "class Cleaner {\n  String clean(String text) => text.trim();\n\n  int spare() => 1;\n\n  int get extra => 2;\n\n  int count = 0;\n}\n",
    )?;

    let (code, json) = strict_check(&fixture)?;

    assert_eq!(code, 1);
    assert_eq!(json["summary"]["unused_class_members"], 3);
    let owners_and_names = member_messages(&json);
    for name in ["spare", "extra", "count"] {
        assert!(
            owners_and_names
                .iter()
                .any(|message| message.contains(name)),
            "missing finding for {name}: {owners_and_names:?}"
        );
    }
    assert!(
        !owners_and_names
            .iter()
            .any(|message| message.contains("clean")),
        "clean is used: {owners_and_names:?}"
    );

    Ok(())
}

#[test]
fn strict_check_keeps_members_that_a_contract_or_another_file_uses()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/api.dart';\nimport 'src/generated.dart';\nimport 'src/plain.dart';\nimport 'src/shapes.dart';\n\nvoid main() {\n  Square(2).area();\n  Generated();\n  Api();\n  final Plain plain = Plain();\n  print(plain.used());\n}\n",
    )?;
    write(&fixture, "lib/package.dart", "export 'src/api.dart';\n")?;
    write(
        &fixture,
        "lib/src/api.dart",
        "class Api {\n  int extra() => 1;\n}\n",
    )?;
    write(
        &fixture,
        "lib/src/generated.dart",
        "part 'generated.g.dart';\n\nclass Generated {\n  int value = 0;\n}\n",
    )?;
    write(
        &fixture,
        "lib/src/generated.g.dart",
        "part of 'generated.dart';\n",
    )?;
    write(
        &fixture,
        "lib/src/plain.dart",
        "class Plain {\n  int used() => 1;\n\n  @override\n  String toString() => 'plain';\n\n  Map<String, Object?> toJson() => {};\n}\n",
    )?;
    write(
        &fixture,
        "lib/src/shapes.dart",
        "abstract class Shape {\n  double area();\n}\n\nclass Square extends Shape {\n  Square(this.side);\n\n  final double side;\n\n  @override\n  double area() => side * side;\n\n  double perimeter() => 4 * side;\n}\n",
    )?;

    let (code, json) = strict_check(&fixture)?;

    assert_eq!(code, 0, "{:?}", member_messages(&json));
    assert_eq!(json["summary"]["unused_class_members"], 0);

    Ok(())
}

#[test]
fn unused_member_message_names_the_visibility_of_the_member()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/cleaner.dart';\n\nvoid main() {\n  Cleaner();\n}\n",
    )?;
    write(
        &fixture,
        "lib/src/cleaner.dart",
        "class Cleaner {\n  int spare() => 1;\n\n  int _hidden() => 2;\n}\n",
    )?;

    let (_, json) = strict_check(&fixture)?;

    let messages = member_messages(&json);
    assert_eq!(messages.len(), 2, "{messages:?}");
    assert!(
        messages
            .iter()
            .any(|message| message.starts_with("Unused public class-like member Cleaner.spare")),
        "{messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|message| message.starts_with("Unused private class-like member Cleaner._hidden")),
        "{messages:?}"
    );

    Ok(())
}

fn member_messages(json: &Value) -> Vec<String> {
    json["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|finding| finding["rule_id"] == "dart-decimate/unused-class-member")
        .filter_map(|finding| finding["message"].as_str().map(str::to_owned))
        .collect()
}

fn strict_check(fixture: &TempDir) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--threshold",
            "0",
            "--strict",
            "--format",
            "json",
        ],
        &mut output,
    )?;
    Ok((code, serde_json::from_slice::<Value>(&output)?))
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
