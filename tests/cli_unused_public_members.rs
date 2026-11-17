use std::fs;
use std::path::Path;

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
        "abstract class Shape {\n  double area();\n}\n\nclass Square extends Shape {\n  Square(this.side);\n\n  final double side;\n\n  @override\n  double area() => side * side;\n}\n",
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
fn strict_check_keeps_base_members_for_interfaces_that_the_tool_cannot_read()
-> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        "import 'dart:collection';\n\ntypedef IM = IterableMixin<int>;\n\nclass BagBase {\n  Iterator<int> get iterator => throw UnimplementedError();\n}\n\nclass Bag extends BagBase with IM {}\n",
        "import 'dart:collection';\n\nclass BagBase {\n  Iterator<int> get iterator => throw UnimplementedError();\n}\n\nclass Bag = BagBase with IterableMixin<int>;\n",
        "class Ordered {\n  int compareTo(Object other) => 0;\n}\n\n@Deprecated('x')\ntypedef Alias = Ordered;\n\nclass Item extends Alias implements Comparable<Object> {}\n",
        "class Ordered {\n  int compareTo(Object other) => 0;\n}\n\ntypedef Alias = /* old */ Ordered;\n\nclass Item extends Alias implements Comparable<Object> {}\n",
    ];
    for items in cases {
        let fixture = tempfile::tempdir()?;
        write_app_with_class_hierarchy(&fixture, items, "  print(Bag());\n  print(Item());\n")?;

        let (_, json) = strict_check(&fixture)?;

        assert_eq!(
            json["summary"]["unused_class_members"],
            0,
            "{items}: {:?}",
            member_messages(&json)
        );
    }

    Ok(())
}

#[test]
fn strict_check_protects_only_the_members_that_a_project_interface_declares()
-> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (
            "class Base {\n  String get label => 'b';\n\n  String get spare => 's';\n}\n\nmixin Named {\n  String get label;\n}\n\nclass P extends Base with Named {}\n",
            "  print(P());\n",
            "Base.spare",
            "Base.label",
        ),
        (
            "abstract class Repo {\n  int size();\n}\n\nclass Base {\n  int size() => 0;\n\n  int legacy() => 1;\n}\n\nclass RepoImpl extends Base implements Repo {}\n",
            "  print(RepoImpl().size());\n",
            "Base.legacy",
            "Base.size",
        ),
        (
            "abstract class Iface {\n  void foo();\n}\n\nmixin class Mixed {\n  void foo() {}\n\n  void spare() {}\n}\n\nclass P with Mixed implements Iface {}\n",
            "  print(P());\n",
            "Mixed.spare",
            "Mixed.foo",
        ),
        (
            "abstract class Iface {\n  void foo();\n}\n\nclass Base {}\n\nmixin class Mixed {\n  void foo() {}\n\n  void spare() {}\n}\n\nclass P extends Base with Mixed implements Iface {}\n",
            "  print(P());\n",
            "Mixed.spare",
            "Mixed.foo",
        ),
    ];
    for (items, main_body, reported, kept) in cases {
        let fixture = tempfile::tempdir()?;
        write_app_with_class_hierarchy(&fixture, items, main_body)?;

        let (_, json) = strict_check(&fixture)?;
        let messages = member_messages(&json);

        assert!(
            messages.iter().any(|message| message.contains(reported)),
            "{reported}: {messages:?}"
        );
        assert!(
            !messages.iter().any(|message| message.contains(kept)),
            "{kept}: {messages:?}"
        );
    }

    Ok(())
}

#[test]
fn strict_check_protects_a_mixin_member_only_for_an_abstract_declaration()
-> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (
            "mixin class A {\n  void x() {}\n\n  void ya() {}\n}\n\nmixin class B {\n  void x() {}\n\n  void yb() {}\n}\n\nclass P with A, B {}\n",
            vec!["A.x", "A.ya", "B.x", "B.yb"],
        ),
        (
            "abstract mixin class A {\n  void x();\n}\n\nmixin class B {\n  void x() {}\n\n  void y() {}\n}\n\nclass P with B, A {}\n",
            vec!["A.x", "B.y"],
        ),
    ];
    for (items, expected) in cases {
        let fixture = tempfile::tempdir()?;
        write_app_with_class_hierarchy(&fixture, items, "  print(P());\n")?;

        let (_, json) = strict_check(&fixture)?;
        let mut found = member_messages(&json)
            .iter()
            .filter_map(|message| message.split_whitespace().nth(4).map(str::to_owned))
            .collect::<Vec<_>>();
        found.sort();

        assert_eq!(found, expected, "{items}");
    }

    Ok(())
}

#[test]
fn strict_check_checks_the_members_of_a_mixin() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (
            "mixin M {\n  void used() {}\n\n  void spare() {}\n}\n\nclass P with M {}\n",
            "  P().used();\n",
            vec!["M.spare"],
        ),
        (
            "class Base {}\n\nmixin M on Base {\n  void twice() {}\n}\n\nclass A extends Base with M {}\n",
            "  A().twice();\n",
            vec![],
        ),
        (
            "mixin M {\n  void foo() {}\n\n  void spare() {}\n}\n\nabstract class I {\n  void foo();\n}\n\nclass P with M implements I {}\n",
            "  print(P());\n",
            vec!["I.foo", "M.spare"],
        ),
    ];
    for (items, main_body, expected) in cases {
        let fixture = tempfile::tempdir()?;
        write_app_with_class_hierarchy(&fixture, items, main_body)?;

        let (_, json) = strict_check(&fixture)?;
        let mut found = member_messages(&json)
            .iter()
            .filter_map(|message| message.split_whitespace().nth(4).map(str::to_owned))
            .collect::<Vec<_>>();
        found.sort();

        assert_eq!(found, expected, "{items}");
    }

    Ok(())
}

