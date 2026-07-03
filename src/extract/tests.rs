use super::*;

#[test]
fn extracts_imports_exports_and_top_level_declarations() -> Result<(), ExtractError> {
    let source = "\
import 'dart:async';
import \"package:app/src/service.dart\";
import '../shared.dart' as shared show Thing;
export 'src/public.dart';
part 'src/generated.g.dart';

class App {
  void render() {}
}
enum Mode { light, dark }
void main() { void nested() {} }
int helper(String value) => value.length;
";

    let extracted = extract_dart_source("lib/main.dart", source)?;

    assert_eq!(
        extracted.imports,
        vec![
            DartImport {
                uri: "dart:async".to_owned(),
                condition: None,
                prefix: None,
                deferred: false,
                combinators: Vec::new(),
                location: loc(1, 0),
            },
            DartImport {
                uri: "package:app/src/service.dart".to_owned(),
                condition: None,
                prefix: None,
                deferred: false,
                combinators: Vec::new(),
                location: loc(2, 0),
            },
            DartImport {
                uri: "../shared.dart".to_owned(),
                condition: None,
                prefix: Some("shared".to_owned()),
                deferred: false,
                combinators: vec![DartCombinator {
                    kind: DartCombinatorKind::Show,
                    names: vec!["Thing".to_owned()],
                    location: loc(3, 34),
                }],
                location: loc(3, 0),
            },
        ]
    );
    assert_eq!(
        extracted.exports,
        vec![DartExport {
            uri: "src/public.dart".to_owned(),
            condition: None,
            combinators: Vec::new(),
            location: loc(4, 0),
        }]
    );
    assert_eq!(
        extracted.parts,
        vec![DartPart {
            uri: "src/generated.g.dart".to_owned(),
            location: loc(5, 0),
        }]
    );
    assert_eq!(
        extracted.declarations,
        vec![
            TopLevelDeclaration {
                kind: DeclarationKind::Class,
                name: "App".to_owned(),
                location: loc(7, 0),
                range: range(7, 9),
            },
            TopLevelDeclaration {
                kind: DeclarationKind::Enum,
                name: "Mode".to_owned(),
                location: loc(10, 0),
                range: range(10, 10),
            },
            TopLevelDeclaration {
                kind: DeclarationKind::Function,
                name: "main".to_owned(),
                location: loc(11, 0),
                range: range(11, 11),
            },
            TopLevelDeclaration {
                kind: DeclarationKind::Function,
                name: "helper".to_owned(),
                location: loc(12, 0),
                range: range(12, 12),
            },
        ]
    );

    Ok(())
}

#[test]
fn extracts_import_export_visibility_metadata() -> Result<(), ExtractError> {
    let source = "\
import 'deferred.dart' deferred as deferred_lib show DeferredThing hide HiddenThing;
import 'side_effects.dart' as _;
export 'public.dart' show PublicThing hide InternalThing;
";

    let extracted = extract_dart_source("lib/visibility.dart", source)?;

    assert_eq!(extracted.imports[0].uri, "deferred.dart");
    assert_eq!(extracted.imports[0].prefix.as_deref(), Some("deferred_lib"));
    assert!(extracted.imports[0].deferred);
    assert_eq!(
        extracted.imports[0]
            .combinators
            .iter()
            .map(|combinator| (combinator.kind, combinator.names.as_slice()))
            .collect::<Vec<_>>(),
        vec![
            (DartCombinatorKind::Show, &["DeferredThing".to_owned()][..]),
            (DartCombinatorKind::Hide, &["HiddenThing".to_owned()][..]),
        ]
    );
    assert_eq!(extracted.imports[1].uri, "side_effects.dart");
    assert_eq!(extracted.imports[1].prefix, None);
    assert_eq!(
        extracted.exports[0]
            .combinators
            .iter()
            .map(|combinator| (combinator.kind, combinator.names.as_slice()))
            .collect::<Vec<_>>(),
        vec![
            (DartCombinatorKind::Show, &["PublicThing".to_owned()][..]),
            (DartCombinatorKind::Hide, &["InternalThing".to_owned()][..]),
        ]
    );

    Ok(())
}

