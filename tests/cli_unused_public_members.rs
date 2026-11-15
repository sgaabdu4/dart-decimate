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
fn strict_check_keeps_the_api_of_a_published_flutter_package_with_a_bin_script()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: fancy_splash\nversion: 2.1.0\nenvironment:\n  sdk: ^3.8.0\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )?;
    write(
        &fixture,
        "lib/fancy_splash.dart",
        "export 'src/splash.dart';\n",
    )?;
    write(
        &fixture,
        "lib/src/splash.dart",
        "class SplashConfig {\n  SplashConfig(this.color);\n\n  final int color;\n\n  int get brightness => color >> 8;\n\n  SplashConfig withColor(int value) => SplashConfig(value);\n}\n",
    )?;
    write(
        &fixture,
        "bin/create.dart",
        "import 'package:fancy_splash/fancy_splash.dart';\n\nvoid main() {\n  print(SplashConfig(1).color);\n}\n",
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

#[test]
fn strict_check_keeps_the_api_of_a_package_that_has_a_library_entry_named_after_it()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: toolkit\npublish_to: none\nenvironment:\n  sdk: ^3.8.0\n",
    )?;
    write(&fixture, "lib/toolkit.dart", "export 'src/config.dart';\n")?;
    write(
        &fixture,
        "lib/src/config.dart",
        "class Config {\n  int get level => 1;\n\n  int get spare => 2;\n}\n",
    )?;
    write(
        &fixture,
        "bin/run.dart",
        "import 'package:toolkit/toolkit.dart';\n\nvoid main() {\n  print(Config().level);\n}\n",
    )?;

    let (code, json) = strict_check(&fixture)?;

    assert_eq!(code, 0, "{:?}", member_messages(&json));
    assert_eq!(json["summary"]["unused_class_members"], 0);

    Ok(())
}

fn write_app_with_class_hierarchy(
    fixture: &TempDir,
    items: &str,
    main_body: &str,
) -> Result<(), std::io::Error> {
    write(fixture, "pubspec.yaml", "name: app\npublish_to: none\n")?;
    write(
        fixture,
        "lib/main.dart",
        &format!("import 'src/items.dart';\n\nvoid main() {{\n{main_body}}}\n"),
    )?;
    write(fixture, "lib/src/items.dart", items)
}

#[test]
fn strict_check_keeps_base_members_that_a_subclass_uses_for_an_interface()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_app_with_class_hierarchy(
        &fixture,
        "class Ordered {\n  Ordered(this.rank);\n\n  final int rank;\n\n  int compareTo(Ordered other) => rank - other.rank;\n}\n\nclass Item extends Ordered implements Comparable<Ordered> {\n  Item(super.rank);\n}\n",
        "  final items = [Item(2), Item(1)]..sort();\n  print(items.length);\n",
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

#[test]
fn strict_check_keeps_iterator_members_that_a_base_class_supplies()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_app_with_class_hierarchy(
        &fixture,
        "class CountBase {\n  CountBase(this.left);\n\n  int left;\n\n  int get current => left;\n\n  bool moveNext() => left-- > 0;\n}\n\nclass Countdown extends CountBase implements Iterator<int> {\n  Countdown(super.left);\n}\n\nclass Range extends Iterable<int> {\n  Range(this.n);\n\n  final int n;\n\n  @override\n  Iterator<int> get iterator => Countdown(n);\n}\n",
        "  for (final n in Range(3)) {\n    print(n);\n  }\n",
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

#[test]
fn strict_check_fails_an_unused_member_of_a_base_class_without_an_interface()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_app_with_class_hierarchy(
        &fixture,
        "class Base {\n  int spare() => 1;\n}\n\nclass Child extends Base {\n  Child();\n}\n",
        "  print(Child());\n",
    )?;

    let (_, json) = strict_check(&fixture)?;

    assert_eq!(json["summary"]["unused_class_members"], 1);
    assert!(member_messages(&json)[0].contains("Base.spare"));

    Ok(())
}

#[test]
fn strict_check_keeps_base_members_that_a_subclass_mixes_in_an_interface_for()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_app_with_class_hierarchy(
        &fixture,
        "import 'dart:collection';\n\nclass BagBase {\n  Iterator<int> get iterator => throw UnimplementedError();\n}\n\nclass Bag extends BagBase with IterableMixin<int> {}\n\nclass Store {\n  Iterable<String> get keys => const [];\n\n  void clear() {}\n\n  int? remove(Object? key) => null;\n}\n\nclass Settings extends Store with MapMixin<String, int> {}\n",
        "  for (final n in Bag()) {\n    print(n);\n  }\n  print(Settings());\n",
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

#[test]
fn strict_check_fails_an_unused_member_of_an_implemented_class()
-> Result<(), Box<dyn std::error::Error>> {
    let forms = [
        "class FakeRepo implements Repo {\n  @override\n  int fetch() => 3;\n\n  @override\n  int legacy() => 4;\n}\n",
        "class Mock {}\n\nclass MockRepo extends Mock implements Repo {}\n",
        "class RepoImpl implements Repo {\n  @override\n  int fetch() => 3;\n\n  @override\n  int legacy() => 4;\n}\n",
    ];
    for form in forms {
        let fixture = tempfile::tempdir()?;
        write_app_with_class_hierarchy(
            &fixture,
            "class Repo {\n  int fetch() => 1;\n\n  int legacy() => 2;\n}\n",
            "  print(Repo().fetch());\n",
        )?;
        write(
            &fixture,
            "test/repo_test.dart",
            &format!("import 'package:app/src/items.dart';\n\n{form}"),
        )?;

        let (_, json) = strict_check(&fixture)?;

        assert_eq!(
            json["summary"]["unused_class_members"],
            1,
            "{form}: {:?}",
            member_messages(&json)
        );
        assert!(member_messages(&json)[0].contains("Repo.legacy"));
    }

    Ok(())
}

#[test]
fn strict_check_fails_an_unused_member_of_a_class_that_shares_a_name_with_a_base_class()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\npublish_to: none\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/a.dart';\nimport 'src/b.dart' as other;\n\nvoid main() {\n  final items = [Item(2), Item(1)]..sort();\n  print(items.length);\n  print(other.Ordered());\n}\n",
    )?;
    write(
        &fixture,
        "lib/src/a.dart",
        "class Ordered {\n  Ordered(this.rank);\n\n  final int rank;\n\n  int compareTo(Ordered other) => rank - other.rank;\n}\n\nclass Item extends Ordered implements Comparable<Ordered> {\n  Item(super.rank);\n}\n",
    )?;
    write(
        &fixture,
        "lib/src/b.dart",
        "class Ordered {\n  int reallyUnused() => 1;\n}\n",
    )?;

    let (_, json) = strict_check(&fixture)?;

    assert_eq!(
        json["summary"]["unused_class_members"],
        1,
        "{:?}",
        member_messages(&json)
    );
    assert!(member_messages(&json)[0].contains("Ordered.reallyUnused"));

    Ok(())
}

