use std::fs;

use tempfile::TempDir;

use super::*;
use crate::{find_dead_code, scan_project};

#[test]
fn reports_unreferenced_public_declarations_in_reachable_internal_libraries()
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
        "class Used {}\nclass Unused {}\nvoid helper() {}\nclass Review {\n}\n",
    )?;
    let project = scan_project(fixture.path())?;
    let dead_code = find_dead_code(&project.graph, ["lib/main.dart"]);

    let report = analyze_unused_exports(&project, &dead_code);

    assert_eq!(
        report
            .unused_exports
            .iter()
            .map(|unused| (unused.name.as_str(), unused.kind, unused.safe_to_delete))
            .collect::<Vec<_>>(),
        vec![
            ("Unused", DeclarationKind::Class, true),
            ("helper", DeclarationKind::Function, true),
            ("Review", DeclarationKind::Class, false),
        ]
    );

    Ok(())
}

#[test]
fn skips_private_entry_generated_and_dead_file_declarations()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/live.dart';\nvoid main() { Live(); }\nclass EntryOnly {}\n",
    )?;
    write(
        &fixture,
        "lib/src/live.dart",
        "part 'live.g.dart';\nclass Live {}\nclass _Private {}\n",
    )?;
    write(&fixture, "lib/src/live.g.dart", "class Generated {}\n")?;
    write(&fixture, "lib/src/dead.dart", "class Dead {}\n")?;
    let project = scan_project(fixture.path())?;
    let dead_code = find_dead_code(&project.graph, ["lib/main.dart"]);

    let report = analyze_unused_exports(&project, &dead_code);

    assert!(report.unused_exports.is_empty());

    Ok(())
}

#[test]
fn include_entry_exports_reports_entry_declarations_except_main()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "void main() {}\nclass EntryOnly {}\nvoid helper() {}\n",
    )?;
    let project = scan_project(fixture.path())?;
    let dead_code = find_dead_code(&project.graph, ["lib/main.dart"]);

    let default_report = analyze_symbols(&project, Some(&dead_code));
    assert!(default_report.unused_exports.is_empty());

    let report = analyze_symbols_with_options(
        &project,
        Some(&dead_code),
        SymbolAnalysisOptions {
            include_entry_exports: true,
            private_type_leaks: false,
        },
    );

    assert_eq!(
        report
            .unused_exports
            .iter()
            .map(|unused| unused.name.as_str())
            .collect::<Vec<_>>(),
        vec!["EntryOnly", "helper"]
    );

    Ok(())
}

#[test]
fn treats_public_library_exports_as_api_usage() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "export 'src/api.dart' show PublicApi;\n",
    )?;
    write(
        &fixture,
        "lib/src/api.dart",
        "class PublicApi {}\nclass HiddenApi {}\n",
    )?;
    let project = scan_project(fixture.path())?;
    let dead_code = find_dead_code(&project.graph, ["lib/package.dart"]);

    let report = analyze_unused_exports(&project, &dead_code);

    assert_eq!(
        report
            .unused_exports
            .iter()
            .map(|unused| unused.name.as_str())
            .collect::<Vec<_>>(),
        vec!["HiddenApi"]
    );

    Ok(())
}

#[test]
fn treats_public_library_part_declarations_as_api_usage() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(&fixture, "lib/package.dart", "part 'src/api_part.dart';\n")?;
    write(
        &fixture,
        "lib/src/api_part.dart",
        "part of '../package.dart';\nclass PartApi {}\n",
    )?;
    let project = scan_project(fixture.path())?;
    let dead_code = find_dead_code(&project.graph, ["lib/package.dart"]);

    let report = analyze_unused_exports(&project, &dead_code);

    assert!(report.unused_exports.is_empty());

    Ok(())
}