#[test]
fn extracts_library_and_part_of_directive_metadata() -> Result<(), ExtractError> {
    let library_source = "\
@visibleForTesting
library app.src.model;
part 'model_part.dart';
";
    let unnamed_source = "library;\n";
    let augment_source = "library augment 'src/base.dart';\n";
    let named_part_source = "part of app.src.model;\n";
    let uri_part_source = "part of 'model.dart';\n";

    let library = extract_dart_source("lib/src/model.dart", library_source)?;
    let unnamed = extract_dart_source("lib/unnamed.dart", unnamed_source)?;
    let augment = extract_dart_source("lib/augment.dart", augment_source)?;
    let named_part = extract_dart_source("lib/src/model_part.dart", named_part_source)?;
    let uri_part = extract_dart_source("lib/src/model_uri_part.dart", uri_part_source)?;

    assert_eq!(
        library.library,
        Some(DartLibrary {
            name: Some("app.src.model".to_owned()),
            augment_uri: None,
            location: Location { line: 1, column: 0 },
        })
    );
    assert_eq!(
        unnamed.library,
        Some(DartLibrary {
            name: None,
            augment_uri: None,
            location: Location { line: 1, column: 0 },
        })
    );
    assert_eq!(
        augment.library,
        Some(DartLibrary {
            name: None,
            augment_uri: Some("src/base.dart".to_owned()),
            location: Location { line: 1, column: 0 },
        })
    );
    assert_eq!(
        named_part.part_of,
        Some(DartPartOf {
            name: Some("app.src.model".to_owned()),
            uri: None,
            location: Location { line: 1, column: 0 },
        })
    );
    assert_eq!(
        uri_part.part_of,
        Some(DartPartOf {
            name: None,
            uri: Some("model.dart".to_owned()),
            location: Location { line: 1, column: 0 },
        })
    );

    Ok(())
}

#[test]
fn import_deferred_flag_requires_deferred_as_clause() -> Result<(), ExtractError> {
    let source = "\
import 'deferred.dart' as loader;
import 'api.dart' as deferred_loader;
import 'stub.dart' if (dart.library.io) 'deferred' as platform;
import 'comment.dart' /* deferred as */ as comment_alias;
import 'commented_real.dart' deferred /*comment*/ as commented_real;
import 'real.dart' deferred as real;
";

    let extracted = extract_dart_source("lib/deferred_flags.dart", source)?;

    assert_eq!(
        extracted
            .imports
            .iter()
            .map(|import| {
                (
                    import.uri.as_str(),
                    import.prefix.as_deref(),
                    import.deferred,
                )
            })
            .collect::<Vec<_>>(),
        vec![
            ("deferred.dart", Some("loader"), false),
            ("api.dart", Some("deferred_loader"), false),
            ("stub.dart", Some("platform"), false),
            ("deferred", Some("platform"), false),
            ("comment.dart", Some("comment_alias"), false),
            ("commented_real.dart", Some("commented_real"), true),
            ("real.dart", Some("real"), true),
        ]
    );

    Ok(())
}

#[test]
fn extracts_modifier_and_external_function_declarations() -> Result<(), ExtractError> {
    let source = "\
@pragma('vm:entry-point')
external Future<void> boot();
String get currentName => 'dart_decimate';
external set currentName(String value);
sealed class State {}
enum Kind { primary }
";

    let extracted = extract_dart_source("lib/bootstrap.dart", source)?;

    assert_eq!(
        extracted.declarations,
        vec![
            TopLevelDeclaration {
                kind: DeclarationKind::Function,
                name: "boot".to_owned(),
                location: Location { line: 1, column: 0 },
                range: range(1, 2),
            },
            TopLevelDeclaration {
                kind: DeclarationKind::Function,
                name: "currentName".to_owned(),
                location: Location { line: 3, column: 0 },
                range: range(3, 3),
            },
            TopLevelDeclaration {
                kind: DeclarationKind::Function,
                name: "currentName".to_owned(),
                location: Location { line: 4, column: 0 },
                range: range(4, 4),
            },
            TopLevelDeclaration {
                kind: DeclarationKind::Class,
                name: "State".to_owned(),
                location: Location { line: 5, column: 0 },
                range: range(5, 5),
            },
            TopLevelDeclaration {
                kind: DeclarationKind::Enum,
                name: "Kind".to_owned(),
                location: Location { line: 6, column: 0 },
                range: range(6, 6),
            },
        ]
    );

    Ok(())
}

