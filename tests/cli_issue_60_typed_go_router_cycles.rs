use std::{fs, process::Command};

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

const MIXED_ROUTING_RULE: &str = "dart-decimate/mixed-go-router-style";

#[test]
fn check_reports_raw_context_navigation_after_typed_route_adoption()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n  go_router: any\n",
    )?;
    write(
        &fixture,
        "lib/routes.dart",
        r"import 'package:go_router/go_router.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {}
",
    )?;
    write(
        &fixture,
        "lib/home_screen.dart",
        r"import 'package:flutter/widgets.dart';
import 'package:go_router/go_router.dart';

void openHome(BuildContext context) {
  context.go('/home');
}
",
    )?;

    let (code, json) = run_json(&fixture, "check")?;

    assert_eq!(code, 1);
    let finding = finding(&json, MIXED_ROUTING_RULE);
    assert_eq!(finding["kind"], "mixed-go-router-style");
    assert_eq!(finding["severity"], "error");
    assert_eq!(finding["path"], "lib/home_screen.dart");
    assert_eq!(finding["line"], 5);
    assert_eq!(finding["actions"][0]["action"], "use-typed-go-router");
    assert_eq!(
        finding["actions"][0]["suppression_comment"],
        "// dart-decimate-ignore-next-line mixed-go-router-style -- <reason>"
    );
    Ok(())
}

#[test]
fn check_accepts_generated_route_object_navigation() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n  go_router: any\n",
    )?;
    write(
        &fixture,
        "lib/routes.dart",
        r"import 'package:go_router/go_router.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {}
",
    )?;
    write(
        &fixture,
        "lib/home_screen.dart",
        r"import 'package:flutter/widgets.dart';
import 'routes.dart';

void openHome(BuildContext context) {
  const HomeRoute().go(context);
}
",
    )?;

    let (_code, json) = run_json(&fixture, "check")?;

    assert_no_finding(&json, MIXED_ROUTING_RULE);
    Ok(())
}

#[test]
fn check_accepts_raw_navigation_before_typed_route_adoption()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n  go_router: any\n",
    )?;
    write(
        &fixture,
        "lib/router.dart",
        r"import 'package:flutter/widgets.dart';
import 'package:go_router/go_router.dart';

final router = GoRouter(
  routes: [
    GoRoute(path: '/', builder: (_, _) => const SizedBox()),
  ],
);

void openHome(BuildContext context) {
  context.go('/');
}
",
    )?;

    let (_code, json) = run_json(&fixture, "check")?;

    assert_no_finding(&json, MIXED_ROUTING_RULE);
    Ok(())
}

#[test]
fn check_ignores_shadowed_build_context_navigation() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  go_router: any\n",
    )?;
    write(
        &fixture,
        "lib/routes.dart",
        r"import 'package:go_router/go_router.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {}
",
    )?;
    write(
        &fixture,
        "lib/analytics.dart",
        r"import 'package:go_router/go_router.dart';

class BuildContext {
  void go(String event) {}
}

void record(BuildContext context) {
  context.go('opened-home');
}
",
    )?;

    let (_code, json) = run_json(&fixture, "check")?;

    assert_no_finding(&json, MIXED_ROUTING_RULE);
    Ok(())
}

#[test]
fn check_reports_raw_go_route_after_typed_route_adoption() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  go_router: any\n",
    )?;
    write(
        &fixture,
        "lib/typed_routes.dart",
        r"import 'package:go_router/go_router.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {}
",
    )?;
    write(
        &fixture,
        "lib/legacy_routes.dart",
        r"import 'package:go_router/go_router.dart';

final legacyRoute = GoRoute(
  path: '/legacy',
  builder: (_, _) => throw UnimplementedError(),
);
",
    )?;

    let (code, json) = run_json(&fixture, "check")?;

    assert_eq!(code, 1);
    let finding = finding(&json, MIXED_ROUTING_RULE);
    assert_eq!(finding["path"], "lib/legacy_routes.dart");
    assert_eq!(finding["line"], 3);
    assert!(
        finding["message"]
            .as_str()
            .is_some_and(|message| message.contains("raw `GoRoute`"))
    );
    Ok(())
}