#[test]
fn strict_check_keeps_a_mixin_member_that_the_superclass_chain_requires()
-> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (
            "abstract class Base {\n  void hook();\n}\n\nmixin M {\n  void hook() {}\n}\n\nclass P extends Base with M {}\n",
            "  print(P());\n",
            "M.hook",
        ),
        (
            "abstract class Base {\n  void hook();\n}\n\nmixin class M {\n  void hook() {}\n}\n\nclass P extends Base with M {}\n",
            "  print(P());\n",
            "M.hook",
        ),
        (
            "import 'dart:async';\n\nmixin M {\n  Stream<int> bind(Stream<int> s) => s;\n}\n\nclass P extends StreamTransformerBase<int, int> with M {}\n",
            "  Stream.value(1).transform(P());\n",
            "M.bind",
        ),
        (
            "import 'dart:async';\n\nmixin M {\n  Stream<int> bind(Stream<int> s) => s;\n}\n\nabstract class Mid extends StreamTransformerBase<int, int> {}\n\nclass P extends Mid with M {}\n",
            "  Stream.value(1).transform(P());\n",
            "M.bind",
        ),
        (
            "abstract class I {\n  void hook();\n}\n\nabstract class Base implements I {}\n\nmixin M {\n  void hook() {}\n}\n\nclass P extends Base with M {}\n",
            "  print(P());\n",
            "M.hook",
        ),
    ];
    for (items, main_body, kept) in cases {
        let fixture = tempfile::tempdir()?;
        write_app_with_class_hierarchy(&fixture, items, main_body)?;

        let (_, json) = strict_check(&fixture)?;
        let messages = member_messages(&json);

        assert!(
            !messages.iter().any(|message| message.contains(kept)),
            "{items}: {messages:?}"
        );
    }

    for superclass in ["Object", "core.Object"] {
        let fixture = tempfile::tempdir()?;
        write_app_with_class_hierarchy(
            &fixture,
            &format!(
                "import 'dart:core' as core;\n\nmixin M {{\n  void used() {{}}\n\n  void spare() {{}}\n}}\n\nclass P extends {superclass} with M {{}}\n"
            ),
            "  P().used();\n",
        )?;

        let (_, json) = strict_check(&fixture)?;
        let messages = member_messages(&json);

        assert!(
            messages.iter().any(|message| message.contains("M.spare")),
            "{superclass}: {messages:?}"
        );
    }

    Ok(())
}

#[test]
fn strict_check_checks_the_members_of_classes_declared_in_entry_files()
-> Result<(), Box<dyn std::error::Error>> {
    let app = "name: app\npublish_to: none\n";
    let package = "name: pkg\nversion: 1.0.0\nenvironment:\n  sdk: ^3.8.0\n";
    let plain = "class Plain {\n  void used() {}\n\n  void spare() {}\n}\n\nvoid main() {\n  Plain().used();\n}\n";
    let widget = "class MyApp {\n  void build() {}\n}\n\nvoid main() {\n  print(MyApp());\n}\n";
    let cases = [
        (app, "lib/main.dart", plain, vec!["Plain.spare"]),
        (app, "lib/main.dart", widget, vec![]),
        (package, "lib/src/runner.dart", plain, vec![]),
    ];
    for (pubspec, path, source, expected) in cases {
        let fixture = tempfile::tempdir()?;
        write(&fixture, "pubspec.yaml", pubspec)?;
        write(&fixture, path, source)?;

        let (_, json) = strict_check_with(&fixture, &["--entry", path])?;
        let found = member_messages(&json)
            .iter()
            .filter_map(|message| message.split_whitespace().nth(4).map(str::to_owned))
            .collect::<Vec<_>>();

        assert_eq!(found, expected, "{path}: {source}");
    }

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

const FRAMEWORK: &str = "abstract class BuildContext {}\n\nabstract class Widget {}\n\nabstract class StatefulWidget extends Widget {\n  State createState();\n}\n\nabstract class StatelessWidget extends Widget {\n  Widget build(BuildContext context);\n}\n\nabstract class State<T extends StatefulWidget> {\n  void initState() {}\n\n  void dispose() {}\n\n  Widget build(BuildContext context);\n}\n";

fn write_toolchain(
    toolchain: &TempDir,
    sdk_version: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let flutter = toolchain.path().join("flutter");
    let sky = flutter.join("bin/cache/pkg/sky_engine/lib");
    let files = [
        (flutter.join("bin/cache/dart-sdk/version"), sdk_version),
        (
            flutter.join("bin/cache/dart-sdk/lib/libraries.json"),
            "{\"vm_common\":{\"libraries\":{}},\"vm\":{\"libraries\":{}}}",
        ),
        (
            sky.join("_embedder.yaml"),
            "embedded_libs:\n  \"dart:core\": \"core/core.dart\"\n  \"dart:io\": \"io/io.dart\"\n",
        ),
        (
            sky.join("io/io.dart"),
            "library dart.io;\n\npart \"file.dart\";\npart \"embedder_config.dart\";\n\nclass File {}\n",
        ),
        (
            sky.join("io/file.dart"),
            "part of dart.io;\n\nclass Stdin {}\n",
        ),
        (
            sky.join("core/core.dart"),
            "library dart.core;\n\npart \"comparable.dart\";\n\nclass Object {}\n",
        ),
        (
            sky.join("core/comparable.dart"),
            "part of dart.core;\n\nabstract interface class Comparable<T> {\n  int compareTo(T other);\n}\n",
        ),
        (
            flutter.join("packages/flutter/lib/widgets.dart"),
            "export 'src/framework.dart';\n",
        ),
        (
            flutter.join("packages/flutter/lib/src/framework.dart"),
            FRAMEWORK,
        ),
    ];
    for (path, source) in files {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, source)?;
    }
    Ok(format!(
        "{{\"name\":\"flutter\",\"rootUri\":\"file://{flutter}/packages/flutter\",\"packageUri\":\"lib/\"}},{{\"name\":\"sky_engine\",\"rootUri\":\"file://{flutter}/bin/cache/pkg/sky_engine\",\"packageUri\":\"lib/\"}}",
        flutter = flutter.display()
    ))
}

fn write_package_config(
    fixture: &TempDir,
    toolchain: &TempDir,
    packages: &str,
    extra: &[(&str, &Path)],
) -> Result<(), std::io::Error> {
    let extra = extra
        .iter()
        .map(|(name, root)| {
            format!(
                ",{{\"name\":\"{name}\",\"rootUri\":\"file://{}\",\"packageUri\":\"lib/\"}}",
                root.display()
            )
        })
        .collect::<Vec<_>>()
        .concat();
    write(
        fixture,
        ".dart_tool/package_config.json",
        &format!(
            "{{\"configVersion\":2,\"generatorVersion\":\"3.14.0\",\"flutterRoot\":\"file://{}/flutter\",\"packages\":[{packages}{extra},{{\"name\":\"app\",\"rootUri\":\"../\",\"packageUri\":\"lib/\"}}]}}",
            toolchain.path().display()
        ),
    )
}

fn found_members(json: &Value) -> Vec<String> {
    json["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|finding| {
            matches!(
                finding["rule_id"].as_str(),
                Some("dart-decimate/unused-class-member" | "dart-decimate/unused-enum-member")
            )
        })
        .filter_map(|finding| finding["message"].as_str())
        .filter_map(|message| message.split_whitespace().find(|word| word.contains('.')))
        .map(str::to_owned)
        .collect()
}

