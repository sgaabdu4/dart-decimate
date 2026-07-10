use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn git_ignored_generated_files_do_not_emit_source_findings()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, ".gitignore", "**/*.g.dart\n**/*.gen.dart\n")?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(
        &fixture,
        "lib/src/example_dto.g.dart",
        "// GENERATED CODE - DO NOT MODIFY BY HAND\npart of 'example_dto.dart';\n",
    )?;
    write(
        &fixture,
        "lib/gen/assets.gen.dart",
        "// GENERATED CODE - DO NOT MODIFY BY HAND\nimport 'package:vector_graphics/vector_graphics.dart';\n",
    )?;
    write(&fixture, "packages/shared/pubspec.yaml", "name: shared\n")?;
    write(
        &fixture,
        "packages/shared/lib/nested.g.dart",
        "// GENERATED CODE - DO NOT MODIFY BY HAND\npart of 'nested.dart';\n",
    )?;
    write(
        &fixture,
        "packages/shared/lib/assets.gen.dart",
        "// GENERATED CODE - DO NOT MODIFY BY HAND\nimport 'package:vector_graphics/vector_graphics.dart';\n",
    )?;

    let json = check(&fixture)?;

    assert_no_finding_path(
        &json,
        "dart-decimate/part-of-violation",
        "lib/src/example_dto.g.dart",
    );
    assert_no_finding_path(
        &json,
        "dart-decimate/unlisted-dependency",
        "lib/gen/assets.gen.dart",
    );
    assert_no_finding_path(
        &json,
        "dart-decimate/part-of-violation",
        "packages/shared/lib/nested.g.dart",
    );
    assert_no_finding_path(
        &json,
        "dart-decimate/unlisted-dependency",
        "packages/shared/lib/assets.gen.dart",
    );
    Ok(())
}

#[test]
fn runner_discovered_tests_are_reachability_roots() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\npatrol:\n  test_file_suffix: _patrol.dart\n",
    )?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(
        &fixture,
        "integration_test/features/login_patrol.dart",
        "import '../../integration_test_utilities/run_integration_test.dart';\nvoid main() => runIntegrationTest();\n",
    )?;
    write(
        &fixture,
        "integration_test_utilities/run_integration_test.dart",
        "@isTest\nvoid runIntegrationTest() {}\n",
    )?;
    write(&fixture, "test/features/smoke.dart", "void main() {}\n")?;

    let json = check(&fixture)?;

    for path in [
        "integration_test/features/login_patrol.dart",
        "integration_test_utilities/run_integration_test.dart",
        "test/features/smoke.dart",
    ] {
        assert_no_finding_path(&json, "dart-decimate/dead-file", path);
    }
    Ok(())
}

#[test]
fn patrol_suffix_is_scoped_to_its_package() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\npatrol:\n  test_file_suffix: _patrol.dart\n",
    )?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(
        &fixture,
        "integration_test/features/root_patrol.dart",
        "class RootPatrolTest {}\n",
    )?;
    write(&fixture, "packages/plain/pubspec.yaml", "name: plain\n")?;
    write(
        &fixture,
        "packages/plain/integration_test/features/plain_patrol.dart",
        "class PlainPatrolHelper {}\n",
    )?;
    write(
        &fixture,
        "packages/shared/pubspec.yaml",
        "name: shared\npatrol:\n  test_file_suffix: _patrol.dart\n",
    )?;
    write(
        &fixture,
        "packages/shared/integration_test/features/shared_patrol.dart",
        "class SharedPatrolTest {}\n",
    )?;

    let json = check(&fixture)?;
    assert_no_finding_path(
        &json,
        "dart-decimate/dead-file",
        "integration_test/features/root_patrol.dart",
    );
    assert_finding_path(
        &json,
        "dart-decimate/dead-file",
        "packages/plain/integration_test/features/plain_patrol.dart",
    );
    assert_no_finding_path(
        &json,
        "dart-decimate/dead-file",
        "packages/shared/integration_test/features/shared_patrol.dart",
    );
    Ok(())
}