#[test]
fn extracts_dart_top_level_symbol_declarations() -> Result<(), ExtractError> {
    let source = "\
mixin Trackable {}
extension FancyString on String {}
extension type UserId(String value) {}
typedef JsonMap = Map<String, Object?>;
typedef String Stringifier(int value);
const topLevelLimit = 1, topLevelOther = 2;
late final cached = 1;
external String hostName;
class Alias = Object with Trackable;
";

    let extracted = extract_dart_source("lib/symbols.dart", source)?;

    assert_eq!(
        extracted
            .declarations
            .iter()
            .map(|declaration| (declaration.kind, declaration.name.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (DeclarationKind::Mixin, "Trackable"),
            (DeclarationKind::Extension, "FancyString"),
            (DeclarationKind::ExtensionType, "UserId"),
            (DeclarationKind::TypeAlias, "JsonMap"),
            (DeclarationKind::TypeAlias, "Stringifier"),
            (DeclarationKind::Variable, "topLevelLimit"),
            (DeclarationKind::Variable, "topLevelOther"),
            (DeclarationKind::Variable, "cached"),
            (DeclarationKind::Variable, "hostName"),
            (DeclarationKind::Class, "Alias"),
        ]
    );

    Ok(())
}

#[test]
fn parses_modern_dart_primary_constructor_syntax() -> Result<(), ExtractError> {
    let source = "\
extension type UserId(int value) implements int {}

class Point(var int x, var int y);

enum Color(final String hex) {
  red('#FF0000'),
  blue('#0000FF');
}

void useModern() {
  final Color color = .red;
  final point = Point(1, 2);
  final (:name, :age) = (name: 'Ada', age: 37);
  switch ((name, age)) {
    case ('Ada', final years):
      print('$color $point $years');
  }
}
";

    let extracted = extract_dart_source("lib/modern.dart", source)?;

    assert_eq!(
        extracted
            .declarations
            .iter()
            .map(|declaration| (declaration.kind, declaration.name.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (DeclarationKind::ExtensionType, "UserId"),
            (DeclarationKind::Class, "Point"),
            (DeclarationKind::Enum, "Color"),
            (DeclarationKind::Function, "useModern"),
        ]
    );
    assert!(
        extracted
            .references
            .iter()
            .any(|reference| reference.name == "Color")
    );
    assert!(
        extracted
            .references
            .iter()
            .any(|reference| reference.name == "Point")
    );

    Ok(())
}

#[test]
fn excludes_typedef_names_from_identifier_references() -> Result<(), ExtractError> {
    let source = "\
typedef UsedAlias = String;
typedef UnusedAlias = int;

void run(UsedAlias value) {}
";

    let extracted = extract_dart_source("lib/types.dart", source)?;
    let names = extracted
        .references
        .iter()
        .map(|reference| reference.name.as_str())
        .collect::<Vec<_>>();

    assert!(names.contains(&"UsedAlias"));
    assert!(!names.contains(&"UnusedAlias"));

    Ok(())
}

#[test]
fn extracts_public_signature_type_references_without_bodies() -> Result<(), ExtractError> {
    let source = "\
class Api extends _Base with _Mixin implements _Contract {
  _BodyOnly make() => throw _BodyOnly();
}

typedef Alias = _Hidden Function(_Input value);
_TopLevel expose(_Param value) => throw _BodyOnly();
final _Hidden exposed = _Hidden();
final _Hidden first = _Hidden(), second = _Hidden();
";

    let extracted = extract_dart_source("lib/api.dart", source)?;
    let references = extracted
        .signature_references
        .iter()
        .map(|reference| (reference.declaration.as_str(), reference.name.as_str()))
        .collect::<Vec<_>>();

    assert!(references.contains(&("Api", "_Base")));
    assert!(references.contains(&("Api", "_Mixin")));
    assert!(references.contains(&("Api", "_Contract")));
    assert!(references.contains(&("Alias", "_Hidden")));
    assert!(references.contains(&("Alias", "_Input")));
    assert!(references.contains(&("expose", "_TopLevel")));
    assert!(references.contains(&("expose", "_Param")));
    assert!(references.contains(&("exposed", "_Hidden")));
    assert!(references.contains(&("first", "_Hidden")));
    assert!(references.contains(&("second", "_Hidden")));
    assert!(!references.iter().any(|(_, name)| *name == "_BodyOnly"));
    assert!(
        !references
            .iter()
            .any(|(declaration, _)| *declaration == "<variable>")
    );

    Ok(())
}

#[test]
fn extracts_class_like_member_declarations() -> Result<(), ExtractError> {
    let source = "\
class User {
  final String name;
  static const role = 'admin';
  User(this.name);
  User.named(this.name);
  String get label => name;
  set label(String value) {}
  void save() {}
  bool operator ==(Object other) => identical(this, other);
}

enum Mode {
  light, dark;
  const Mode();
  bool get enabled => true;
}

extension Fancy on String {
  int get chars => length;
  void track() {}
}

extension type UserId(String value) {
  bool get blank => value.isEmpty;
}
";

    let extracted = extract_dart_source("lib/members.dart", source)?;
    let members = extracted
        .members
        .iter()
        .map(|member| (member.owner.as_str(), member.kind, member.name.as_str()))
        .collect::<Vec<_>>();

    for expected in [
        ("User", MemberKind::Field, "name"),
        ("User", MemberKind::Field, "role"),
        ("User", MemberKind::Constructor, "User"),
        ("User", MemberKind::Constructor, "named"),
        ("User", MemberKind::Getter, "label"),
        ("User", MemberKind::Setter, "label"),
        ("User", MemberKind::Method, "save"),
        ("User", MemberKind::Operator, "=="),
        ("Mode", MemberKind::EnumConstant, "light"),
        ("Mode", MemberKind::EnumConstant, "dark"),
        ("Mode", MemberKind::Constructor, "Mode"),
        ("Mode", MemberKind::Getter, "enabled"),
        ("Fancy", MemberKind::Getter, "chars"),
        ("Fancy", MemberKind::Method, "track"),
        ("UserId", MemberKind::Getter, "blank"),
    ] {
        assert!(members.contains(&expected), "missing member {expected:?}");
    }

    let references = extracted
        .references
        .iter()
        .map(|reference| reference.name.as_str())
        .collect::<Vec<_>>();
    for declared_name in [
        "named", "role", "label", "save", "enabled", "chars", "track", "blank",
    ] {
        assert!(
            !references.contains(&declared_name),
            "member declaration name {declared_name} was counted as a reference"
        );
    }

    Ok(())
}