#[test]
fn check_reports_every_resolved_raw_destination_api_after_typed_adoption()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = typed_fixture()?;
    write(
        &fixture,
        "lib/navigation.dart",
        r"import 'package:flutter/widgets.dart';
import 'package:go_router/go_router.dart';

void navigate(BuildContext context, GoRouter router) {
  context.go('/go');
  context.push<int>('/push');
  context.pushReplacement('/push-replacement');
  context.replace('/replace');
  context.goNamed('go-named');
  context.pushNamed('push-named');
  context.pushReplacementNamed('push-replacement-named');
  context.replaceNamed('replace-named');
  router.namedLocation('home');
  GoRouter.of(context).go('/factory');
  final alias = GoRouter.of(context);
  alias.push('/alias');
  final constructed = GoRouter(routes: const []);
  constructed.replace('/constructed');
  GoRouter? optional;
  optional?.push('/optional');
}
",
    )?;

    let (code, json) = run_json(&fixture, "check")?;

    assert_eq!(code, 1);
    let findings = mixed_findings(&json);
    assert_eq!(findings.len(), 13, "{findings:?}");
    assert_eq!(json["summary"]["mixed_go_router_styles"], 13);
    for (target, count) in [
        ("go", 2),
        ("push", 3),
        ("pushReplacement", 1),
        ("replace", 2),
        ("goNamed", 1),
        ("pushNamed", 1),
        ("pushReplacementNamed", 1),
        ("replaceNamed", 1),
        ("namedLocation", 1),
    ] {
        assert_target_count(&findings, target, count);
    }
    Ok(())
}

#[test]
fn check_accepts_all_generated_route_object_navigation_helpers()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = typed_fixture()?;
    write(
        &fixture,
        "lib/navigation.dart",
        r"import 'package:flutter/widgets.dart';
import 'package:go_router/go_router.dart';
import 'routes.dart';

void navigate(BuildContext context) {
  const HomeRoute().go(context);
  const HomeRoute().push<int>(context);
  const HomeRoute().pushReplacement(context);
  const HomeRoute().replace(context);
  const HomeRoute().goRelative(context);
  const HomeRoute().pushRelative(context);
  const HomeRoute().pushReplacementRelative(context);
  const HomeRoute().replaceRelative(context);
  final location = const HomeRoute().location;
}
",
    )?;

    let (_code, json) = run_json(&fixture, "check")?;

    assert_no_finding(&json, MIXED_ROUTING_RULE);
    Ok(())
}

#[test]
fn check_reports_prefixed_imported_go_route_after_typed_adoption()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = typed_fixture()?;
    write(
        &fixture,
        "lib/legacy_routes.dart",
        r"import 'package:go_router/go_router.dart' as go;

final legacyRoute = go.GoRoute(
  path: '/legacy',
  builder: (_, _) => throw UnimplementedError(),
);
",
    )?;

    let (code, json) = run_json(&fixture, "check")?;

    assert_eq!(code, 1);
    let finding = finding(&json, MIXED_ROUTING_RULE);
    assert_eq!(finding["actions"][0]["target_symbol"], "GoRoute");
    Ok(())
}

#[test]
fn check_ignores_locally_shadowed_go_route_and_go_router_types()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = typed_fixture()?;
    write(
        &fixture,
        "lib/custom_router.dart",
        r"import 'package:go_router/go_router.dart' as go;

class GoRoute {
  const GoRoute({required String path});
}

class GoRouter {
  void push(String event) {}
}

final customRoute = GoRoute(path: '/analytics');

void record(GoRouter router) {
  router.push('opened-home');
}
",
    )?;

    let (_code, json) = run_json(&fixture, "check")?;

    assert_no_finding(&json, MIXED_ROUTING_RULE);
    Ok(())
}

#[test]
fn check_ignores_navigator_pop_deep_links_and_custom_navigation_methods()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = typed_fixture()?;
    write(
        &fixture,
        "lib/deep_links.dart",
        r"import 'package:flutter/widgets.dart';
import 'package:go_router/go_router.dart' as go;

class AnalyticsContext {
  void go(String event) {}
  void pushNamed(String event) {}
}

void handle(
  BuildContext context,
  Uri incoming,
  AnalyticsContext analytics,
) {
  Navigator.of(context).pushNamed('/modal');
  context.pop();
  final deepLink = incoming.toString();
  analytics.go('opened-home');
  analytics.pushNamed('opened-settings');
}
",
    )?;

    let (_code, json) = run_json(&fixture, "check")?;

    assert_no_finding(&json, MIXED_ROUTING_RULE);
    Ok(())
}

#[test]
fn check_ignores_raw_navigation_in_generated_and_test_files()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = typed_fixture()?;
    write(
        &fixture,
        "lib/legacy.g.dart",
        r"import 'package:flutter/widgets.dart';
import 'package:go_router/go_router.dart';
void generated(BuildContext context) => context.go('/generated');
",
    )?;
    write(
        &fixture,
        "test/legacy_test.dart",
        r"import 'package:flutter/widgets.dart';
import 'package:go_router/go_router.dart';
void fixture(BuildContext context) => context.go('/fixture');
final route = GoRoute(path: '/fixture', builder: (_, _) => throw UnimplementedError());
",
    )?;

    let (_code, json) = run_json(&fixture, "check")?;

    assert_no_finding(&json, MIXED_ROUTING_RULE);
    Ok(())
}