#[test]
fn re_exported_library_part_declarations_respect_combinators()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "export 'src/api.dart' show PublicPartApi;\n",
    )?;
    write(&fixture, "lib/src/api.dart", "part 'api_part.dart';\n")?;
    write(
        &fixture,
        "lib/src/api_part.dart",
        "part of 'api.dart';\nclass PublicPartApi {}\nclass HiddenPartApi {}\n",
    )?;
    let project = scan_project(fixture.path())?;
    let dead_code = find_dead_code(&project.graph, ["lib/package.dart"]);

    let report = analyze_unused_exports(&project, &dead_code);

    assert_eq!(
        report
            .unused_exports
            .iter()
            .map(|unused| unused.name.as_str())
            .collect::<Vec<_>>(),
        vec!["HiddenPartApi"]
    );

    Ok(())
}

#[test]
fn reports_unused_enum_constants_and_private_class_members()
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
enum Mode { on, off }

class Controller {
  Controller();
  Controller.named();
  final int _unusedField = 1;
  void _used() {}
  void _unused() {}
  void publicUnused() {}
  bool operator ==(Object other) => identical(this, other);
}

extension InternalText on String {
  int get _unusedLength => length;
}

void runLive() {
  final controller = Controller();
  controller._used();
  final mode = Mode.on;
  print(mode);
}
",
    )?;
    let project = scan_project(fixture.path())?;
    let dead_code = find_dead_code(&project.graph, ["lib/main.dart"]);

    let report = analyze_unused_exports(&project, &dead_code);

    assert_eq!(
        report
            .unused_members
            .iter()
            .map(|unused| (unused.owner.as_str(), unused.name.as_str(), unused.kind))
            .collect::<Vec<_>>(),
        vec![
            ("Mode", "off", MemberKind::EnumConstant),
            ("Controller", "_unusedField", MemberKind::Field),
            ("Controller", "_unused", MemberKind::Method),
            ("InternalText", "_unusedLength", MemberKind::Getter),
        ]
    );

    Ok(())
}

#[test]
fn skips_enum_constants_on_public_api_enums() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(&fixture, "lib/package.dart", "export 'src/api.dart';\n")?;
    write(
        &fixture,
        "lib/src/api.dart",
        "enum PublicMode { one, two }\n",
    )?;
    let project = scan_project(fixture.path())?;
    let dead_code = find_dead_code(&project.graph, ["lib/package.dart"]);

    let report = analyze_unused_exports(&project, &dead_code);

    assert!(report.unused_members.is_empty());

    Ok(())
}

#[test]
fn counts_native_dot_shorthand_enum_references() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        r"enum Readiness { loading, signedOut, ready }

String label(Readiness readiness) => switch (readiness) {
  .loading => 'loading',
  .signedOut => 'signed-out',
  .ready => 'ready',
};

void main() => print(label(.loading));
",
    )?;
    let project = scan_project(fixture.path())?;
    let dead_code = find_dead_code(&project.graph, ["lib/main.dart"]);

    let report = analyze_unused_exports(&project, &dead_code);

    assert!(
        report
            .unused_members
            .iter()
            .all(|unused| unused.owner != "Readiness"),
        "{:?}",
        report.unused_members
    );
    Ok(())
}

#[test]
fn member_references_are_scoped_to_dart_library_parts() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/live.dart';\nimport 'src/other.dart';\nvoid main() { runLive(); runOther(); }\n",
    )?;
    write(
        &fixture,
        "lib/src/live.dart",
        "\
part 'live_part.dart';
class Live {
  void _fromPart() {}
  void _shadowedElsewhere() {}
}
void runLive() { Live()._fromPart(); }
",
    )?;
    write(
        &fixture,
        "lib/src/live_part.dart",
        "part of 'live.dart';\nvoid touchPart(Live live) { live._fromPart(); }\n",
    )?;
    write(
        &fixture,
        "lib/src/other.dart",
        "\
class Other {
  void _shadowedElsewhere() {}
}
void runOther() { Other()._shadowedElsewhere(); }
",
    )?;
    let project = scan_project(fixture.path())?;
    let dead_code = find_dead_code(&project.graph, ["lib/main.dart"]);

    let report = analyze_unused_exports(&project, &dead_code);

    assert_eq!(
        report
            .unused_members
            .iter()
            .map(|unused| (unused.owner.as_str(), unused.name.as_str()))
            .collect::<Vec<_>>(),
        vec![("Live", "_shadowedElsewhere")]
    );

    Ok(())
}