fn strict_check_with_toolchain(
    items: &str,
    main: &str,
) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let toolchain = tempfile::tempdir()?;
    let packages = write_toolchain(&toolchain, "3.14.0")?;
    write(&fixture, "pubspec.yaml", "name: app\npublish_to: none\n")?;
    write(&fixture, "lib/main.dart", main)?;
    write(&fixture, "lib/src/items.dart", items)?;
    write_package_config(&fixture, &toolchain, &packages, &[])?;
    strict_check(&fixture)
}

#[test]
fn outside_superclass_mixin_reports_names_the_outside_type_lacks()
-> Result<(), Box<dyn std::error::Error>> {
    let (_, json) = strict_check_with_toolchain(
        "mixin Tracker {\n  void initState() {}\n\n  void spare() {}\n}\n",
        "import 'package:flutter/widgets.dart';\nimport 'src/items.dart';\n\nclass Page extends State<StatefulWidget> with Tracker {}\n\nvoid main() {\n  print(Page());\n}\n",
    )?;

    assert_eq!(found_members(&json), ["Tracker.spare"]);

    Ok(())
}

#[test]
fn outside_hook_with_body_stays_protected() -> Result<(), Box<dyn std::error::Error>> {
    let (_, json) = strict_check_with_toolchain(
        "mixin Tracker {\n  void dispose() {}\n\n  void spare() {}\n}\n",
        "import 'package:flutter/widgets.dart';\nimport 'src/items.dart';\n\nclass Page extends State<StatefulWidget> with Tracker {}\n\nvoid main() {\n  print(Page());\n}\n",
    )?;

    assert_eq!(found_members(&json), ["Tracker.spare"]);

    Ok(())
}

#[test]
fn outside_interface_base_reports_names_the_interface_lacks()
-> Result<(), Box<dyn std::error::Error>> {
    let (_, json) = strict_check_with_toolchain(
        "import 'dart:io';\n\nclass Base {\n  int compareTo(Object other) => 0;\n\n  int spare() => 1;\n}\n\nclass Item extends Base implements Comparable<Item> {}\n",
        "import 'src/items.dart';\n\nvoid main() {\n  print(Item());\n}\n",
    )?;

    assert_eq!(found_members(&json), ["Base.spare"]);

    Ok(())
}

#[test]
fn outside_supertype_chain_protects_inherited_names() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let toolchain = tempfile::tempdir()?;
    let packages = write_toolchain(&toolchain, "3.14.0")?;
    let pkg = toolchain.path().join("pkg");
    let files = [
        (
            "lib/pkg.dart",
            "export 'src/alpha.dart' show Alpha, Entry;\nexport 'src/zed.dart' hide Entry;\n",
        ),
        (
            "lib/src/alpha.dart",
            "import 'beta.dart';\n\ntypedef Entry = Alpha;\n\nabstract class Alpha implements Beta {\n  void alphaHook();\n}\n",
        ),
        (
            "lib/src/beta.dart",
            "import 'gamma.dart' as g;\n\nabstract class Beta extends g.Gamma {\n  void betaHook() {}\n}\n",
        ),
        (
            "lib/src/gamma.dart",
            "class Gamma {\n  void gammaHook() {}\n}\n",
        ),
        (
            "lib/src/zed.dart",
            "class Entry {\n  void zedHook() {}\n}\n",
        ),
    ];
    for (path, source) in files {
        let path = pkg.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, source)?;
    }
    write(&fixture, "pubspec.yaml", "name: app\npublish_to: none\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/items.dart';\n\nvoid main() {\n  print(Item());\n}\n",
    )?;
    write(
        &fixture,
        "lib/src/items.dart",
        "import 'package:pkg/pkg.dart' as p;\n\nclass Base {\n  void alphaHook() {}\n\n  void betaHook() {}\n\n  void gammaHook() {}\n\n  void zedHook() {}\n}\n\nclass Item extends Base implements p.Entry {}\n",
    )?;
    write_package_config(&fixture, &toolchain, &packages, &[("pkg", &pkg)])?;

    let (_, json) = strict_check(&fixture)?;

    assert_eq!(found_members(&json), ["Base.zedHook"]);

    Ok(())
}

#[test]
fn unread_outside_types_keep_whole_class_skip() -> Result<(), Box<dyn std::error::Error>> {
    let items = "import 'package:pkg/pkg.dart';\n\nclass Base {\n  int compareTo(Object other) => 0;\n\n  int spare() => 1;\n}\n\nclass Item extends Base implements Ambiguous, Remote {}\n";
    let main = "import 'src/items.dart';\n\nvoid main() {\n  print(Item());\n}\n";
    let sources = [
        (
            "lib/pkg.dart",
            "export 'src/remote.dart';\nexport 'src/other.dart';\n",
        ),
        (
            "lib/src/remote.dart",
            "abstract class Remote {\n  int compareTo(Object other);\n}\n\nabstract class Ambiguous {}\n",
        ),
        ("lib/src/other.dart", "abstract class Ambiguous {}\n"),
    ];
    let cases = [
        "no config",
        "missing folder",
        "parse error",
        "other version",
        "ambiguous name",
        "missing part",
    ];
    for case in cases {
        let fixture = tempfile::tempdir()?;
        let toolchain = tempfile::tempdir()?;
        let sdk_version = if case == "other version" {
            "3.13.0"
        } else {
            "3.14.0"
        };
        let packages = write_toolchain(&toolchain, sdk_version)?;
        let pkg = toolchain.path().join("pkg");
        for (path, source) in sources {
            let source = if case == "parse error" && path.ends_with("remote.dart") {
                "abstract class Remote {{{\n"
            } else if case == "missing part" && path.ends_with("remote.dart") {
                "part 'gone.dart';\n"
            } else if case == "ambiguous name" || !path.ends_with("other.dart") {
                source
            } else {
                "class Spare {}\n"
            };
            if case != "missing folder" {
                let path = pkg.join(path);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(path, source)?;
            }
        }
        write(
            &fixture,
            "pubspec.yaml",
            "name: app\npublish_to: none\ndependencies:\n  pkg: any\n",
        )?;
        write(&fixture, "lib/main.dart", main)?;
        write(&fixture, "lib/src/items.dart", items)?;
        if case != "no config" {
            write_package_config(&fixture, &toolchain, &packages, &[("pkg", &pkg)])?;
        }

        let (code, json) = strict_check(&fixture)?;

        assert_eq!(found_members(&json), Vec::<String>::new(), "{case}");
        assert_eq!(code, 0, "{case}");
        assert!(
            json["summary"]["outside_types_unread"].as_u64() > Some(0),
            "{case}: {}",
            json["summary"]
        );
        let commands = json["next_steps"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|step| step["id"] == "read-outside-types")
            .filter_map(|step| step["command"].as_str())
            .collect::<Vec<_>>();
        assert_eq!(commands, ["dart pub get"], "{case}");
    }

    Ok(())
}