#[test]
fn check_does_not_activate_from_typed_routes_declared_only_in_tests()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n  go_router: any\n",
    )?;
    write(
        &fixture,
        "lib/router.dart",
        r"import 'package:flutter/widgets.dart';
import 'package:go_router/go_router.dart';
final route = GoRoute(path: '/', builder: (_, _) => throw UnimplementedError());
void open(BuildContext context) => context.go('/');
",
    )?;
    write(
        &fixture,
        "test/typed_route_test.dart",
        r"import 'package:go_router/go_router.dart';
@TypedGoRoute<TestRoute>(path: '/test')
class TestRoute extends GoRouteData {}
",
    )?;

    let (_code, json) = run_json(&fixture, "check")?;

    assert_no_finding(&json, MIXED_ROUTING_RULE);
    Ok(())
}

#[test]
fn check_does_not_activate_from_same_named_local_typed_route_api()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n  go_router: any\n",
    )?;
    write(
        &fixture,
        "lib/fake_typed_routes.dart",
        r"class TypedGoRoute<T> {
  const TypedGoRoute({required String path});
}
class GoRouteData {}
@TypedGoRoute<FakeRoute>(path: '/fake')
class FakeRoute extends GoRouteData {}
",
    )?;
    write(
        &fixture,
        "lib/router.dart",
        r"import 'package:flutter/widgets.dart';
import 'package:go_router/go_router.dart';
void open(BuildContext context) => context.go('/');
",
    )?;

    let (_code, json) = run_json(&fixture, "check")?;

    assert_no_finding(&json, MIXED_ROUTING_RULE);
    Ok(())
}

#[test]
fn check_activates_from_prefixed_imported_typed_route_api() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n  go_router: any\n",
    )?;
    write(
        &fixture,
        "lib/routes.dart",
        r"import 'package:go_router/go_router.dart' as go;
@go.TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends go.GoRouteData {}
",
    )?;
    write(
        &fixture,
        "lib/router.dart",
        r"import 'package:flutter/widgets.dart';
import 'package:go_router/go_router.dart';
void open(BuildContext context) => context.go('/');
",
    )?;

    let (code, json) = run_json(&fixture, "check")?;

    assert_eq!(code, 1);
    assert_eq!(
        finding(&json, MIXED_ROUTING_RULE)["path"],
        "lib/router.dart"
    );
    Ok(())
}

#[test]
fn check_activates_from_imported_typed_shell_route_api() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n  go_router: any\n",
    )?;
    write(
        &fixture,
        "lib/routes.dart",
        r"import 'package:go_router/go_router.dart';
@TypedShellRoute<AppShell>(routes: [])
class AppShell extends ShellRouteData {}
",
    )?;
    write(
        &fixture,
        "lib/router.dart",
        r"import 'package:flutter/widgets.dart';
import 'package:go_router/go_router.dart';
void open(BuildContext context) => context.go('/');
",
    )?;

    let (code, json) = run_json(&fixture, "check")?;

    assert_eq!(code, 1);
    assert_eq!(
        finding(&json, MIXED_ROUTING_RULE)["path"],
        "lib/router.dart"
    );
    Ok(())
}

#[test]
fn check_respects_go_router_import_combinators_for_shadowed_types()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = typed_fixture()?;
    write(
        &fixture,
        "lib/custom_router.dart",
        r"class GoRoute {
  const GoRoute({required String path});
}
class GoRouter {
  void push(String event) {}
}
",
    )?;
    write(
        &fixture,
        "lib/legacy.dart",
        r"import 'package:go_router/go_router.dart' hide GoRoute, GoRouter;
import 'custom_router.dart';

final customRoute = GoRoute(path: '/custom');
void record(GoRouter router) => router.push('opened-home');
",
    )?;

    let (_code, json) = run_json(&fixture, "check")?;

    assert_no_finding(&json, MIXED_ROUTING_RULE);
    Ok(())
}

