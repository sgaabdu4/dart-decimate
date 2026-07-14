use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn ignored_generated_runtime_import_prevents_test_only_dependency()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, ".gitignore", "**.g.dart\n")?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  runtime_core: any\n",
    )?;
    write(
        &fixture,
        "lib/bootstrap.dart",
        "import 'runtime_registrar.g.dart';\nvoid bootstrap() => registerGeneratedTypes();\n",
    )?;
    write(
        &fixture,
        "lib/runtime_registrar.g.dart",
        "import 'package:runtime_core/runtime_core.dart';\nvoid registerGeneratedTypes() {}\n",
    )?;
    write(
        &fixture,
        "test/bootstrap_test.dart",
        "import 'package:runtime_core/runtime_core.dart';\nvoid main() {}\n",
    )?;

    let (code, json) = run_json(["dart-decimate", "check", root(&fixture), "--format", "json"])?;

    assert_eq!(code, 0);
    assert_no_rule(&json, "dart-decimate/test-only-dependency");
    assert_no_rule(&json, "dart-decimate/unused-dependency");
    Ok(())
}

#[test]
fn ignored_generated_import_reachable_only_from_tests_stays_test_only()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, ".gitignore", "**.g.dart\n")?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  runtime_core: any\n",
    )?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(
        &fixture,
        "lib/test_registrar.g.dart",
        "import 'package:runtime_core/runtime_core.dart';\nvoid registerTests() {}\n",
    )?;
    write(
        &fixture,
        "test/app_test.dart",
        "import '../lib/test_registrar.g.dart';\nimport 'package:runtime_core/runtime_core.dart';\nvoid main() => registerTests();\n",
    )?;

    let (code, json) = run_json(["dart-decimate", "check", root(&fixture), "--format", "json"])?;

    assert_eq!(code, 1);
    assert_rule(&json, "dart-decimate/test-only-dependency");
    Ok(())
}

#[test]
fn typed_route_navigation_resolves_captured_context() -> Result<(), Box<dyn std::error::Error>> {
    assert_typed_route_warning(
        r"void open(BuildContext context) {
  onPressed(() => const HomeRoute().push<void>(context));
}

void onPressed(void Function() callback) {}
",
    )
}

#[test]
fn typed_route_navigation_resolves_context_alias() -> Result<(), Box<dyn std::error::Error>> {
    assert_typed_route_warning(
        r"void open(BuildContext context) {
  final navigationContext = context;
  const HomeRoute().push<void>(navigationContext);
}
",
    )
}

#[test]
fn typed_route_navigation_resolves_destructured_context_field()
-> Result<(), Box<dyn std::error::Error>> {
    assert_typed_route_warning(
        r"class Request {
  const Request(this.context);
  final BuildContext context;
}

void handle(Request request) {
  final Request(:context) = request;
  const HomeRoute().go(context);
}
",
    )
}

#[test]
fn typed_route_navigation_resolves_typed_context_field() -> Result<(), Box<dyn std::error::Error>> {
    assert_typed_route_warning(
        r"class Request {
  const Request(this.context);
  final BuildContext context;
}

void handle(Request request) {
  const HomeRoute().go(request.context);
}
",
    )
}

#[test]
fn typed_route_navigation_resolves_navigator_context_alias()
-> Result<(), Box<dyn std::error::Error>> {
    assert_typed_route_warning(
        r"void open(BuildContext context) {
  final navigationContext = Navigator.of(context, rootNavigator: true).context;
  const HomeRoute().push<void>(navigationContext);
}
",
    )
}

fn assert_typed_route_warning(helper_body: &str) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/core/router/app_routes.dart",
        r"import 'package:app/features/home/home_helper.dart';

part 'app_routes.g.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {
  const HomeRoute();
  Widget build(BuildContext context, GoRouterState state) => HomeScreen();
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
        "lib/features/home/home_helper.dart",
        &format!(
            "import 'package:app/core/router/app_routes.dart';\nimport 'package:flutter/widgets.dart';\n\nclass HomeScreen extends Widget {{}}\n\n{helper_body}\n\nextension HomeRouteNavigation on HomeRoute {{\n  void go(BuildContext context) {{}}\n  void push<T>(BuildContext context) {{}}\n}}\n"
        ),
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "cycles",
        root(&fixture),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0, "{json:#}");
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["cycles"], 1);
    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "dart-decimate/circular-dependency")
    }) else {
        panic!("circular dependency finding");
    };
    assert_eq!(finding["severity"], "warning");
    Ok(())
}

fn run_json<const N: usize>(args: [&str; N]) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(args, &mut output)?;
    Ok((code, serde_json::from_slice(&output)?))
}

fn assert_no_rule(json: &Value, rule_id: &str) {
    assert!(
        json["findings"]
            .as_array()
            .is_none_or(|findings| findings.iter().all(|finding| finding["rule_id"] != rule_id)),
        "unexpected {rule_id} finding"
    );
}

fn assert_rule(json: &Value, rule_id: &str) {
    assert!(
        json["findings"]
            .as_array()
            .is_some_and(|findings| findings.iter().any(|finding| finding["rule_id"] == rule_id)),
        "missing {rule_id} finding"
    );
}

fn root(fixture: &TempDir) -> &str {
    fixture.path().to_str().unwrap_or(".")
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