#[test]
fn extension_on_types_are_not_unread_outside_types() -> Result<(), Box<dyn std::error::Error>> {
    let (code, json) = strict_check_with_toolchain(
        "extension Doubled<T extends num> on T {\n  T twice() => this;\n}\n\nextension Pairs on (int, int) {\n  int sum() => 1;\n}\n\nextension Names<T> on List<T> {\n  int count() => 1;\n}\n",
        "import 'src/items.dart';\n\nvoid main() {\n  print(1.twice());\n  print((1, 2).sum());\n  print([1].count());\n}\n",
    )?;

    assert_eq!(code, 0);
    assert_eq!(
        json["summary"]["outside_types_unread"], 0,
        "{}",
        json["summary"]
    );
    assert!(
        json["next_steps"]
            .as_array()
            .into_iter()
            .flatten()
            .all(|step| step["id"] != "read-outside-types")
    );

    Ok(())
}

#[test]
fn project_barrel_exports_of_outside_libraries_are_read() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    let toolchain = tempfile::tempdir()?;
    let packages = write_toolchain(&toolchain, "3.14.0")?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\npublish_to: none\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )?;
    write(
        &fixture,
        "lib/barrel.dart",
        "export 'package:flutter/widgets.dart';\nexport 'dart:io';\n",
    )?;
    write(
        &fixture,
        "lib/src/items.dart",
        "import '../barrel.dart';\n\nclass Item extends StatelessWidget {\n  Widget build(BuildContext context) => this;\n\n  int spare() => 1;\n}\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/items.dart';\n\nvoid main() {\n  print(Item());\n}\n",
    )?;
    write_package_config(&fixture, &toolchain, &packages, &[])?;

    let (code, json) = strict_check(&fixture)?;

    assert_eq!(
        json["summary"]["outside_types_unread"], 0,
        "{}",
        json["summary"]
    );
    assert_eq!(found_members(&json), ["Item.spare"]);
    assert_eq!(code, 1);

    Ok(())
}

#[test]
fn project_barrel_export_combinators_hide_outside_names() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    let toolchain = tempfile::tempdir()?;
    let packages = write_toolchain(&toolchain, "3.14.0")?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\npublish_to: none\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )?;
    write(
        &fixture,
        "lib/barrel.dart",
        "export 'package:flutter/widgets.dart' hide StatelessWidget;\n",
    )?;
    write(
        &fixture,
        "lib/src/items.dart",
        "import '../barrel.dart';\n\nclass Item extends StatelessWidget {\n  Widget build(BuildContext context) => this;\n\n  int spare() => 1;\n}\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/items.dart';\n\nvoid main() {\n  print(Item());\n}\n",
    )?;
    write_package_config(&fixture, &toolchain, &packages, &[])?;

    let (code, json) = strict_check(&fixture)?;

    assert_eq!(
        json["summary"]["outside_types_unread"], 1,
        "{}",
        json["summary"]
    );
    assert_eq!(found_members(&json), Vec::<String>::new());
    assert_eq!(code, 0);

    Ok(())
}

#[test]
fn annotated_extension_on_types_are_not_unread_outside_types()
-> Result<(), Box<dyn std::error::Error>> {
    let (code, json) = strict_check_with_toolchain(
        "@visibleForTesting\nextension RX on (int, int) {\n  int get sum => $1 + $2;\n}\n\n@visibleForTesting\nextension Call on void Function() {\n  void twice() {}\n}\n",
        "import 'src/items.dart';\n\nvoid main() {\n  print((1, 2).sum);\n  (() {}).twice();\n}\n",
    )?;

    assert_eq!(code, 0);
    assert_eq!(json["summary"]["outside_types_unread"], 0);
    assert!(
        json["next_steps"]
            .as_array()
            .into_iter()
            .flatten()
            .all(|step| step["id"] != "read-outside-types")
    );

    Ok(())
}