#[test]
fn strict_check_fails_an_unused_member_of_a_class_that_only_a_project_mixin_extends()
-> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (
            "class Base {\n  int get value => 1;\n\n  int spare() => 0;\n}\n\nmixin M on Base {\n  int twice() => value * 2;\n}\n\nclass A extends Base with M {}\n",
            "  print(A().twice());\n",
            "Base.spare",
        ),
        (
            "class BaseRepo {\n  int fetch() => 1;\n\n  int legacy() => 2;\n}\n\nmixin Logs {\n  void log() {}\n}\n\nclass Repo extends BaseRepo with Logs {}\n",
            "  print(Repo().fetch());\n  Repo().log();\n",
            "BaseRepo.legacy",
        ),
        (
            "mixin class Helper {\n  int used() => 1;\n\n  int spare() => 2;\n}\n\nclass A with Helper {}\n",
            "  print(A().used());\n",
            "Helper.spare",
        ),
    ];
    for (items, main_body, expected) in cases {
        let fixture = tempfile::tempdir()?;
        write_app_with_class_hierarchy(&fixture, items, main_body)?;

        let (_, json) = strict_check(&fixture)?;

        assert_eq!(
            json["summary"]["unused_class_members"],
            1,
            "{expected}: {:?}",
            member_messages(&json)
        );
        assert!(member_messages(&json)[0].contains(expected));
    }

    Ok(())
}

#[test]
fn strict_check_keeps_base_members_for_a_project_mixin_that_implements_an_interface()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_app_with_class_hierarchy(
        &fixture,
        "class Base {\n  int compareTo(Object other) => 0;\n}\n\nmixin M implements Comparable<Object> {}\n\nclass A extends Base with M {}\n",
        "  print(A());\n",
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

#[test]
fn strict_check_follows_a_typedef_supertype_to_its_class() -> Result<(), Box<dyn std::error::Error>>
{
    let cases = [
        (
            "class Ordered {\n  int compareTo(Object other) => 0;\n}\n\ntypedef Alias = Ordered;\n\nclass Item extends Alias implements Comparable<Object> {}\n",
            0,
        ),
        (
            "class Ordered {\n  int compareTo(Object other) => 0;\n}\n\ntypedef Inner<T> = Ordered;\n\ntypedef Alias = Inner<int>;\n\nclass Item extends Alias implements Comparable<Object> {}\n",
            0,
        ),
        (
            "class Ordered {\n  int spare() => 0;\n}\n\ntypedef Alias = Ordered;\n\nclass Item extends Alias {}\n",
            1,
        ),
    ];
    for (items, expected) in cases {
        let fixture = tempfile::tempdir()?;
        write_app_with_class_hierarchy(&fixture, items, "  print(Item());\n")?;

        let (_, json) = strict_check(&fixture)?;

        assert_eq!(
            json["summary"]["unused_class_members"],
            expected,
            "{items}: {:?}",
            member_messages(&json)
        );
    }

    Ok(())
}

#[test]
fn strict_check_finishes_on_a_typedef_loop() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_app_with_class_hierarchy(
        &fixture,
        "abstract class I {}\n\ntypedef A = B;\n\ntypedef B = A;\n\nclass X extends A implements I {}\n",
        "  print(X());\n",
    )?;

    let (code, json) = strict_check(&fixture)?;

    assert_eq!(code, 0, "{:?}", member_messages(&json));

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