#[test]
fn extracts_identifier_references_without_directive_metadata_or_declaration_names()
-> Result<(), ExtractError> {
    let source = "\
import 'src/internal.dart' as internal show InternalThing hide HiddenThing;
export 'src/public.dart' show PublicThing;

class Uses extends Base with Trackable implements Contract {
  final InternalThing? value;
  Uses(this.value);
}

void boot(Config config) {
  final service = internal.Service();
  service.start(config);
}
";

    let extracted = extract_dart_source("lib/references.dart", source)?;
    let references = extracted
        .references
        .iter()
        .map(|reference| (reference.name.as_str(), reference.location))
        .collect::<Vec<_>>();

    assert!(references.contains(&(
        "Base",
        Location {
            line: 4,
            column: 19
        }
    )));
    assert!(references.contains(&(
        "Trackable",
        Location {
            line: 4,
            column: 29
        }
    )));
    assert!(references.contains(&(
        "Contract",
        Location {
            line: 4,
            column: 50
        }
    )));
    assert!(references.contains(&("InternalThing", Location { line: 5, column: 8 })));
    assert!(references.contains(&(
        "internal",
        Location {
            line: 10,
            column: 18
        }
    )));
    assert!(references.contains(&(
        "Service",
        Location {
            line: 10,
            column: 27
        }
    )));
    assert!(references.contains(&(
        "start",
        Location {
            line: 11,
            column: 10
        }
    )));
    assert!(!references.iter().any(|(name, _)| *name == "HiddenThing"));
    assert!(!references.iter().any(|(name, _)| *name == "PublicThing"));
    assert!(!references.iter().any(|(name, _)| *name == "Uses"));
    assert!(!references.iter().any(|(name, _)| *name == "boot"));

    Ok(())
}