#[test]
fn monorepo_root_reads_each_packages_own_config() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let toolchain = tempfile::tempdir()?;
    let packages = write_toolchain(&toolchain, "3.14.0")?;
    write(
        &fixture,
        "apps/app/pubspec.yaml",
        "name: app\npublish_to: none\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )?;
    write(
        &fixture,
        "apps/app/lib/main.dart",
        "import 'package:flutter/widgets.dart';\n\nclass Item extends StatelessWidget {\n  Widget build(BuildContext context) => this;\n\n  int spare() => 1;\n}\n\nvoid main() {\n  print(Item());\n}\n",
    )?;
    write(
        &fixture,
        "apps/app/.dart_tool/package_config.json",
        &format!(
            "{{\"configVersion\":2,\"generatorVersion\":\"3.14.0\",\"flutterRoot\":\"file://{}/flutter\",\"packages\":[{packages},{{\"name\":\"app\",\"rootUri\":\"../\",\"packageUri\":\"lib/\"}}]}}",
            toolchain.path().display()
        ),
    )?;
    write(
        &fixture,
        "apps/pure app/pubspec.yaml",
        "name: pure\npublish_to: none\n",
    )?;
    write(
        &fixture,
        "apps/pure app/lib/main.dart",
        "class Other extends Missing {\n  int used() => 1;\n}\n\nvoid main() {\n  print(Other().used());\n}\n",
    )?;

    let (_, json) = strict_check(&fixture)?;

    assert_eq!(found_members(&json), ["Item.spare"]);
    assert_eq!(json["summary"]["outside_types_unread"], 1);
    let commands = json["next_steps"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|step| step["id"] == "read-outside-types")
        .filter_map(|step| step["command"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(commands, ["cd 'apps/pure app' && dart pub get"]);

    Ok(())
}

#[test]
fn unlisted_packages_under_one_config_give_the_same_result_in_any_order()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let toolchain = tempfile::tempdir()?;
    let packages = write_toolchain(&toolchain, "3.14.0")?;
    let pubspec = "publish_to: none\ndependencies:\n  flutter:\n    sdk: flutter\n";
    write(&fixture, "pubspec.yaml", &format!("name: app\n{pubspec}"))?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    for name in ["aaa", "zzz"] {
        let class = format!("{}Item", name.to_uppercase());
        write(
            &fixture,
            &format!("{name}/pubspec.yaml"),
            &format!("name: {name}\n{pubspec}"),
        )?;
        write(
            &fixture,
            &format!("{name}/lib/main.dart"),
            &format!(
                "import 'package:flutter/widgets.dart';\n\nclass {class} extends StatelessWidget {{\n  Widget build(BuildContext context) => this;\n\n  int spare{name}() => 1;\n}}\n\nvoid main() {{\n  print({class}());\n}}\n"
            ),
        )?;
    }
    write_package_config(&fixture, &toolchain, &packages, &[])?;

    let (_, json) = strict_check(&fixture)?;

    assert_eq!(
        found_members(&json),
        ["AAAItem.spareaaa", "ZZZItem.sparezzz"]
    );
    assert_eq!(json["summary"]["outside_types_unread"], 0);

    Ok(())
}

#[test]
fn unlisted_package_resolves_its_own_name_and_path_dependencies()
-> Result<(), Box<dyn std::error::Error>> {
    for (name, with_path_dependency) in [("aaa", false), ("zzz", false), ("aaa", true)] {
        let fixture = tempfile::tempdir()?;
        let toolchain = tempfile::tempdir()?;
        let packages = write_toolchain(&toolchain, "3.14.0")?;
        write(&fixture, "pubspec.yaml", "name: app\npublish_to: none\n")?;
        write(&fixture, "lib/main.dart", "void main() {}\n")?;
        let (dependency, import) = if with_path_dependency {
            write(
                &fixture,
                "bbb/pubspec.yaml",
                "name: bbb\npublish_to: none\n",
            )?;
            write(&fixture, "bbb/lib/b.dart", "class Bee {}\n")?;
            (
                "dependencies:\n  bbb:\n    path: ../bbb\n",
                "package:bbb/b.dart".to_owned(),
            )
        } else {
            ("", format!("package:{name}/other.dart"))
        };
        write(
            &fixture,
            &format!("{name}/pubspec.yaml"),
            &format!("name: {name}\npublish_to: none\n{dependency}"),
        )?;
        write(
            &fixture,
            &format!("{name}/lib/other.dart"),
            "class Other {}\n",
        )?;
        write(
            &fixture,
            &format!("{name}/lib/main.dart"),
            &format!(
                "import '{import}';\nimport 'other.dart';\n\nclass AItem implements Comparable<AItem> {{\n  int compareTo(AItem other) => 0;\n\n  int spare() => 1;\n}}\n\nvoid main() {{\n  print(AItem());\n  print(Other());\n}}\n"
            ),
        )?;
        write_package_config(&fixture, &toolchain, &packages, &[])?;

        let (_, json) = strict_check(&fixture)?;

        assert_eq!(
            found_members(&json),
            ["AItem.spare"],
            "{name} {with_path_dependency}"
        );
        assert_eq!(
            json["summary"]["outside_types_unread"], 0,
            "{name} {with_path_dependency}"
        );
    }

    Ok(())
}

#[test]
fn supertypes_declared_in_ignored_project_files_are_read_from_those_files()
-> Result<(), Box<dyn std::error::Error>> {
    let supertype = "abstract class Comparable<T> {\n  int used();\n}\n";
    let item = |import: &str| {
        format!(
            "import '{import}';\n\nclass AItem implements Comparable<AItem> {{\n  int compareTo(AItem other) => 0;\n\n  int used() => 1;\n\n  int spare() => 1;\n}}\n\nvoid main() {{\n  print(AItem());\n}}\n"
        )
    };
    let cases = [
        ("unlisted package, ignored own file", "aaa", None),
        (
            "unlisted package, ignored path dependency",
            "aaa",
            Some("bbb"),
        ),
        ("listed app, ignored path dependency", "", Some("bbb")),
    ];
    for (case, folder, dependency) in cases {
        let fixture = tempfile::tempdir()?;
        let toolchain = tempfile::tempdir()?;
        let packages = write_toolchain(&toolchain, "3.14.0")?;
        write(
            &fixture,
            ".dart-decimaterc",
            "{\"ignorePatterns\": [\"**/generated/**\"]}\n",
        )?;
        let prefix = if folder.is_empty() {
            String::new()
        } else {
            format!("{folder}/")
        };
        let owner = dependency.unwrap_or(if folder.is_empty() { "app" } else { folder });
        let import = format!("package:{owner}/generated/cmp.dart");
        let dependencies = dependency.map_or(String::new(), |name| {
            format!("dependencies:\n  {name}:\n    path: ../{name}\n")
        });
        if folder.is_empty() {
            write(
                &fixture,
                "pubspec.yaml",
                "name: app\npublish_to: none\ndependencies:\n  bbb:\n    path: bbb\n",
            )?;
        } else {
            write(&fixture, "pubspec.yaml", "name: app\npublish_to: none\n")?;
            write(&fixture, "lib/main.dart", "void main() {}\n")?;
            write(
                &fixture,
                &format!("{prefix}pubspec.yaml"),
                &format!("name: {folder}\npublish_to: none\n{dependencies}"),
            )?;
        }
        if let Some(name) = dependency {
            write(
                &fixture,
                &format!("{name}/pubspec.yaml"),
                &format!("name: {name}\npublish_to: none\n"),
            )?;
            write(
                &fixture,
                &format!("{name}/lib/generated/cmp.dart"),
                supertype,
            )?;
        } else {
            write(
                &fixture,
                &format!("{prefix}lib/generated/cmp.dart"),
                supertype,
            )?;
        }
        write(&fixture, &format!("{prefix}lib/main.dart"), &item(&import))?;
        write_package_config(&fixture, &toolchain, &packages, &[])?;

        let (_, json) = strict_check(&fixture)?;

        assert_eq!(
            found_members(&json),
            ["AItem.compareTo", "AItem.spare"],
            "{case}"
        );
        assert_eq!(json["summary"]["outside_types_unread"], 0, "{case}");
    }

    Ok(())
}

#[test]
fn project_classes_reached_through_ignored_files_keep_their_own_hierarchy()
-> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (
            "class Model implements Comparable<Model> {\n  int compareTo(Model other) => 0;\n\n  int modelSpare() => 1;\n}\n",
            "Model.compareTo",
        ),
        (
            "abstract class Base {\n  int hook();\n}\n\nclass Model extends Base {\n  int hook() => 0;\n\n  int modelSpare() => 1;\n}\n",
            "Model.hook",
        ),
    ];
    for (model, protected) in cases {
        let fixture = tempfile::tempdir()?;
        let toolchain = tempfile::tempdir()?;
        let packages = write_toolchain(&toolchain, "3.14.0")?;
        write(
            &fixture,
            ".dart-decimaterc",
            "{\"ignorePatterns\": [\"**/generated/**\"]}\n",
        )?;
        write(&fixture, "pubspec.yaml", "name: app\npublish_to: none\n")?;
        write(&fixture, "lib/model.dart", model)?;
        write(
            &fixture,
            "lib/generated/gen.dart",
            "import '../model.dart';\n\nclass GenModel extends Model {}\n",
        )?;
        write(
            &fixture,
            "lib/main.dart",
            "import 'generated/gen.dart';\nimport 'model.dart';\n\nclass Impl extends GenModel {\n  int spare() => 1;\n}\n\nvoid main() {\n  print(Impl());\n  print(Model());\n}\n",
        )?;
        write_package_config(&fixture, &toolchain, &packages, &[])?;

        let (_, json) = strict_check(&fixture)?;

        let found = found_members(&json);
        assert!(found.contains(&"Model.modelSpare".to_owned()), "{found:?}");
        assert!(!found.contains(&protected.to_owned()), "{found:?}");
    }

    Ok(())
}