#[test]
fn imported_widget_declarations_resolve_through_prefixes_exports_and_parts()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/avatar.dart';\nimport 'src/a.dart' as a;\nimport 'src/b.dart' as b;\nimport 'src/input_barrel.dart' as visible;\nvoid main() { const Avatar(); a.SearchInput(); visible.SearchInput(); }\n",
    )?;
    write(
        &fixture,
        "lib/src/framework.dart",
        "class Widget {}\nclass StatelessWidget extends Widget { const StatelessWidget(); }\nclass StatefulWidget extends Widget { const StatefulWidget(); }\nclass BuildContext {}\nclass SizedBox extends Widget { const SizedBox.square({double? dimension}); }\n",
    )?;
    write(
        &fixture,
        "lib/src/avatar_library.dart",
        "import 'framework.dart';\npart 'avatar_base.dart';\n",
    )?;
    write(
        &fixture,
        "lib/src/avatar_base.dart",
        "part of 'avatar_library.dart';\nabstract class AvatarBase extends StatelessWidget { const AvatarBase({this.size}); final double? size; }\n",
    )?;
    write(
        &fixture,
        "lib/src/avatar_barrel.dart",
        "export 'avatar_library.dart';\n",
    )?;
    write(
        &fixture,
        "lib/src/avatar.dart",
        "import 'framework.dart';\nimport 'avatar_barrel.dart' as shared;\nclass Avatar extends shared.AvatarBase { const Avatar({super.size}); Widget build(BuildContext context) => SizedBox.square(dimension: size); }\n",
    )?;
    write(
        &fixture,
        "lib/src/a.dart",
        "import 'framework.dart';\nclass BaseInput extends StatefulWidget {}\nclass SearchInput extends BaseInput {}\n",
    )?;
    write(
        &fixture,
        "lib/src/b.dart",
        "import 'framework.dart';\nclass BaseInput extends StatefulWidget {}\nclass SearchInput extends BaseInput {}\n",
    )?;
    write(
        &fixture,
        "lib/src/input_barrel.dart",
        "export 'a.dart' show SearchInput;\nexport 'b.dart' hide SearchInput;\n",
    )?;

    let json = check(&fixture)?;

    assert_no_action_target(
        &json,
        "dart-decimate/unused-widget-param",
        "AvatarBase.size",
    );
    assert_no_finding_path(&json, "dart-decimate/unrendered-widget", "lib/src/a.dart");
    assert_finding_path(&json, "dart-decimate/unrendered-widget", "lib/src/b.dart");
    Ok(())
}

#[test]
fn tooling_conventions_count_as_dev_dependency_usage() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  envied: ^1.0.0\n\
dev_dependencies:\n  build_runner: ^2.0.0\n  envied_generator: ^1.0.0\n  flutter_gen_runner: ^5.0.0\n  flutter_launcher_icons: ^0.14.0\n  test: ^1.0.0\n\
flutter_gen:\n  integrations:\n    flutter_svg: true\n",
    )?;
    write(
        &fixture,
        "lib/env.dart",
        "import 'package:envied/envied.dart';\npart 'env.g.dart';\n@Envied()\nabstract class Env {}\n",
    )?;
    write(
        &fixture,
        "lib/env.g.dart",
        "// GENERATED CODE - DO NOT MODIFY BY HAND\npart of 'env.dart';\n",
    )?;
    write(
        &fixture,
        "flutter_launcher_icons.yaml",
        "flutter_launcher_icons:\n  android: true\n",
    )?;
    write(&fixture, "test/smoke.dart", "void main() {}\n")?;

    let json = check(&fixture)?;

    for dependency in [
        "build_runner",
        "envied_generator",
        "flutter_gen_runner",
        "flutter_launcher_icons",
        "test",
    ] {
        assert_no_unused_dev_dependency(&json, dependency);
    }
    Ok(())
}

#[test]
fn test_dependency_requires_a_runner_discoverable_file() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndev_dependencies:\n  test: ^1.0.0\n",
    )?;
    write(
        &fixture,
        "test/support.dart",
        "String fixtureName() => 'support';\n",
    )?;

    let helper_only = check(&fixture)?;
    assert_unused_dev_dependency(&helper_only, "test");

    write(
        &fixture,
        "test/scenarios/smoke.dart",
        "void main() { fixtureName(); }\n",
    )?;
    let runnable = check(&fixture)?;
    assert_no_unused_dev_dependency(&runnable, "test");
    Ok(())
}