#[test]
fn groups_field_getter_and_setter_member_findings() -> Result<(), Box<dyn std::error::Error>> {
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
class Live {
  int _value = 0;
  int get _value => 1;
  set _value(int value) {}
}
void runLive() { Live(); }
",
    )?;
    let project = scan_project(fixture.path())?;
    let dead_code = find_dead_code(&project.graph, ["lib/main.dart"]);

    let report = analyze_unused_exports(&project, &dead_code);

    assert_eq!(
        report
            .unused_members
            .iter()
            .filter(|unused| unused.name == "_value")
            .count(),
        1
    );

    Ok(())
}

#[test]
fn propagates_public_api_through_re_export_chains() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(&fixture, "lib/package.dart", "export 'src/barrel.dart';\n")?;
    write(
        &fixture,
        "lib/src/barrel.dart",
        "export 'api.dart' show PublicApi;\n",
    )?;
    write(
        &fixture,
        "lib/src/api.dart",
        "class PublicApi {}\nclass HiddenApi {}\n",
    )?;
    let project = scan_project(fixture.path())?;
    let dead_code = find_dead_code(&project.graph, ["lib/package.dart"]);

    let report = analyze_unused_exports(&project, &dead_code);

    assert_eq!(
        report
            .unused_exports
            .iter()
            .map(|unused| unused.name.as_str())
            .collect::<Vec<_>>(),
        vec!["HiddenApi"]
    );

    Ok(())
}

#[test]
fn reports_duplicate_public_symbol_from_public_barrel() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "export 'src/a.dart';\nexport 'src/b.dart';\n",
    )?;
    write(&fixture, "lib/src/a.dart", "class Api {}\nclass OnlyA {}\n")?;
    write(&fixture, "lib/src/b.dart", "class Api {}\nclass OnlyB {}\n")?;
    let project = scan_project(fixture.path())?;

    let report = analyze_symbols(&project, None);

    assert_eq!(report.duplicate_exports.len(), 1);
    let duplicate = &report.duplicate_exports[0];
    assert_eq!(
        duplicate.entry_path,
        fixture.path().join("lib/package.dart")
    );
    assert_eq!(duplicate.name, "Api");
    assert_eq!(
        duplicate
            .declarations
            .iter()
            .map(|declaration| declaration.path.clone())
            .collect::<Vec<_>>(),
        vec![
            fixture.path().join("lib/src/a.dart"),
            fixture.path().join("lib/src/b.dart"),
        ]
    );
    assert!(!duplicate.safe_to_delete);

    Ok(())
}

#[test]
fn duplicate_exports_respect_show_and_hide() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "export 'src/a.dart' show Api;\nexport 'src/b.dart' hide Api;\n",
    )?;
    write(&fixture, "lib/src/a.dart", "class Api {}\n")?;
    write(&fixture, "lib/src/b.dart", "class Api {}\nclass Other {}\n")?;
    let project = scan_project(fixture.path())?;

    let report = analyze_symbols(&project, None);

    assert!(report.duplicate_exports.is_empty());

    Ok(())
}

#[test]
fn reports_duplicate_public_symbol_through_re_export_chain()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "export 'src/feature_a.dart';\nexport 'src/feature_b.dart';\n",
    )?;
    write(&fixture, "lib/src/feature_a.dart", "export 'a_api.dart';\n")?;
    write(&fixture, "lib/src/feature_b.dart", "export 'b_api.dart';\n")?;
    write(&fixture, "lib/src/a_api.dart", "class Api {}\n")?;
    write(&fixture, "lib/src/b_api.dart", "class Api {}\n")?;
    let project = scan_project(fixture.path())?;

    let report = analyze_symbols(&project, None);

    assert_eq!(report.duplicate_exports.len(), 1);
    assert_eq!(
        report.duplicate_exports[0].entry_path,
        fixture.path().join("lib/package.dart")
    );
    assert_eq!(
        report.duplicate_exports[0]
            .declarations
            .iter()
            .map(|declaration| declaration.path.clone())
            .collect::<Vec<_>>(),
        vec![
            fixture.path().join("lib/src/a_api.dart"),
            fixture.path().join("lib/src/b_api.dart"),
        ]
    );

    Ok(())
}