#[test]
fn missing_local_imports_name_the_file_in_the_next_step() -> Result<(), Box<dyn std::error::Error>>
{
    let cases = [
        ("missing.g.dart", "dart run build_runner build"),
        (
            "missing.dart",
            "create or fix the import of lib/missing.dart",
        ),
    ];
    for (file, command) in cases {
        let fixture = tempfile::tempdir()?;
        let toolchain = tempfile::tempdir()?;
        let packages = write_toolchain(&toolchain, "3.14.0")?;
        write(&fixture, "pubspec.yaml", "name: app\npublish_to: none\n")?;
        write(
            &fixture,
            "lib/main.dart",
            &format!(
                "import '{file}';\n\nclass Item implements Comparable<Item> {{\n  int compareTo(Item other) => 0;\n\n  int spare() => 1;\n}}\n\nvoid main() {{\n  print(Item());\n}}\n"
            ),
        )?;
        write_package_config(&fixture, &toolchain, &packages, &[])?;

        let (_, json) = strict_check(&fixture)?;

        assert!(
            json["summary"]["outside_types_unread"].as_u64() > Some(0),
            "{file}"
        );
        let steps = json["next_steps"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|step| step["command"].as_str().zip(step["reason"].as_str()))
            .collect::<Vec<_>>();
        assert_eq!(steps.len(), 1, "{file}: {steps:?}");
        assert_eq!(steps[0].0, command, "{file}");
        assert!(
            steps[0].1.contains(&format!("lib/{file}")),
            "{file}: {steps:?}"
        );
    }

    Ok(())
}

fn next_step_commands(json: &Value) -> Vec<String> {
    let mut commands = json["next_steps"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|step| {
            matches!(
                step["id"].as_str(),
                Some("read-outside-types" | "generate-missing-import" | "fix-missing-import")
            )
        })
        .filter_map(|step| step["command"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    commands.sort();
    commands
}

fn missing_import_fixture(
    files: &[(&str, &str)],
) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let toolchain = tempfile::tempdir()?;
    let packages = write_toolchain(&toolchain, "3.14.0")?;
    write(&fixture, "pubspec.yaml", "name: app\npublish_to: none\n")?;
    write(
        &fixture,
        ".dart-decimaterc",
        "{\"ignorePatterns\": [\"**/generated/**\"]}\n",
    )?;
    for (path, source) in files {
        write(&fixture, path, source)?;
    }
    write_package_config(&fixture, &toolchain, &packages, &[])?;
    strict_check(&fixture)
}

#[test]
fn missing_generated_part_files_name_the_generator_step() -> Result<(), Box<dyn std::error::Error>>
{
    let cases = [
        "part 'main.freezed.dart';\n\nclass Item with _$Item {\n  int spare() => 1;\n}\n\nvoid main() {\n  print(Item());\n}\n",
        "part 'main.g.dart';\n\nclass Item extends GenBase {\n  int spare() => 1;\n}\n\nvoid main() {\n  print(Item());\n}\n",
    ];
    for main in cases {
        let (_, json) = missing_import_fixture(&[("lib/main.dart", main)])?;

        assert!(
            json["summary"]["outside_types_unread"].as_u64() > Some(0),
            "{main}"
        );
        assert_eq!(
            next_step_commands(&json),
            ["dart run build_runner build"],
            "{main}"
        );
    }

    Ok(())
}

#[test]
fn missing_imports_of_followed_ignored_files_name_the_generator_step()
-> Result<(), Box<dyn std::error::Error>> {
    let (_, json) = missing_import_fixture(&[
        (
            "lib/generated/gen.dart",
            "import 'gen.g.dart';\n\nclass GenModel extends GenBase {}\n",
        ),
        (
            "lib/main.dart",
            "import 'generated/gen.dart';\n\nclass Impl extends GenModel {\n  int spare() => 1;\n}\n\nvoid main() {\n  print(Impl());\n}\n",
        ),
    ])?;

    assert!(json["summary"]["outside_types_unread"].as_u64() > Some(0));
    assert_eq!(next_step_commands(&json), ["dart run build_runner build"]);

    Ok(())
}

#[test]
fn missing_files_and_unlisted_packages_keep_both_steps() -> Result<(), Box<dyn std::error::Error>> {
    let (_, json) = missing_import_fixture(&[
        (
            "lib/a.dart",
            "import 'missing.g.dart';\n\nclass A implements Comparable<A> {\n  int compareTo(A other) => 0;\n\n  int spare() => 1;\n}\n",
        ),
        (
            "lib/main.dart",
            "import 'a.dart';\nimport 'package:nope/x.dart';\n\nclass Item implements Thing {\n  int spare() => 1;\n}\n\nvoid main() {\n  print(A());\n  print(Item());\n}\n",
        ),
    ])?;

    assert_eq!(json["summary"]["outside_types_unread"], 2);
    assert_eq!(
        next_step_commands(&json),
        ["dart pub get", "dart run build_runner build"]
    );

    Ok(())
}

fn step_reasons(json: &Value, id: &str) -> Vec<String> {
    json["next_steps"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|step| step["id"] == id)
        .filter_map(|step| step["reason"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn missing_parts_give_one_step_for_each_package_that_can_use_them()
-> Result<(), Box<dyn std::error::Error>> {
    let (_, json) = missing_import_fixture(&[
        (
            "lib/main.dart",
            "part 'a.g.dart';\npart 'b.g.dart';\npart 'c.g.dart';\n\nclass Item extends GenBase {\n  int spare() => 1;\n}\n\nvoid main() {\n  print(Item());\n}\n",
        ),
        (
            "test/w.dart",
            "part 'w.g.dart';\n\nclass Plain {\n  int spare() => 1;\n}\n\nvoid main() {\n  print(Plain());\n}\n",
        ),
    ])?;

    assert_eq!(next_step_commands(&json), ["dart run build_runner build"]);
    let reasons = step_reasons(&json, "generate-missing-import");
    assert_eq!(reasons.len(), 1, "{reasons:?}");
    for file in ["a.g.dart", "b.g.dart", "c.g.dart"] {
        assert!(reasons[0].contains(file), "{reasons:?}");
    }
    assert!(!reasons[0].contains("w.g.dart"), "{reasons:?}");
    assert!(reasons[0].contains("is a part but missing"), "{reasons:?}");
    assert!(reasons[0].contains("generate them"), "{reasons:?}");
    assert!(!reasons[0].contains("imported but missing"), "{reasons:?}");

    Ok(())
}

#[test]
fn missing_files_of_a_local_path_package_get_the_step_in_that_package()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let toolchain = tempfile::tempdir()?;
    let packages = write_toolchain(&toolchain, "3.14.0")?;
    let flutter = toolchain.path().join("flutter");
    let config = |extra: &str, name: &str| {
        format!(
            "{{\"configVersion\":2,\"generatorVersion\":\"3.14.0\",\"flutterRoot\":\"file://{}\",\"packages\":[{packages},{extra}{{\"name\":\"{name}\",\"rootUri\":\"../\",\"packageUri\":\"lib/\"}}]}}",
            flutter.display()
        )
    };
    write(
        &fixture,
        "app/pubspec.yaml",
        "name: app\npublish_to: none\ndependencies:\n  core:\n    path: ../core\n",
    )?;
    write(
        &fixture,
        "app/lib/main.dart",
        "import 'package:core/x.g.dart';\n\nclass Impl extends GenBase {\n  int spare() => 1;\n}\n\nvoid main() {\n  print(Impl());\n}\n",
    )?;
    write(
        &fixture,
        "app/.dart_tool/package_config.json",
        &config(
            "{\"name\":\"core\",\"rootUri\":\"../../core\",\"packageUri\":\"lib/\"},",
            "app",
        ),
    )?;
    write(
        &fixture,
        "core/pubspec.yaml",
        "name: core\npublish_to: none\n",
    )?;
    write(&fixture, "core/lib/core.dart", "void core() {}\n")?;
    write(
        &fixture,
        "core/.dart_tool/package_config.json",
        &config("", "core"),
    )?;

    let (_, from_parent) = strict_check(&fixture)?;
    assert_eq!(
        next_step_commands(&from_parent),
        [
            "cd app && dart pub get",
            "cd core && dart run build_runner build"
        ]
    );

    let mut output = Vec::new();
    run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().join("app").to_str().unwrap_or("."),
            "--threshold",
            "0",
            "--strict",
            "--format",
            "json",
        ],
        &mut output,
    )?;
    let from_app = serde_json::from_slice::<Value>(&output)?;
    let commands = next_step_commands(&from_app);
    assert!(
        commands
            .iter()
            .any(|command| command.contains("core")
                && command.ends_with("dart run build_runner build")),
        "{commands:?}"
    );

    Ok(())
}

type Row<'a> = (
    &'a str,
    &'a [(&'a str, &'a str)],
    &'a str,
    Vec<&'a str>,
    &'a str,
);

