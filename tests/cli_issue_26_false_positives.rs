use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn check_ignores_generated_l10n_cycles() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'l10n/app_localizations.dart';\nvoid main() { AppLocalizations(); }\n",
    )?;
    write(
        &fixture,
        "lib/l10n/app_localizations.dart",
        "import 'app_localizations_en.dart';\nclass AppLocalizations { AppLocalizationsEn? en; }\n",
    )?;
    write(
        &fixture,
        "lib/l10n/app_localizations_en.dart",
        "import 'app_localizations.dart';\nclass AppLocalizationsEn extends AppLocalizations {}\n",
    )?;

    let (_code, json) = run_json(["dart-decimate", "check", root(&fixture), "--format", "json"])?;

    assert_no_rule(&json, "dart-decimate/circular-dependency");
    assert_no_rule(&json, "dart-decimate/dead-file");
    Ok(())
}

#[test]
fn check_resolves_copied_nested_path_packages_by_owner() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        ".dart_tool/package_config.json",
        r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":"lib/"}]}"#,
    )?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    for function in ["function_a", "function_b"] {
        write(
            &fixture,
            &format!("functions/{function}/pubspec.yaml"),
            "name: function_app\ndependencies:\n  shared:\n    path: shared\n",
        )?;
        write(
            &fixture,
            &format!("functions/{function}/lib/main.dart"),
            "import 'package:shared/http.dart';\nvoid main() { sharedHttp(); }\n",
        )?;
        write(
            &fixture,
            &format!("functions/{function}/shared/pubspec.yaml"),
            "name: shared\n",
        )?;
        write(
            &fixture,
            &format!("functions/{function}/shared/lib/http.dart"),
            "String sharedHttp() {\n  final headers = <String, String>{\n    'accept': 'application/json',\n    'content-type': 'application/json',\n    'x-client': 'function-client',\n  };\n  final values = ['alpha', 'beta', 'gamma', headers.keys.join('|')];\n  final normalized = values.map((value) => value.trim().toLowerCase()).where((value) => value.isNotEmpty).toList();\n  return normalized.join(',');\n}\n",
        )?;
    }

    let (_code, json) = run_json(["dart-decimate", "check", root(&fixture), "--format", "json"])?;

    assert_no_finding_path(
        &json,
        "dart-decimate/dead-file",
        "functions/function_a/shared/lib/http.dart",
    );
    assert_no_finding_path(
        &json,
        "dart-decimate/dead-file",
        "functions/function_b/shared/lib/http.dart",
    );
    assert_no_rule(&json, "dart-decimate/code-duplication");
    Ok(())
}

#[test]
fn cycles_downgrades_typed_go_router_builder_registry_cycles()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) => const HomeRoute().go(context);
}

extension HomeRouteNavigation on HomeRoute {
  void go(BuildContext context) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "warning");
    assert!(
        findings(&json)[0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("Typed GoRouter"))
    );
    Ok(())
}

#[test]
fn cycles_downgrades_named_constructor_typed_route_extension_navigation()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute.fromId(this.id);
  final String id;
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) => const HomeRoute.fromId('home').go(context);
}

extension HomeRouteNavigation on HomeRoute {
  void go(BuildContext context) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "warning");
    Ok(())
}

#[test]
fn cycles_keeps_qualified_named_constructor_route_navigation_as_error()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute.fromId(this.id);
  final String id;
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/other_routes.dart",
        r"class HomeRoute {
  const HomeRoute.fromId(String id);
  void go(BuildContext context) {}
}

class BuildContext {}
",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';
import 'other_routes.dart' as other;

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) => const other.HomeRoute.fromId('other').go(context);
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn cycles_downgrades_generic_typed_route_extension_navigation()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) => const HomeRoute().push<void>(context);
}

extension HomeRouteNavigation on HomeRoute {
  void push<T>(BuildContext context) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "warning");
    Ok(())
}

#[test]
fn cycles_downgrades_generic_context_route_location_navigation()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
  String get location => '/';
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) => context.push<void>(const HomeRoute().location);
}

extension BuildContextNavigation on BuildContext {
  void push<T>(String location) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "warning");
    Ok(())
}