#[test]
fn widget_initializer_and_inheritance_reads_count_as_usage()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'avatar.dart';\nimport 'widgets.dart';\nvoid main() {\n  TimerView(builder: (_, _) => const SizedBox());\n  TableView.builder(itemCount: 1);\n  const AssertedView(count: 1);\n  const InitialsAvatar();\n  const CrossFileAvatar();\n  const Screen();\n}\n",
    )?;
    write(
        &fixture,
        "lib/widgets.dart",
        r"class BuildContext {}
class Widget {}
class StatelessWidget extends Widget { const StatelessWidget(); }
class StatefulWidget extends Widget { const StatefulWidget(); }
class State<T> {}
class SizedBox extends Widget {
  const SizedBox();
  const SizedBox.square({double? dimension});
}

class TimerView extends StatefulWidget {
  const TimerView({required this.builder});
  final Widget Function(BuildContext context, Object state) builder;
}

class _TimerViewState extends State<TimerView> {
  Widget build(BuildContext context) => widget.builder(context, Object());
}

class TableView extends StatelessWidget {
  TableView.builder({this.header, required int itemCount})
      : itemCount = itemCount + (header == null ? 0 : 1);
  final Widget? header;
  final int itemCount;
  Widget build(BuildContext context) => const SizedBox();
}

class AssertedView extends StatelessWidget {
  const AssertedView({required this.count}) : assert(count > 0);
  final int count;
  Widget build(BuildContext context) => const SizedBox();
}

abstract class BaseAvatar extends StatelessWidget {
  const BaseAvatar({this.size});
  final double? size;
}

class InitialsAvatar extends BaseAvatar {
  const InitialsAvatar({super.size});
  Widget build(BuildContext context) => SizedBox.square(dimension: size ?? 40);
}

class BaseInput extends StatefulWidget {
  const BaseInput();
}

class SearchInput extends BaseInput {
  const SearchInput();
}

class Screen extends StatelessWidget {
  const Screen();
  Widget build(BuildContext context) => const SearchInput();
}
",
    )?;
    write(
        &fixture,
        "lib/base_avatar.dart",
        r"import 'widgets.dart';

abstract class CrossFileBase extends StatelessWidget {
  const CrossFileBase({this.size});
  final double? size;
}
",
    )?;
    write(
        &fixture,
        "lib/avatar.dart",
        r"import 'base_avatar.dart';
import 'widgets.dart';

class CrossFileAvatar extends CrossFileBase {
  const CrossFileAvatar({super.size});
  Widget build(BuildContext context) => SizedBox.square(dimension: size ?? 40);
}
",
    )?;

    let json = check(&fixture)?;

    for target in [
        "TimerView.builder",
        "TableView.header",
        "TableView.itemCount",
        "AssertedView.count",
        "BaseAvatar.size",
        "CrossFileBase.size",
    ] {
        assert_no_action_target(&json, "dart-decimate/unused-widget-param", target);
    }
    assert_no_action_target(&json, "dart-decimate/unrendered-widget", "BaseInput");
    Ok(())
}

#[test]
fn unrelated_class_name_does_not_render_widget_hierarchy() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'other.dart';\nvoid main() { SearchInput(); }\n",
    )?;
    write(&fixture, "lib/other.dart", "class SearchInput {}\n")?;
    write(
        &fixture,
        "lib/widgets.dart",
        r"class Widget {}
class StatefulWidget extends Widget { const StatefulWidget(); }
class BaseInput extends StatefulWidget { const BaseInput(); }
class SearchInput extends BaseInput { const SearchInput(); }
",
    )?;

    let json = check(&fixture)?;
    assert_action_target(&json, "dart-decimate/unrendered-widget", "BaseInput");
    Ok(())
}

#[test]
fn security_classification_keeps_only_real_review_candidates()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        r"import 'dart:io';

const authorizationEndpoint = 'https://login.acme.com/oauth2/authorize';
const tokenEndpoint = 'https://login.acme.com/oauth2/token';
const firebaseOptions = FirebaseOptions(
  apiKey: 'AIzaSyExamplePublicFirebaseApiKey',
  appId: '1:000000000000:android:example',
  messagingSenderId: '000000000000',
  projectId: 'example-project',
);

Never fail() => throw const AuthException('User ID not found or invalid in token payload');

Future<Process> startAnalysisServer() {
  final dartExe = Platform.resolvedExecutable;
  final snapshot = '/path/to/analysis_server.dart.snapshot';
  return Process.start(dartExe, [snapshot]);
}

Future<Process> unsafeStart(String command, String argument) {
  return Process.start(command, [argument], runInShell: true);
}