#[test]
fn missing_local_file_steps_follow_one_rule() -> Result<(), Box<dyn std::error::Error>> {
    let base_case: &[(&str, &str)] = &[
        ("lib/base.dart", "part 'base.g.dart';\n"),
        (
            "lib/main.dart",
            "import 'base.dart';\n\nclass Impl extends GenBase {\n  int spare() => 1;\n}\n\nvoid main() {\n  print(Impl());\n}\n",
        ),
    ];
    let unrelated: &[(&str, &str)] = &[
        (
            "lib/main.dart",
            "class Impl extends Missing {\n  int spare() => 1;\n}\n\nvoid main() {\n  print(Impl());\n}\n",
        ),
        (
            "test/w.dart",
            "part 'w.g.dart';\n\nclass Plain {}\n\nvoid main() {\n  print(Plain());\n}\n",
        ),
    ];
    let workspace: &[(&str, &str)] = &[
        (
            "pubspec.yaml",
            "name: app\npublish_to: none\nenvironment:\n  sdk: ^3.6.0\nworkspace:\n  - packages/a\n",
        ),
        (
            "packages/a/pubspec.yaml",
            "name: a\npublish_to: none\nresolution: workspace\nenvironment:\n  sdk: ^3.6.0\n",
        ),
        ("packages/a/lib/base.dart", "part 'base.g.dart';\n"),
        (
            "packages/a/lib/main.dart",
            "import 'base.dart';\n\nclass Impl extends GenBase {\n  int spare() => 1;\n}\n\nvoid main() {\n  print(Impl());\n}\n",
        ),
    ];
    // (case, files, folder to scan, commands, reason fragment)
    let rows: [Row; 5] = [
        (
            "part reached through an import",
            base_case,
            "",
            vec!["dart run build_runner build"],
            "lib/base.g.dart is a part but missing; generate it ",
        ),
        (
            "unrelated missing part",
            unrelated,
            "",
            vec!["dart pub get"],
            "",
        ),
        (
            "scan root is the package",
            base_case,
            ".",
            vec!["dart run build_runner build"],
            "lib/base.g.dart",
        ),
        (
            "scan root below the package",
            base_case,
            "lib",
            vec!["dart run build_runner build"],
            "lib/base.g.dart",
        ),
        (
            "pub workspace member",
            workspace,
            "",
            vec!["cd packages/a && dart run build_runner build"],
            "lib/base.g.dart",
        ),
    ];
    for (case, files, scan, commands, reason) in rows {
        let fixture = tempfile::tempdir()?;
        let toolchain = tempfile::tempdir()?;
        let packages = write_toolchain(&toolchain, "3.14.0")?;
        write(&fixture, "pubspec.yaml", "name: app\npublish_to: none\n")?;
        for (path, source) in files {
            write(&fixture, path, source)?;
        }
        write_package_config(&fixture, &toolchain, &packages, &[])?;
        let mut output = Vec::new();
        run_from(
            [
                "dart-decimate",
                "check",
                fixture.path().join(scan).to_str().unwrap_or("."),
                "--threshold",
                "0",
                "--strict",
                "--format",
                "json",
            ],
            &mut output,
        )?;
        let json = serde_json::from_slice::<Value>(&output)?;

        assert_eq!(next_step_commands(&json), commands, "{case}");
        let reasons = step_reasons(&json, "generate-missing-import").join(" | ");
        assert!(reasons.contains(reason), "{case}: {reasons}");
    }

    Ok(())
}