#[test]
fn cycles_keeps_disconnected_go_router_helper_references_as_errors()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  HomeRoute fallback() => const HomeRoute();
  void open(BuildContext context) => context.go('/home');
}

extension BuildContextNavigation on BuildContext {
  void go(String location) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn cycles_downgrades_route_location_on_build_context_navigation()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
  String get location => '/';
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) => context.go(const HomeRoute().location);
}

extension BuildContextNavigation on BuildContext {
  void go(String location) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "warning");
    Ok(())
}

#[test]
fn cycles_downgrades_route_location_on_typed_go_router_receiver()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
  String get location => '/';
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
class GoRouter {
  void go(String location) {}
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(GoRouter router) => router.go(const HomeRoute().location);
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "warning");
    Ok(())
}

#[test]
fn cycles_downgrades_local_route_alias_extension_navigation()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) {
    final route = const HomeRoute();
    route.go(context);
  }
}

extension HomeRouteNavigation on HomeRoute {
  void go(BuildContext context) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "warning");
    Ok(())
}

#[test]
fn cycles_downgrades_new_prefixed_route_alias_extension_navigation()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) {
    final newRoute = const HomeRoute();
    newRoute.go(context);
  }
}

extension HomeRouteNavigation on HomeRoute {
  void go(BuildContext context) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "warning");
    Ok(())
}

#[test]
fn cycles_downgrades_context_after_prior_block_local_shadow()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) {
    if (Object() == Object()) {
      final context = FakeContext();
      context.track();
    }
    const HomeRoute().go(context);
  }
}

class FakeContext {
  void track() {}
}

extension HomeRouteNavigation on HomeRoute {
  void go(BuildContext context) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "warning");
    Ok(())
}

#[test]
fn cycles_downgrades_state_context_route_navigation() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
  String get location => '/';
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class State<T> {
  BuildContext get context => BuildContext();
}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';
import 'package:flutter/widgets.dart';

class HomeScreen extends Widget {
  const HomeScreen();
}

class HomeScreenState extends State<HomeScreen> {
  void openExtension() => const HomeRoute().go(context);
  void openLocation() => context.go(const HomeRoute().location);
}

extension HomeRouteNavigation on HomeRoute {
  void go(BuildContext context) {}
}

extension BuildContextNavigation on BuildContext {
  void go(String location) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "warning");
    Ok(())
}

#[test]
fn cycles_downgrades_this_context_route_navigation() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
  String get location => '/';
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class State<T> {
  BuildContext get context => BuildContext();
}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';
import 'package:flutter/widgets.dart';

class HomeScreen extends Widget {
  const HomeScreen();
}

class HomeScreenState extends State<HomeScreen> {
  void openExtension() => const HomeRoute().go(this.context);
  void openLocation() => this.context.go(const HomeRoute().location);
}

extension HomeRouteNavigation on HomeRoute {
  void go(BuildContext context) {}
}

extension BuildContextNavigation on BuildContext {
  void go(String location) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "warning");
    Ok(())
}

#[test]
fn cycles_downgrades_member_route_alias_location_navigation()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
  String get location => '/';
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  HomeScreen();
  final route = const HomeRoute();
  void open(BuildContext context) => context.go(this.route.location);
}

extension BuildContextNavigation on BuildContext {
  void go(String location) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "warning");
    Ok(())
}

#[test]
fn cycles_downgrades_unqualified_member_route_alias_extension_navigation()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  HomeScreen();
  final route = const HomeRoute();
  void open(BuildContext context) => route.go(context);
}

extension HomeRouteNavigation on HomeRoute {
  void go(BuildContext context) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "warning");
    Ok(())
}

#[test]
fn cycles_keeps_this_reassigned_member_route_alias_navigation_as_error()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  HomeScreen();
  var route = const HomeRoute();

  void open(BuildContext context) {
    this.route = OtherRoute();
    route.go(context);
  }
}

class OtherRoute {
  void go(BuildContext context) {}
}

extension HomeRouteNavigation on HomeRoute {
  void go(BuildContext context) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn cycles_keeps_compact_constructor_prefix_navigation_as_error()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) {
    constHomeRoute().go(context);
    final route = newHomeRoute();
    route.go(context);
  }
}

