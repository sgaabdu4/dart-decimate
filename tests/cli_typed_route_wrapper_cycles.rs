use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;

const NAVIGATION_UTILS: &str = r"import 'package:go_router/go_router.dart';
import 'package:flutter/widgets.dart';
void navigate({required BuildContext context, required GoRouteData destination}) {
  destination.go(context);
}
void inspect({required BuildContext context, required GoRouteData destination}) {
  print(destination);
}
";
const SHADOWED_DESTINATION: &str = r"import 'package:go_router/go_router.dart';
import 'package:flutter/widgets.dart';
void navigate({required BuildContext context, required GoRouteData destination}) {
  <GoRouteData>[].map((destination) => destination.go(context)).toList();
}
";
const SHADOWED_CONTEXT: &str = r"import 'package:go_router/go_router.dart';
import 'package:flutter/widgets.dart';
void navigate({required BuildContext context, required GoRouteData destination}) {
  <BuildContext>[].map((context) => destination.go(context)).toList();
}
";

const CASES: &[(&str, &str, &str, usize)] = &[
    (
        "const DetailRoute().go(context)",
        NAVIGATION_UTILS,
        "import 'navigation_utils.dart';",
        0,
    ),
    (
        "navigate(context: context, destination: const DetailRoute())",
        NAVIGATION_UTILS,
        "import 'navigation_utils.dart';",
        0,
    ),
    (
        "nav.navigate(context: context, destination: const DetailRoute())",
        NAVIGATION_UTILS,
        "import 'navigation_utils.dart' as nav;",
        0,
    ),
    (
        "nav.navigate(context: context, destination: const DetailRoute())",
        NAVIGATION_UTILS,
        "import 'navigation_utils.dart' as nav show navigate;",
        0,
    ),
    (
        "Tile(onPressed: () => navigate(context: context, destination: const DetailRoute()))",
        NAVIGATION_UTILS,
        "import 'navigation_utils.dart';",
        0,
    ),
    (
        "Tile(onPressed: () => nav.navigate(context: context, destination: const DetailRoute()))",
        NAVIGATION_UTILS,
        "import 'navigation_utils.dart' as nav;",
        0,
    ),
    (
        "print(registryLabel)",
        NAVIGATION_UTILS,
        "import 'navigation_utils.dart';",
        1,
    ),
    (
        "inspect(context: context, destination: const DetailRoute())",
        NAVIGATION_UTILS,
        "import 'navigation_utils.dart';",
        1,
    ),
    (
        "navigate(context: context, destination: const FakeRoute())",
        NAVIGATION_UTILS,
        "import 'navigation_utils.dart';",
        1,
    ),
    (
        "Tile(onPressed: () { navigate(context: context, destination: const DetailRoute()); print(registryLabel); })",
        NAVIGATION_UTILS,
        "import 'navigation_utils.dart';",
        1,
    ),
    (
        "navigate(context: context, destination: const DetailRoute())",
        SHADOWED_DESTINATION,
        "import 'navigation_utils.dart';",
        1,
    ),
    (
        "navigate(context: context, destination: const DetailRoute())",
        SHADOWED_CONTEXT,
        "import 'navigation_utils.dart';",
        1,
    ),
    (
        "navigate(context: context, destination: const DetailRoute())",
        NAVIGATION_UTILS,
        "import 'navigation_utils.dart' hide navigate;",
        1,
    ),
    (
        "nav.navigate(context: context, destination: const DetailRoute())",
        NAVIGATION_UTILS,
        "import 'navigation_utils.dart' as nav hide navigate;",
        1,
    ),
    (
        "Tile(onPressed: (nav) => nav.navigate(context: context, destination: const DetailRoute()))",
        NAVIGATION_UTILS,
        "import 'navigation_utils.dart' as nav;",
        1,
    ),
];

#[test]
fn typed_route_wrapper_is_navigation_without_hiding_non_route_cycles()
-> Result<(), Box<dyn std::error::Error>> {
    for &(screen_use, utility_source, utility_import, expected_cycles) in CASES {
        let (code, report) = run_fixture(screen_use, utility_source, utility_import)?;
        let cycles = report["findings"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|finding| finding["rule_id"] == "dart-decimate/circular-dependency")
            .count();
        assert_eq!(cycles, expected_cycles, "{screen_use}: {report:#}");
        if expected_cycles == 0 {
            assert_eq!(code, 0, "{screen_use}: {report:#}");
        }
    }
    Ok(())
}

#[test]
fn local_top_level_wrapper_name_shadows_import_even_when_declared_later()
-> Result<(), Box<dyn std::error::Error>> {
    let (code, report) = run_fixture_with_tail(
        "navigate(context: context, destination: const DetailRoute())",
        NAVIGATION_UTILS,
        "import 'navigation_utils.dart';",
        "void navigate({required BuildContext context, required DetailRoute destination}) { print(destination); }",
    )?;
    assert_eq!(code, 1, "{report:#}");
    assert!(
        report["findings"]
            .as_array()
            .is_some_and(|findings| findings
                .iter()
                .any(|finding| { finding["rule_id"] == "dart-decimate/circular-dependency" })),
        "{report:#}"
    );
    Ok(())
}

fn run_fixture(
    screen_use: &str,
    utility_source: &str,
    utility_import: &str,
) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    run_fixture_with_tail(screen_use, utility_source, utility_import, "")
}

fn run_fixture_with_tail(
    screen_use: &str,
    utility_source: &str,
    utility_import: &str,
    screen_tail: &str,
) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let lib = fixture.path().join("lib");
    fs::create_dir_all(&lib)?;
    fs::write(
        fixture.path().join("pubspec.yaml"),
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n  go_router: any\n",
    )?;
    fs::write(
        lib.join("routes.dart"),
        r"import 'screen.dart';
part 'routes.g.dart';
@TypedGoRoute<DetailRoute>(path: '/detail')
class DetailRoute extends GoRouteData {
  const DetailRoute();
  Widget build(BuildContext context, GoRouterState state) => const Screen();
}
const registryLabel = 'extra';
class TypedGoRoute<T> { const TypedGoRoute({required String path}); }
class GoRouteData {}
class GoRouterState {}
class BuildContext {}
class Widget {}
",
    )?;
    fs::write(lib.join("routes.g.dart"), "part of 'routes.dart';\n")?;
    fs::write(lib.join("navigation_utils.dart"), utility_source)?;
    fs::write(
        lib.join("screen.dart"),
        format!(
            "import 'routes.dart' show DetailRoute, registryLabel;\n{utility_import}\nclass Screen {{\n  const Screen();\n  void open(BuildContext context) => {screen_use};\n}}\nclass FakeRoute {{ const FakeRoute(); }}\n{screen_tail}\n"
        ),
    )?;
    let mut output = Vec::new();
    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--threshold",
            "0",
            "--format",
            "json",
        ],
        &mut output,
    )?;
    let report: Value = serde_json::from_slice(&output)?;
    Ok((code, report))
}