abstract class Env {
  @EnviedField(
    varName: 'STRIPE_SECRET_KEY',
    defaultValue: 'sk_test_replace_with_your_secret',
  )
  static final String stripeSecretKey = _Env.stripeSecretKey;
}
",
    )?;

    let json = security(&fixture)?;
    let candidates = json["security_candidates"]
        .as_array()
        .unwrap_or_else(|| panic!("security_candidates array"));

    assert_eq!(candidates.len(), 2, "{json:#}");
    let secret = candidate(candidates, "hardcoded-secret");
    assert_eq!(secret["occurrences"].as_array().map(Vec::len), Some(2));
    assert_eq!(secret["occurrences"][0]["line"], 26);
    assert_eq!(secret["occurrences"][1]["line"], 27);
    let process = candidate(candidates, "process-execution");
    assert_eq!(process["occurrences"].as_array().map(Vec::len), Some(1));
    assert_eq!(process["occurrences"][0]["line"], 21);
    Ok(())
}

#[test]
fn stripe_secret_names_and_prefixes_are_independent_candidates()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        r"const envName = 'STRIPE_SECRET_KEY';
const stripeSecretKey = 'placeholder';
const testKey = 'sk_test_replace_with_your_secret';
const liveKey = 'sk_live_replace_with_your_secret';
",
    )?;

    let json = security(&fixture)?;
    let candidates = json["security_candidates"]
        .as_array()
        .unwrap_or_else(|| panic!("security_candidates array"));
    let secret = candidate(candidates, "hardcoded-secret");
    assert_eq!(secret["occurrences"].as_array().map(Vec::len), Some(4));
    Ok(())
}

#[test]
fn multiline_run_in_shell_is_a_process_candidate() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        r"import 'dart:io';

Future<Process> unsafeStart() {
  final dartExe = Platform.resolvedExecutable;
  final snapshot = '/path/to/analysis_server.dart.snapshot';
  return Process.start(
    dartExe,
    [
      snapshot,
    ],
    runInShell: true,
  );
}
",
    )?;

    let json = security(&fixture)?;
    let candidates = json["security_candidates"]
        .as_array()
        .unwrap_or_else(|| panic!("security_candidates array"));

    let process = candidate(candidates, "process-execution");
    assert_eq!(process["occurrences"].as_array().map(Vec::len), Some(1));
    Ok(())
}

#[test]
fn process_start_exemption_requires_lexical_fixed_values_and_disabled_shell()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        r"import 'dart:io';

final dartExe = Platform.resolvedExecutable;
const snapshot = '/path/to/analysis_server.dart.snapshot';
const useShell = true;

Future<Process> safeStart() {
  final localDartExe = Platform.resolvedExecutable;
  final localSnapshot = '/path/to/local.snapshot';
  return Process.start(
    localDartExe,
    [localSnapshot],
    runInShell: false,
  );
}

Future<Process> unsafeShellStart() {
  return Process.start(dartExe, [snapshot], runInShell: useShell);
}

Future<Process> unsafeShadowedStart(String dartExe) {
  return Process.start(dartExe, [snapshot]);
}

Future<Process?> unsafeCatchShadow() async {
  try {
    return null;
  } catch (dartExe) {
    return Process.start(dartExe, [snapshot]);
  }
}

class Runner {
  final String dartExe;
  Runner(this.dartExe);

  Future<Process> unsafeClassShadow() {
    return Process.start(dartExe, [snapshot]);
  }
}

Future<Process> unsafeMutableStart(String command, bool replace) {
  var mutableDartExe = Platform.resolvedExecutable;
  final localSnapshot = '/path/to/mutable.snapshot';
  if (replace) {
    mutableDartExe = command;
  }
  return Process.start(mutableDartExe, [localSnapshot]);
}
",
    )?;

    let json = security(&fixture)?;
    let candidates = json["security_candidates"]
        .as_array()
        .unwrap_or_else(|| panic!("security_candidates array"));
    let process = candidate(candidates, "process-execution");
    let occurrences = process["occurrences"]
        .as_array()
        .unwrap_or_else(|| panic!("process occurrences: {json:#}"));

    assert_eq!(occurrences.len(), 5, "{json:#}");
    assert_eq!(
        occurrences
            .iter()
            .map(|occurrence| occurrence["line"].as_u64())
            .collect::<Vec<_>>(),
        vec![Some(18), Some(22), Some(29), Some(38), Some(48)]
    );
    Ok(())
}

#[test]
fn fixed_shells_and_for_in_shadowing_remain_process_candidates()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        r"import 'dart:io';