class constHomeRoute {
  void go(BuildContext context) {}
}

class newHomeRoute {
  void go(BuildContext context) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn cycles_downgrades_member_route_alias_after_nested_callback_shadows()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  HomeScreen();
  final route = const HomeRoute();

  void open(BuildContext context, List<RouteWrapper> routes) {
    final labels = routes.map((route) => route.id).toList();
    final details = routes.map((item) {
      final route = item;
      return route.id;
    }).toList();
    route.go(context);
  }
}

class RouteWrapper {
  String get id => '';
}

extension HomeRouteNavigation on HomeRoute {
  void go(BuildContext context) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "warning");
    Ok(())
}

#[test]
fn cycles_keeps_header_shadowed_route_alias_navigation_as_error()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  HomeScreen();
  final route = const HomeRoute();

  void open(BuildContext context, List<RouteWrapper> routes) {
    for (final route in routes) {
      route.go(context);
    }
  }

  void recover(BuildContext context) {
    try {
      throw Object();
    } catch (route) {
      route.go(context);
    }
  }
}

class RouteWrapper {
  void go(BuildContext context) {}
}

extension HomeRouteNavigation on HomeRoute {
  void go(BuildContext context) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn cycles_keeps_local_function_shadowed_route_alias_navigation_as_error()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  HomeScreen();
  final route = const HomeRoute();

  void open(BuildContext context) {
    void route() {}
    route.go(context);
  }
}

extension HomeRouteNavigation on HomeRoute {
  void go(BuildContext context) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn cycles_keeps_reassigned_route_alias_navigation_as_error()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) {
    var route = const HomeRoute();
    route = OtherRoute();
    route.go(context);
  }
}

class OtherRoute {
  void go(BuildContext context) {}
}

extension HomeRouteNavigation on HomeRoute {
  void go(BuildContext context) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn cycles_downgrades_route_alias_after_block_local_shadow_reassignment()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) {
    final route = const HomeRoute();
    if (condition) {
      var route = OtherRoute();
      route = OtherRoute();
    }
    route.go(context);
  }
}

class OtherRoute {
  void go(BuildContext context) {}
}

extension HomeRouteNavigation on HomeRoute {
  void go(BuildContext context) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "warning");
    Ok(())
}

#[test]
fn cycles_keeps_route_location_in_extra_argument_as_error() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
  String get location => '/';
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) =>
      context.go('/other', extra: const HomeRoute().location);
}

extension BuildContextNavigation on BuildContext {
  void go(String location, {Object? extra}) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn cycles_keeps_route_location_on_non_navigation_receiver_as_error()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
  String get location => '/';
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  HomeRoute fallback() => const HomeRoute();
  void track(AnalyticsContext analyticsContext) =>
      analyticsContext.push(const HomeRoute().location);
}

class AnalyticsContext {
  void push(String location) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn cycles_keeps_route_extension_with_non_context_argument_as_error()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(Object fake) => const HomeRoute().push(fake);
}

extension HomeRouteNavigation on HomeRoute {
  void push(Object target) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn cycles_keeps_named_route_apis_as_errors() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
  String get location => '/';
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) => context.goNamed(const HomeRoute().location);
  void openRoute(BuildContext context) => const HomeRoute().goNamed(context);
}

extension BuildContextNamedNavigation on BuildContext {
  void goNamed(String name) {}
}

extension HomeRouteNamedNavigation on HomeRoute {
  void goNamed(BuildContext context) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn cycles_keeps_route_location_on_wrapper_router_receiver_as_error()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
  String get location => '/';
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open() {
    final router = RouteLocationWrapper();
    router.go(const HomeRoute().location);
    final routerWithInnerGoRouter = RouteLocationWrapper(GoRouter());
    routerWithInnerGoRouter.go(const HomeRoute().location);
    RouteLocationWrapper(GoRouter()).go(const HomeRoute().location);
  }
}

class GoRouter {}

class RouteLocationWrapper {
  RouteLocationWrapper([GoRouter? router]);
  void go(String location) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn cycles_keeps_static_go_router_wrapper_receivers_as_errors()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
  String get location => '/';
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) {
    GoRouter.wrapper.go(const HomeRoute().location);
    final router = GoRouter.wrapped();
    router.go(const HomeRoute().location);
  }
}