#[test]
fn check_distinguishes_prefixed_official_and_unprefixed_custom_go_router_types()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = typed_fixture()?;
    write(
        &fixture,
        "lib/custom_router.dart",
        r"class GoRouter {
  void push(String event) {}
}
",
    )?;
    write(
        &fixture,
        "lib/custom_navigation.dart",
        r"import 'package:go_router/go_router.dart' as go;
import 'custom_router.dart';

void record(GoRouter router) => router.push('opened-home');
",
    )?;
    write(
        &fixture,
        "lib/official_navigation.dart",
        r"import 'package:go_router/go_router.dart' as go;

void navigate(go.GoRouter router) => router.push('/home');
",
    )?;

    let (code, json) = run_json(&fixture, "check")?;

    assert_eq!(code, 1);
    let findings = mixed_findings(&json);
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0]["path"], "lib/official_navigation.dart");
    assert_eq!(findings[0]["actions"][0]["target_symbol"], "push");
    Ok(())
}

#[test]
fn check_reports_prefixed_go_router_constructor_alias_navigation()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = typed_fixture()?;
    write(
        &fixture,
        "lib/navigation.dart",
        r"import 'package:go_router/go_router.dart' as go;

final router = go.GoRouter(routes: const []);
final inheritedRouter = go.GoRouter.maybeOf(null)!;

void navigate() => router.push('/home');
void navigateWithInheritedRouter() => inheritedRouter.replace('/settings');
void navigateWithDirectRouter() => new go.GoRouter(routes: const []).go('/direct');
",
    )?;

    let (code, json) = run_json(&fixture, "check")?;

    assert_eq!(code, 1);
    let findings = mixed_findings(&json);
    assert_eq!(findings.len(), 3, "{findings:?}");
    assert!(findings.iter().all(|finding| {
        finding["path"] == "lib/navigation.dart"
            && matches!(
                finding["actions"][0]["target_symbol"].as_str(),
                Some("go" | "push" | "replace")
            )
    }));
    Ok(())
}

#[test]
fn check_respects_flutter_import_combinators_for_shadowed_build_context()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = typed_fixture()?;
    write(
        &fixture,
        "lib/custom_context.dart",
        r"class BuildContext {
  void go(String event) {}
}
",
    )?;
    write(
        &fixture,
        "lib/analytics.dart",
        r"import 'package:flutter/widgets.dart' hide BuildContext;
import 'package:go_router/go_router.dart';
import 'custom_context.dart';

void record(BuildContext context) => context.go('opened-home');
",
    )?;

    let (_code, json) = run_json(&fixture, "check")?;

    assert_no_finding(&json, MIXED_ROUTING_RULE);
    Ok(())
}

#[test]
fn check_reports_literal_raw_redirect_destinations_after_typed_adoption()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n  go_router: any\n",
    )?;
    write(
        &fixture,
        "lib/routes.dart",
        r"import 'package:flutter/widgets.dart';
import 'package:go_router/go_router.dart';

@TypedGoRoute<GuardRoute>(path: '/guard')
class GuardRoute extends GoRouteData {
  String? redirect(BuildContext context, GoRouterState state) => '/login';
}

@TypedGoRoute<BlockGuardRoute>(path: '/block')
class BlockGuardRoute extends GoRouteData {
  String? redirect(BuildContext context, GoRouterState state) {
    return '/block-login';
  }
}

@TypedGoRoute<ConditionalGuardRoute>(path: '/conditional')
class ConditionalGuardRoute extends GoRouteData {
  String? redirect(BuildContext context, GoRouterState state) =>
      state.matchedLocation == '/conditional' ? '/conditional-login' : null;
}

final router = GoRouter(
  routes: $appRoutes,
  redirect: (context, state) => '/splash',
);
",
    )?;

    let (code, json) = run_json(&fixture, "check")?;

    assert_eq!(code, 1);
    let findings = mixed_findings(&json);
    assert_eq!(findings.len(), 4, "{findings:?}");
    assert_target_count(&findings, "redirect", 4);
    Ok(())
}

#[test]
fn check_accepts_generated_route_locations_in_redirects() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n  go_router: any\n",
    )?;
    write(
        &fixture,
        "lib/routes.dart",
        r"import 'package:flutter/widgets.dart';
import 'package:go_router/go_router.dart';

@TypedGoRoute<GuardRoute>(path: '/guard')
class GuardRoute extends GoRouteData {
  String? redirect(BuildContext context, GoRouterState state) =>
      const LoginRoute().location;
}

@TypedGoRoute<ConditionalGuardRoute>(path: '/conditional')
class ConditionalGuardRoute extends GoRouteData {
  String? redirect(BuildContext context, GoRouterState state) =>
      state.matchedLocation == '/conditional'
          ? const LoginRoute().location
          : null;
}

@TypedGoRoute<LoginRoute>(path: '/login')
class LoginRoute extends GoRouteData {}

final router = GoRouter(
  routes: $appRoutes,
  redirect: (context, state) => const LoginRoute().location,
);
",
    )?;

    let (_code, json) = run_json(&fixture, "check")?;

    assert_no_finding(&json, MIXED_ROUTING_RULE);
    Ok(())
}