final dartExe = Platform.resolvedExecutable;
const snapshot = '/path/to/analysis_server.dart.snapshot';

Future<Process> shellStart(String command) {
  return Process.start('sh', ['-c', command]);
}

Future<ProcessResult> shellRun(String command) {
  return Process.run('/bin/bash', ['-c', command]);
}

Future<void> shadowedLoop(List<String> commands) async {
  for (final dartExe in commands) {
    await Process.start(dartExe, [snapshot]);
  }
}
",
    )?;

    let json = security(&fixture)?;
    let candidates = json["security_candidates"]
        .as_array()
        .unwrap_or_else(|| panic!("security_candidates array"));
    let process = candidate(candidates, "process-execution");
    let occurrences = process["occurrences"]
        .as_array()
        .unwrap_or_else(|| panic!("process occurrences: {json:#}"));

    assert_eq!(occurrences.len(), 3, "{json:#}");
    assert_eq!(
        occurrences
            .iter()
            .map(|occurrence| occurrence["line"].as_u64())
            .collect::<Vec<_>>(),
        vec![Some(7), Some(11), Some(16)]
    );
    Ok(())
}

#[test]
fn quoted_oauth_map_keys_are_metadata() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        r"const oauthMetadata = <String, String>{
  'authorizationEndpoint': 'https://login.acme.com/oauth2/authorize',
  'tokenEndpoint': 'https://login.acme.com/oauth2/token',
};
",
    )?;

    let json = security(&fixture)?;
    assert!(
        json["security_candidates"]
            .as_array()
            .is_none_or(Vec::is_empty),
        "{json:#}"
    );
    Ok(())
}

fn check(fixture: &TempDir) -> Result<Value, Box<dyn std::error::Error>> {
    run_json(["dart-decimate", "check", root(fixture), "--format", "json"])
}

fn security(fixture: &TempDir) -> Result<Value, Box<dyn std::error::Error>> {
    run_json([
        "dart-decimate",
        "security",
        root(fixture),
        "--format",
        "json",
    ])
}

fn run_json<const N: usize>(args: [&str; N]) -> Result<Value, Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let _code = run_from(args, &mut output)?;
    Ok(serde_json::from_slice(&output)?)
}

fn candidate<'a>(candidates: &'a [Value], category: &str) -> &'a Value {
    candidates
        .iter()
        .find(|candidate| candidate["category"] == category)
        .unwrap_or_else(|| panic!("missing {category} candidate: {candidates:#?}"))
}

fn assert_no_finding_path(json: &Value, rule_id: &str, path: &str) {
    assert!(
        findings(json).all(|finding| finding["rule_id"] != rule_id || finding["path"] != path),
        "unexpected {rule_id} for {path}: {json:#}"
    );
}

fn assert_finding_path(json: &Value, rule_id: &str, path: &str) {
    assert!(
        findings(json).any(|finding| finding["rule_id"] == rule_id && finding["path"] == path),
        "missing {rule_id} for {path}: {json:#}"
    );
}

fn assert_no_unused_dev_dependency(json: &Value, dependency: &str) {
    assert!(
        findings(json).all(|finding| {
            finding["rule_id"] != "dart-decimate/unused-dev-dependency"
                || finding["actions"][0]["target_dependency"] != dependency
        }),
        "unexpected unused dev dependency {dependency}: {json:#}"
    );
}

fn assert_unused_dev_dependency(json: &Value, dependency: &str) {
    assert!(
        findings(json).any(|finding| {
            finding["rule_id"] == "dart-decimate/unused-dev-dependency"
                && finding["actions"][0]["target_dependency"] == dependency
        }),
        "missing unused dev dependency {dependency}: {json:#}"
    );
}

fn assert_no_action_target(json: &Value, rule_id: &str, target: &str) {
    assert!(
        findings(json).all(|finding| {
            finding["rule_id"] != rule_id || finding["actions"][0]["target_symbol"] != target
        }),
        "unexpected {rule_id} for {target}: {json:#}"
    );
}

fn assert_action_target(json: &Value, rule_id: &str, target: &str) {
    assert!(
        findings(json).any(|finding| {
            finding["rule_id"] == rule_id && finding["actions"][0]["target_symbol"] == target
        }),
        "missing {rule_id} for {target}: {json:#}"
    );
}

fn findings(json: &Value) -> impl Iterator<Item = &Value> {
    json["findings"].as_array().into_iter().flatten()
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