class GoRouter {
  static final RouteLocationWrapper wrapper = RouteLocationWrapper();
  static RouteLocationWrapper wrapped() => RouteLocationWrapper();
}

class RouteLocationWrapper {
  void go(String location) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn cycles_downgrades_go_router_of_route_location_navigation()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
  String get location => '/';
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) {
    GoRouter.of(context).go(const HomeRoute().location);
    final router = GoRouter.of(context);
    router.go(const HomeRoute().location);
  }
}

class GoRouter {
  static GoRouter of(BuildContext context) => GoRouter();
  void go(String location) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "warning");
    Ok(())
}

#[test]
fn cycles_downgrades_go_router_factory_aliases_with_null_assertions()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
  String get location => '/';
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) {
    final maybeRouter = GoRouter.maybeOf(context)!;
    maybeRouter.go(const HomeRoute().location);
    final router = GoRouter.of(context)!;
    router.go(const HomeRoute().location);
  }
}

class GoRouter {
  static GoRouter? maybeOf(BuildContext context) => GoRouter();
  static GoRouter? of(BuildContext context) => GoRouter();
  void go(String location) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "warning");
    Ok(())
}

#[test]
fn cycles_downgrades_prefixed_go_router_import_factory_navigation()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
  String get location => '/';
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';
import 'package:go_router/go_router.dart' as go;

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) {
    go.GoRouter.of(context).go(const HomeRoute().location);
    final router = go.GoRouter.maybeOf(context)!;
    router.go(const HomeRoute().location);
  }
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "warning");
    Ok(())
}

#[test]
fn cycles_keeps_unresolved_prefixed_go_router_factory_receiver_as_error()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
  String get location => '/';
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) {
    go.GoRouter.of(context).go(const HomeRoute().location);
  }
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn cycles_keeps_shadowed_go_router_factory_receiver_as_error()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
  String get location => '/';
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) {
    final GoRouter = RouteLocationWrapperFactory();
    GoRouter.of(context).go(const HomeRoute().location);
  }
}

class RouteLocationWrapperFactory {
  RouteLocationWrapper of(BuildContext context) => RouteLocationWrapper();
}

class RouteLocationWrapper {
  void go(String location) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn cycles_keeps_prefixed_fake_navigation_types_as_error() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
  String get location => '/';
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/fake_navigation.dart",
        r"class BuildContext {}

class GoRouter {
  void go(String location) {}
}
",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';
import 'package:app/features/home/fake_navigation.dart' as fake;

class HomeScreen extends Widget {
  const HomeScreen();
  void openWithFakeContext(fake.BuildContext context) => const HomeRoute().go(context);
  void openWithFakeRouter(fake.GoRouter router) => router.go(const HomeRoute().location);
}

extension HomeRouteNavigation on HomeRoute {
  void go(Object context) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn cycles_downgrades_resolved_prefixed_navigation_types() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
  String get location => '/';
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';
import 'package:flutter/widgets.dart' as f;
import 'package:go_router/go_router.dart' as go;

class HomeScreen extends Widget {
  const HomeScreen();
  void open(f.BuildContext context, go.GoRouter router) {
    const HomeRoute().go(context);
    router.go(const HomeRoute().location);
  }
}

extension HomeRouteNavigation on HomeRoute {
  void go(Object context) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "warning");
    Ok(())
}

#[test]
fn cycles_keeps_unresolved_go_router_factory_receiver_as_error()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
  String get location => '/';
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) {
    GoRouter.of(context).go(const HomeRoute().location);
  }
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn cycles_keeps_wrapped_route_location_navigation_as_error()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
  String get location => '/';
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) =>
      context.go(RouteLocationWrapper(const HomeRoute()).location);
}

class RouteLocationWrapper {
  const RouteLocationWrapper(this.route);
  final HomeRoute route;
  String get location => route.location;
}

extension BuildContextNavigation on BuildContext {
  void go(String location) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn cycles_keeps_mixed_go_router_sccs_as_errors() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';
import 'package:app/features/home/home_helpers.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) => const HomeRoute().go(context);
  HomeHelpers helpers() => HomeHelpers();
}