#[test]
fn check_honors_reasoned_line_suppression_for_one_mixed_navigation()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = typed_fixture()?;
    write(
        &fixture,
        "lib/navigation.dart",
        r"import 'package:flutter/widgets.dart';
import 'package:go_router/go_router.dart';

void open(BuildContext context) {
  // dart-decimate-ignore-next-line mixed-go-router-style -- legacy URL migration
  context.go('/legacy');
}
",
    )?;

    let (_code, json) = run_json(&fixture, "check")?;

    assert_no_finding(&json, MIXED_ROUTING_RULE);
    Ok(())
}

#[test]
fn audit_reports_new_raw_navigation_against_existing_typed_routes()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = typed_fixture()?;
    git_commit_all(&fixture)?;
    write(
        &fixture,
        "lib/navigation.dart",
        r"import 'package:flutter/widgets.dart';
import 'package:go_router/go_router.dart';
void open(BuildContext context) => context.go('/legacy');
",
    )?;

    let (code, json) = run_audit_json(&fixture)?;

    assert_eq!(code, 1);
    assert_eq!(json["command"], "audit");
    assert_eq!(
        finding(&json, MIXED_ROUTING_RULE)["path"],
        "lib/navigation.dart"
    );
    Ok(())
}

#[test]
fn check_mixed_fingerprint_is_independent_of_checkout_path()
-> Result<(), Box<dyn std::error::Error>> {
    let first = typed_fixture()?;
    let second = typed_fixture()?;
    for fixture in [&first, &second] {
        write(
            fixture,
            "lib/navigation.dart",
            r"import 'package:flutter/widgets.dart';
import 'package:go_router/go_router.dart';
void open(BuildContext context) => context.go('/legacy');
",
        )?;
    }

    let (_, first_json) = run_json(&first, "check")?;
    let (_, second_json) = run_json(&second, "check")?;

    assert_eq!(
        finding(&first_json, MIXED_ROUTING_RULE)["fingerprint"],
        finding(&second_json, MIXED_ROUTING_RULE)["fingerprint"]
    );
    Ok(())
}

fn typed_fixture() -> Result<TempDir, Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n  go_router: any\n",
    )?;
    write(
        &fixture,
        "lib/routes.dart",
        r"import 'package:go_router/go_router.dart';

@TypedGoRoute<HomeRoute>(path: '/')
class HomeRoute extends GoRouteData {}
",
    )?;
    Ok(fixture)
}

fn run_json(fixture: &TempDir, command: &str) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(
        [
            "dart-decimate",
            command,
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;
    Ok((code, serde_json::from_slice(&output)?))
}

fn run_audit_json(fixture: &TempDir) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(
        [
            "dart-decimate",
            "audit",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--base",
            "HEAD",
        ],
        &mut output,
    )?;
    Ok((code, serde_json::from_slice(&output)?))
}

fn mixed_findings(json: &Value) -> Vec<&Value> {
    json["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|finding| finding["rule_id"] == MIXED_ROUTING_RULE)
        .collect()
}

fn assert_target_count(findings: &[&Value], target: &str, expected: usize) {
    assert_eq!(
        findings
            .iter()
            .filter(|finding| finding["actions"][0]["target_symbol"] == target)
            .count(),
        expected,
        "unexpected {target} findings: {findings:?}"
    );
}

fn finding<'json>(json: &'json Value, rule_id: &str) -> &'json Value {
    json["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|finding| finding["rule_id"] == rule_id)
        .unwrap_or_else(|| panic!("{rule_id} finding missing: {:?}", json["findings"]))
}

fn assert_no_finding(json: &Value, rule_id: &str) {
    assert!(
        !json["findings"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|finding| finding["rule_id"] == rule_id),
        "{rule_id} should not be reported: {:?}",
        json["findings"]
    );
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}

fn git_commit_all(fixture: &TempDir) -> Result<(), Box<dyn std::error::Error>> {
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "tests@example.com"],
        vec!["config", "user.name", "Tests"],
        vec!["config", "commit.gpgsign", "false"],
        vec!["add", "."],
        vec!["commit", "-q", "-m", "baseline"],
    ] {
        let status = Command::new("git")
            .args(args)
            .current_dir(fixture.path())
            .status()?;
        assert!(status.success(), "git fixture command failed");
    }
    Ok(())
}