#[test]
fn extracts_interpolation_references_without_raw_string_prefixes() -> Result<(), ExtractError> {
    let source = r#"
void boot(String suffix, String escaped) {
  print('$suffix ${suffix.toUpperCase()} ${r"raw literal"} \$escaped');
}
"#;

    let extracted = extract_dart_source("lib/interpolation.dart", source)?;
    let references = extracted
        .references
        .iter()
        .map(|reference| reference.name.as_str())
        .collect::<Vec<_>>();

    assert!(references.contains(&"suffix"));
    assert!(references.contains(&"toUpperCase"));
    assert!(!references.contains(&"r"));
    assert!(!references.contains(&"raw"));
    assert!(!references.contains(&"literal"));
    assert!(!references.contains(&"escaped"));

    Ok(())
}

#[test]
fn extracts_references_from_annotations_generics_and_patterns() -> Result<(), ExtractError> {
    let source = "\
@Route(path: '/home')
class HomeController {
  Future<Result<User?>> load(Map<String, List<User>> items) => throw UnimplementedError();
}

void read(Object value, Object record) {
  if (value case User(role: Role.admin)) {}
  if (value case AsyncData(value: final user)) { print(user); }
  switch (record) { case (:final name, _): print(name); }
}
";

    let extracted = extract_dart_source("lib/typed.dart", source)?;
    let names = extracted
        .references
        .iter()
        .map(|reference| reference.name.as_str())
        .collect::<Vec<_>>();

    for expected in
        "Route Future Result User Map String List Object Role AsyncData".split_whitespace()
    {
        assert!(names.contains(&expected), "missing reference {expected}");
    }
    assert!(!names.contains(&"HomeController"));
    assert!(!names.contains(&"load"));
    assert!(!names.contains(&"read"));
    assert!(!names.contains(&"_"));
    assert_eq!(names.iter().filter(|name| **name == "user").count(), 1);
    assert_eq!(names.iter().filter(|name| **name == "name").count(), 1);

    Ok(())
}

const fn range(start_line: usize, end_line: usize) -> SourceRange {
    SourceRange {
        start_line,
        end_line,
    }
}

const fn loc(line: usize, column: usize) -> Location {
    Location { line, column }
}

#[test]
fn extracts_all_uris_from_configurable_directives() -> Result<(), ExtractError> {
    let source = "\
import 'io.dart' if (dart.library.html) 'html.dart' if (dart.library.js_interop == 'true') 'wasm.dart';
export r'src\\raw.dart' if (dart.library.html) 'src\\web.dart';
";

    let extracted = extract_dart_source("lib/platform.dart", source)?;

    assert_eq!(
        extracted
            .imports
            .iter()
            .map(|import| (
                import.uri.as_str(),
                import.condition.as_ref().map(|condition| (
                    condition.variable.as_str(),
                    condition.expected_value.as_str()
                ))
            ))
            .collect::<Vec<_>>(),
        vec![
            ("io.dart", None),
            ("html.dart", Some(("dart.library.html", "true"))),
            ("wasm.dart", Some(("dart.library.js_interop", "true")))
        ]
    );
    assert_eq!(
        extracted
            .exports
            .iter()
            .map(|export| (
                export.uri.as_str(),
                export.condition.as_ref().map(|condition| (
                    condition.variable.as_str(),
                    condition.expected_value.as_str()
                ))
            ))
            .collect::<Vec<_>>(),
        vec![
            ("src\\raw.dart", None),
            ("src\\web.dart", Some(("dart.library.html", "true")))
        ]
    );

    Ok(())
}