extension HomeRouteNavigation on HomeRoute {
  void go(BuildContext context) {}
}
",
    )?;
    write(
        &fixture,
        "lib/features/home/home_helpers.dart",
        "import 'package:app/features/home/home_cycle.dart';\nclass HomeHelpers {}\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_cycle.dart",
        "import 'package:app/features/home/home_screen.dart';\nclass HomeCycle {}\n",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn cycles_keeps_plain_go_router_registry_import_cycles_as_errors()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/data/route_repository.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  RouteRepository repository() => RouteRepository();
}

class GoRouteData {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/data/route_repository.dart",
        r"import 'package:app/core/router/app_routes.dart';

class RouteRepository {
  HomeRoute fallback() => const HomeRoute();
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn cycles_keeps_route_registry_sccs_with_internal_exports_as_errors()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_screen.dart';
export 'package:app/features/home/home_screen.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => const HomeScreen();
}

class BuildContext {}
class GoRouterState {}
class GoRouteData {}
class Widget {}
class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
",
    )?;
    write(
        &fixture,
        "lib/core/router/app_routes.g.dart",
        "part of 'app_routes.dart';\n",
    )?;
    write(
        &fixture,
        "lib/features/home/home_screen.dart",
        r"import 'package:app/core/router/app_routes.dart';

class HomeScreen extends Widget {
  const HomeScreen();
  void open(BuildContext context) => const HomeRoute().go(context);
}

extension HomeRouteNavigation on HomeRoute {
  void go(BuildContext context) {}
}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_finding_severity(&json, "dart-decimate/circular-dependency", "error");
    Ok(())
}

#[test]
fn check_treats_scripts_as_dynamic_entry_points() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(
        &fixture,
        "scripts/check_something.dart",
        "void main() { print('checked'); }\n",
    )?;

    let (_code, json) = run_json(["dart-decimate", "check", root(&fixture), "--format", "json"])?;

    assert_no_finding_path(
        &json,
        "dart-decimate/dead-file",
        "scripts/check_something.dart",
    );
    Ok(())
}

#[test]
fn check_counts_flutter_tool_config_dependencies_as_used() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndev_dependencies:\n  flutter_launcher_icons: ^0.14.0\n",
    )?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(
        &fixture,
        "flutter_launcher_icons.yaml",
        "flutter_launcher_icons:\n  android: launcher_icon\n  ios: true\n",
    )?;

    let (_code, json) = run_json(["dart-decimate", "check", root(&fixture), "--format", "json"])?;

    assert_no_dependency(&json, "flutter_launcher_icons");
    Ok(())
}

#[test]
fn check_counts_private_members_used_in_string_interpolation()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "abstract final class Keys {\n  static const String _prefix = 'item:';\n  static const String _suffix = ':done';\n  static const String _chain = 'chain';\n  static const String _escaped = 'escaped';\n  static const String _raw = 'raw';\n  static String item(String id) => '$_prefix$id ${_suffix.toUpperCase()} ${Keys._chain} \\$_escaped';\n  static String raw() => r'$_raw';\n}\nvoid main() { Keys.item('42'); Keys.raw(); }\n",
    )?;

    let (_code, json) = run_json(["dart-decimate", "check", root(&fixture), "--format", "json"])?;

    assert_no_symbol(&json, "dart-decimate/unused-class-member", "Keys._prefix");
    assert_no_symbol(&json, "dart-decimate/unused-class-member", "Keys._suffix");
    assert_no_symbol(&json, "dart-decimate/unused-class-member", "Keys._chain");
    assert_symbol(&json, "dart-decimate/unused-class-member", "Keys._escaped");
    assert_symbol(&json, "dart-decimate/unused-class-member", "Keys._raw");
    Ok(())
}

#[test]
fn security_reports_firebase_api_keys_as_warning_candidates()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "const options = FirebaseOptions(apiKey: 'DartDecimateFirebaseKeyValue123456789');\n",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "security",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_finding_severity(&json, "dart-decimate/security-firebase-api-key", "warning");
    Ok(())
}