fn scan_json(path: &Path) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(
        [
            "dart-decimate",
            "check",
            path.to_str().unwrap_or("."),
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

#[test]
fn scanning_a_sub_folder_checks_the_package_above_it() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\npublish_to: none\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "class Item {\n  int spare() => 1;\n}\n\nvoid main() {\n  print(Item());\n}\n",
    )?;
    write(
        &fixture,
        "test/other.dart",
        "class Other {\n  int spare() => 1;\n}\n",
    )?;

    let (_, whole) = scan_json(fixture.path())?;
    let (_, sub) = scan_json(&fixture.path().join("lib"))?;

    assert_eq!(found_members(&sub), ["Item.spare"]);
    assert!(found_members(&whole).contains(&"Item.spare".to_owned()));

    let steps = missing_import_fixture(&[
        ("lib/base.dart", "part 'base.g.dart';\n"),
        (
            "lib/main.dart",
            "import 'base.dart';\n\nclass Impl extends GenBase {\n  int spare() => 1;\n}\n\nvoid main() {\n  print(Impl());\n}\n",
        ),
    ])?;
    assert_eq!(
        next_step_commands(&steps.1),
        ["dart run build_runner build"]
    );

    Ok(())
}

#[test]
fn scanning_a_sub_folder_reports_what_a_root_scan_reports_for_it()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\npublish_to: none\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:app/util.dart';\nimport 'package:app/item.dart';\n\nvoid main() {\n  print(Util().used());\n  print(Item());\n}\n",
    )?;
    write(
        &fixture,
        "lib/util.dart",
        "class Util {\n  int used() => 1;\n\n  int spare() => 2;\n}\n",
    )?;
    write(
        &fixture,
        "lib/item.dart",
        "class Item {\n  int testOnly() => 1;\n}\n",
    )?;
    write(
        &fixture,
        "test/item_test.dart",
        "import 'package:app/item.dart';\n\nvoid main() {\n  print(Item().testOnly());\n}\n",
    )?;

    let (_, whole) = scan_json(fixture.path())?;
    let (_, sub) = scan_json(&fixture.path().join("lib"))?;
    let under_lib = |json: &Value| {
        let mut found = json["findings"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|finding| {
                finding["path"]
                    .as_str()
                    .is_some_and(|path| path.starts_with("lib/"))
            })
            .map(|finding| format!("{} {}", finding["path"], finding["message"]))
            .collect::<Vec<_>>();
        found.sort();
        found
    };

    assert_eq!(under_lib(&sub), under_lib(&whole));
    assert_eq!(found_members(&sub), ["Util.spare"]);

    Ok(())
}

#[test]
fn scanning_a_folder_without_a_package_says_so() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let mut output = Vec::new();

    let error = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().join("lib").to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )
    .err()
    .ok_or("a folder without a package must not pass")?;

    let message = error.to_string();
    assert!(message.contains("no pubspec.yaml"), "{message}");
    assert!(message.contains("dart-decimate check"), "{message}");

    Ok(())
}

#[test]
fn human_output_without_findings_names_the_next_step() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\npublish_to: none\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "class Other extends Missing {\n  int used() => 1;\n}\n\nvoid main() {\n  print(Other().used());\n}\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "human",
        ],
        &mut output,
    )?;

    let text = String::from_utf8(output)?;
    assert_eq!(code, 0, "{text}");
    assert!(text.contains("No findings"), "{text}");
    assert!(text.contains("Next Steps"), "{text}");
    assert!(text.contains("flutter pub get"), "{text}");

    Ok(())
}

#[test]
fn bin_class_members_report() -> Result<(), Box<dyn std::error::Error>> {
    let tool = "class Tool {\n  int used() => 1;\n\n  int spare() => 2;\n\n  int _hidden() => 3;\n}\n\nenum Mode { fast, slow }\n\nvoid main() {\n  print(Tool().used());\n  print(Mode.fast);\n}\n";
    let cases = [
        ("name: app\npublish_to: none\n", "lib/main.dart"),
        ("name: pkg\nversion: 1.0.0\n", "lib/pkg.dart"),
    ];
    for (pubspec, library) in cases {
        let fixture = tempfile::tempdir()?;
        write(&fixture, "pubspec.yaml", pubspec)?;
        write(&fixture, library, "void main() {}\n")?;
        write(&fixture, "bin/tool.dart", tool)?;

        let (_, json) = strict_check(&fixture)?;

        assert_eq!(
            found_members(&json),
            ["Tool.spare", "Tool._hidden", "Mode.slow"],
            "{pubspec}"
        );
    }

    Ok(())
}

#[test]
fn classes_with_supertypes_report_own_unused_members() -> Result<(), Box<dyn std::error::Error>> {
    let main = "import 'package:flutter/widgets.dart';\nimport 'src/items.dart';\n\nvoid main() {\n  print(Home());\n  print(Page());\n  print(Child());\n  print(Marked());\n  print(Loose());\n  Base().hook();\n}\n";
    let items = "import 'package:flutter/widgets.dart';\n\nclass Home extends StatelessWidget {\n  Widget build(BuildContext context) => throw UnimplementedError();\n\n  int spare() => 1;\n}\n\nclass Page extends State<StatefulWidget> {\n  void initState() {}\n\n  int spare() => 1;\n}\n\nclass Base {\n  void hook() {}\n}\n\nclass Child extends Base {\n  void hook() {}\n\n  int spare() => 1;\n}\n\n@Deprecated('x')\nclass Marked extends Base {\n  int spare() => 1;\n}\n\nclass Loose extends Missing {\n  int spare() => 1;\n}\n";
    let (_, json) = strict_check_with_toolchain(items, main)?;

    assert_eq!(
        found_members(&json),
        ["Home.spare", "Page.spare", "Child.spare"]
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
    strict_check_with(fixture, &[])
}

fn strict_check_with(
    fixture: &TempDir,
    extra: &[&str],
) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let mut args = vec![
        "dart-decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--threshold",
        "0",
        "--strict",
        "--format",
        "json",
    ];
    args.extend_from_slice(extra);
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
