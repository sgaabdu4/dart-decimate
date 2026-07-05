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