#[test]
fn security_rule_promotes_firebase_candidate_detail_to_error()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".dart-decimaterc",
        "[rules]\n\"dart-decimate/security-firebase-api-key\" = \"error\"\n",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "const options = FirebaseOptions(apiKey: 'DartDecimateFirebaseKeyValue123456789');\n",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "security",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_finding_severity(&json, "dart-decimate/security-firebase-api-key", "error");
    assert_eq!(json["security_candidates"][0]["severity"], "error");
    Ok(())
}

#[test]
fn check_downgrades_dev_e2e_environment_defines() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(
        &fixture,
        "test/e2e_flags_test.dart",
        "const disablePushForE2e = bool.fromEnvironment('E2E_DISABLE_PUSH');\nvoid main() { print(disablePushForE2e); }\n",
    )?;

    let (code, json) = run_json(["dart-decimate", "check", root(&fixture), "--format", "json"])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_finding_severity(&json, "dart-decimate/feature-flag", "warning");
    Ok(())
}

#[test]
fn check_keeps_production_e2e_named_sdk_flags_as_errors() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "bool enabled() => FirebaseRemoteConfig.instance.getBool('APP_E2E_REMOTE_FLAG');\nvoid main() { print(enabled()); }\n",
    )?;

    let (code, json) = run_json(["dart-decimate", "check", root(&fixture), "--format", "json"])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_finding_severity(&json, "dart-decimate/feature-flag", "error");
    Ok(())
}

#[test]
fn check_keeps_bare_iife_type_reference_as_unrendered() -> Result<(), Box<dyn std::error::Error>> {
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
        "import 'widgets.dart';\nvoid main() {}\n",
    )?;
    write(
        &fixture,
        "lib/widgets.dart",
        r"
class DeadCard extends StatelessWidget {
  const DeadCard({super.key});
  Widget build(BuildContext context) => const SizedBox();
}

Type marker() => (() => DeadCard)();
",
    )?;

    let (_code, json) = run_json(["dart-decimate", "check", root(&fixture), "--format", "json"])?;

    assert_symbol(&json, "dart-decimate/unrendered-widget", "DeadCard");
    Ok(())
}

fn run_json<const N: usize>(args: [&str; N]) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(args, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    Ok((code, json))
}

fn root(fixture: &TempDir) -> &str {
    fixture.path().to_str().unwrap_or(".")
}

fn assert_no_rule(json: &Value, rule_id: &str) {
    assert!(
        !findings(json)
            .iter()
            .any(|finding| finding["rule_id"] == rule_id),
        "{rule_id} should not be reported: {:?}",
        findings(json)
    );
}

fn assert_no_finding_path(json: &Value, rule_id: &str, path: &str) {
    assert!(
        !findings(json)
            .iter()
            .any(|finding| finding["rule_id"] == rule_id && finding["path"] == path),
        "{rule_id} should not be reported for {path}: {:?}",
        findings(json)
    );
}

fn assert_no_dependency(json: &Value, dependency: &str) {
    assert!(
        !findings(json)
            .iter()
            .any(|finding| finding["actions"][0]["target_dependency"] == dependency),
        "{dependency} should not be reported unused: {:?}",
        findings(json)
    );
}

fn assert_no_symbol(json: &Value, rule_id: &str, symbol: &str) {
    assert!(
        !findings(json)
            .iter()
            .any(|finding| finding["rule_id"] == rule_id
                && finding["actions"][0]["target_symbol"] == symbol),
        "{symbol} should not be reported for {rule_id}: {:?}",
        findings(json)
    );
}

fn assert_symbol(json: &Value, rule_id: &str, symbol: &str) {
    assert!(
        findings(json)
            .iter()
            .any(|finding| finding["rule_id"] == rule_id
                && finding["actions"][0]["target_symbol"] == symbol),
        "{symbol} should be reported for {rule_id}: {:?}",
        findings(json)
    );
}

fn assert_finding_severity(json: &Value, rule_id: &str, severity: &str) {
    let Some(finding) = findings(json)
        .iter()
        .find(|finding| finding["rule_id"] == rule_id)
    else {
        panic!("{rule_id} finding missing: {:?}", findings(json));
    };
    assert_eq!(finding["severity"], severity);
}

fn findings(json: &Value) -> &[Value] {
    json["findings"].as_array().map_or(&[], Vec::as_slice)
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
