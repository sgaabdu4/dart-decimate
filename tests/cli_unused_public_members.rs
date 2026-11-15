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

fn write_app_with_unused_member(fixture: &TempDir, pubspec: &str) -> Result<(), std::io::Error> {
    write(fixture, "pubspec.yaml", pubspec)?;
    write(
        fixture,
        "lib/main.dart",
        "import 'features/cleaner.dart';\n\nvoid main() {\n  print(Cleaner().clean(' x '));\n}\n",
    )?;
    write(
        fixture,
        "lib/features/cleaner.dart",
        "class Cleaner {\n  String clean(String text) => text.trim();\n\n  int spare() => 1;\n}\n",
    )
}

#[test]
fn strict_check_fails_an_unused_member_of_a_class_in_an_unpublished_package()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_app_with_unused_member(&fixture, "name: app\npublish_to: none\n")?;

    let (code, json) = strict_check(&fixture)?;

    assert_eq!(code, 1);
    assert_eq!(json["summary"]["unused_class_members"], 1);
    assert!(member_messages(&json)[0].contains("Cleaner.spare"));

    Ok(())
}

#[test]
fn strict_check_fails_an_unused_member_of_a_class_in_a_flutter_app()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_app_with_unused_member(
        &fixture,
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )?;

    let (code, json) = strict_check(&fixture)?;

    assert_eq!(code, 1);
    assert_eq!(json["summary"]["unused_class_members"], 1);

    Ok(())
}

#[test]
fn strict_check_keeps_the_members_of_a_published_package_library()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_app_with_unused_member(&fixture, "name: app\n")?;

    let (code, json) = strict_check(&fixture)?;

    assert_eq!(code, 0, "{:?}", member_messages(&json));
    assert_eq!(json["summary"]["unused_class_members"], 0);

    Ok(())
}

#[test]
fn strict_check_keeps_the_members_of_an_unpublished_library_that_has_no_entry_point()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: shared\npublish_to: none\n")?;
    write(
        &fixture,
        "lib/tables.dart",
        "class Fields {\n  static const String first = 'first';\n\n  static const String second = 'second';\n}\n",
    )?;

    let (code, json) = strict_check(&fixture)?;

    assert_eq!(code, 0, "{:?}", member_messages(&json));
    assert_eq!(json["summary"]["unused_class_members"], 0);

    Ok(())
}

#[test]
fn strict_check_fails_an_unused_member_of_a_workspace_package_from_the_root()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: root\nenvironment:\n  sdk: ^3.6.0\nworkspace:\n  - packages/app\n  - packages/kit\n",
    )?;
    write(
        &fixture,
        "packages/app/pubspec.yaml",
        "name: app\npublish_to: none\nresolution: workspace\nenvironment:\n  sdk: ^3.6.0\n",
    )?;
    write(
        &fixture,
        "packages/app/lib/main.dart",
        "import 'features/cleaner.dart';\n\nvoid main() {\n  print(Cleaner().clean(' x '));\n}\n",
    )?;
    write(
        &fixture,
        "packages/app/lib/features/cleaner.dart",
        "class Cleaner {\n  String clean(String text) => text.trim();\n\n  int spare() => 1;\n}\n",
    )?;
    write(
        &fixture,
        "packages/kit/pubspec.yaml",
        "name: kit\nresolution: workspace\nenvironment:\n  sdk: ^3.6.0\n",
    )?;
    write(
        &fixture,
        "packages/kit/lib/kit.dart",
        "import 'src/tool.dart';\n\nvoid run() {\n  Tool().work();\n}\n",
    )?;
    write(
        &fixture,
        "packages/kit/lib/src/tool.dart",
        "class Tool {\n  int work() => 1;\n\n  int idle() => 2;\n}\n",
    )?;

    let (code, json) = strict_check(&fixture)?;

    assert_eq!(code, 1);
    let messages = member_messages(&json);
    assert_eq!(messages.len(), 2, "{messages:?}");
    assert!(
        messages
            .iter()
            .any(|message| message.contains("Cleaner.spare"))
    );
    assert!(messages.iter().any(|message| message.contains("Tool.idle")));

    Ok(())
}

#[test]
fn strict_check_keeps_members_that_callers_and_contracts_use_in_an_unpublished_package()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\npublish_to: none\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'features/helper.dart';\nimport 'features/plain.dart';\n\nvoid main() {\n  final plain = Plain();\n  plain.viaMain();\n  print(plain.more());\n  print(Host().run(plain));\n  print(plain);\n}\n",
    )?;
    write(
        &fixture,
        "lib/features/plain.dart",
        "part 'plain_part.dart';\n\nclass Plain {\n  int viaMain() => 1;\n  int viaTest() => 2;\n  int viaIntegration() => 3;\n  int viaBin() => 4;\n  int viaExtension() => 5;\n  int viaPart() => 6;\n  int viaMixin() => 7;\n\n  Map<String, Object?> toJson() => {};\n\n  @override\n  String toString() => 'plain';\n\n  @override\n  bool operator ==(Object other) => other is Plain;\n\n  @override\n  int get hashCode => 1;\n}\n",
    )?;
    write(
        &fixture,
        "lib/features/plain_part.dart",
        "part of 'plain.dart';\n\nint callVia(Plain plain) => plain.viaPart();\n",
    )?;
    write(
        &fixture,
        "lib/features/helper.dart",
        "import 'plain.dart';\n\nextension PlainTools on Plain {\n  int more() => viaExtension() + callVia(this);\n}\n\nmixin Runner {\n  int run(Plain plain) => plain.viaMixin();\n}\n\nclass Host with Runner {}\n",
    )?;
    write(
        &fixture,
        "test/plain_test.dart",
        "import 'package:app/features/plain.dart';\n\nvoid main() {\n  Plain().viaTest();\n}\n",
    )?;
    write(
        &fixture,
        "integration_test/flow_test.dart",
        "import 'package:app/features/plain.dart';\n\nvoid main() {\n  Plain().viaIntegration();\n}\n",
    )?;
    write(
        &fixture,
        "bin/tool.dart",
        "import 'package:app/features/plain.dart';\n\nvoid main() {\n  Plain().viaBin();\n}\n",
    )?;

    let (_, json) = strict_check(&fixture)?;

    assert_eq!(
        json["summary"]["unused_class_members"],
        0,
        "{:?}",
        member_messages(&json)
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