#[test]
fn duplicate_exports_respect_show_hide_at_each_chain_hop() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "export 'src/feature_a.dart' show Api;\nexport 'src/feature_b.dart';\n",
    )?;
    write(
        &fixture,
        "lib/src/feature_a.dart",
        "export 'a_api.dart' hide Api;\n",
    )?;
    write(
        &fixture,
        "lib/src/feature_b.dart",
        "export 'b_api.dart' show Api;\n",
    )?;
    write(&fixture, "lib/src/a_api.dart", "class Api {}\n")?;
    write(&fixture, "lib/src/b_api.dart", "class Api {}\n")?;
    let project = scan_project(fixture.path())?;

    let report = analyze_symbols(&project, None);

    assert!(report.duplicate_exports.is_empty());

    Ok(())
}

#[test]
fn does_not_treat_private_src_barrel_as_public_surface() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/src/internal_barrel.dart",
        "export 'a.dart';\nexport 'b.dart';\n",
    )?;
    write(&fixture, "lib/src/a.dart", "class Api {}\n")?;
    write(&fixture, "lib/src/b.dart", "class Api {}\n")?;
    let project = scan_project(fixture.path())?;

    let report = analyze_symbols(&project, None);

    assert!(report.duplicate_exports.is_empty());

    Ok(())
}

#[test]
fn reports_private_type_leaks_from_public_library_and_parts()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "\
part 'src/api_part.dart';
class Api extends _Hidden {}
final _Hidden exposed = _Hidden();
class _Hidden {}
",
    )?;
    write(
        &fixture,
        "lib/src/api_part.dart",
        "part of '../package.dart';\ntypedef PartAlias = _Hidden Function();\n",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_symbols_with_options(
        &project,
        None,
        SymbolAnalysisOptions {
            include_entry_exports: false,
            private_type_leaks: true,
        },
    );

    assert_eq!(
        report
            .private_type_leaks
            .iter()
            .map(|leak| (leak.declaration.as_str(), leak.private_type.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("Api", "_Hidden"),
            ("exposed", "_Hidden"),
            ("PartAlias", "_Hidden"),
        ]
    );

    Ok(())
}

#[test]
fn private_type_leaks_respect_export_combinators() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "export 'src/api.dart' show PublicApi;\n",
    )?;
    write(
        &fixture,
        "lib/src/api.dart",
        "class PublicApi extends _Hidden {}\nclass HiddenApi extends _Hidden {}\nclass _Hidden {}\n",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_symbols_with_options(
        &project,
        None,
        SymbolAnalysisOptions {
            include_entry_exports: false,
            private_type_leaks: true,
        },
    );

    assert_eq!(
        report
            .private_type_leaks
            .iter()
            .map(|leak| leak.declaration.as_str())
            .collect::<Vec<_>>(),
        vec!["PublicApi"]
    );

    Ok(())
}

#[test]
fn private_type_leaks_require_same_dart_library_private_type()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(&fixture, "lib/package.dart", "export 'src/api.dart';\n")?;
    write(
        &fixture,
        "lib/src/api.dart",
        "import 'internal.dart';\n_Hidden make() => throw UnimplementedError();\n",
    )?;
    write(&fixture, "lib/src/internal.dart", "class _Hidden {}\n")?;
    let project = scan_project(fixture.path())?;

    let report = analyze_symbols_with_options(
        &project,
        None,
        SymbolAnalysisOptions {
            include_entry_exports: false,
            private_type_leaks: true,
        },
    );

    assert!(report.private_type_leaks.is_empty());

    Ok(())
}

#[test]
fn private_type_leaks_skip_private_and_anonymous_declarations()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "\
class _Api extends _Hidden {}
typedef _Alias = _Hidden Function();
extension on _Hidden {}
class _Hidden {}
",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_symbols_with_options(
        &project,
        None,
        SymbolAnalysisOptions {
            include_entry_exports: false,
            private_type_leaks: true,
        },
    );

    assert!(report.private_type_leaks.is_empty());

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
