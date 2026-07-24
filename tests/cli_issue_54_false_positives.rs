use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn riverpod_notifier_suffix_provider_keeps_owner_live() -> Result<(), Box<dyn std::error::Error>> {
    let json = symbol_usage_report()?;
    assert!(!finding_targets_symbol(
        &json,
        "dart-decimate/unused-export",
        "LoginFormNotifier"
    ));
    assert!(finding_targets_symbol(
        &json,
        "dart-decimate/unused-export",
        "UnusedService"
    ));
    assert!(finding_targets_symbol(
        &json,
        "dart-decimate/unused-export",
        "UnusedBrightness"
    ));
    Ok(())
}

#[test]
fn implicit_extension_member_usage_keeps_extension_live() -> Result<(), Box<dyn std::error::Error>>
{
    let json = symbol_usage_report()?;
    assert!(!finding_targets_symbol(
        &json,
        "dart-decimate/unused-export",
        "ColorAlpha"
    ));
    assert!(finding_targets_symbol(
        &json,
        "dart-decimate/unused-export",
        "UnusedService"
    ));
    Ok(())
}

#[test]
fn external_and_interpolated_enum_member_usage_stays_live() -> Result<(), Box<dyn std::error::Error>>
{
    let json = symbol_usage_report()?;
    for name in ["settings", "doctor", "scan", "other", "tips"] {
        assert!(
            !finding_targets_symbol(&json, "dart-decimate/unused-enum-member", name),
            "{name} is an externally referenced enum constant"
        );
    }
    assert!(finding_targets_symbol(
        &json,
        "dart-decimate/unused-enum-member",
        "unused"
    ));
    assert!(finding_targets_symbol(
        &json,
        "dart-decimate/unused-enum-member",
        "collision"
    ));
    Ok(())
}

fn symbol_usage_report() -> Result<Value, Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_symbol_usage_fixture(&fixture)?;
    let (_, json) = run_json([
        "dart-decimate",
        "check",
        root(&fixture),
        "--format",
        "json",
        "--include-entry-exports",
    ])?;
    Ok(json)
}

fn write_symbol_usage_fixture(fixture: &TempDir) -> Result<(), Box<dyn std::error::Error>> {
    write(
        fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  riverpod_annotation: any\n\
dev_dependencies:\n  riverpod_generator: any\n",
    )?;
    write(
        fixture,
        "lib/main.dart",
        r"import 'login_form_notifier.dart';
import 'src/types.dart';
import 'src/unused_extension.dart';
import 'other.dart';

void main() {
  ref.watch(loginFormProvider);
  print(Color(1).alpha10);
  print(AppRoute.settings.name);
  print(ReminderType.values.map(icon));
  print('/resources/${Tab.tips.id}');
  print(UnusedEnum.used.name);
  print(OtherEnum.collision.name);
  touch(Ordinary());
}

String icon(ReminderType type) => switch (type) {
  ReminderType.doctor => 'doctor',
  ReminderType.scan => 'scan',
  ReminderType.other => 'other',
};

final ref = Ref();
class Ref {
  void watch(Object provider) {}
}
",
    )?;
    write(
        fixture,
        "lib/other.dart",
        r"class Ordinary {
  int get brightness => 1;
}

void touch(Ordinary value) {
  print(value.brightness);
}
",
    )?;
    write(
        fixture,
        "lib/login_form_notifier.dart",
        r"import 'package:riverpod_annotation/riverpod_annotation.dart';
part 'login_form_notifier.g.dart';

@riverpod
class LoginFormNotifier extends _$LoginFormNotifier {
  int build() => 0;
}

class _$LoginFormNotifier {}
",
    )?;
    write(
        fixture,
        "lib/login_form_notifier.g.dart",
        "part of 'login_form_notifier.dart';\nfinal loginFormProvider = Object();\n",
    )?;
    write(
        fixture,
        "lib/src/types.dart",
        r"extension ColorAlpha on Color {
  double get alpha10 => value * 0.1;
}

class Color {
  const Color(this.value);
  final double value;
}

enum AppRoute { home, settings }
enum ReminderType { doctor, scan, other }
enum Tab {
  journey,
  tips;

  String get id => name;
}
enum UnusedEnum { used, unused, collision }
enum OtherEnum { collision }
class UnusedService {}
",
    )?;
    write(
        fixture,
        "lib/src/unused_extension.dart",
        r"extension UnusedBrightness on Object {
  int get brightness => 1;
}
",
    )?;
    Ok(())
}

#[test]
fn flutterfire_api_key_and_password_validation_copy_are_not_secrets()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/firebase_options.dart",
        r"class FirebaseOptions {
  const FirebaseOptions({required this.apiKey});
  final String apiKey;
}

const web = FirebaseOptions(
  apiKey: 'AIzaSyA0000000000000000000000000000000000',
);
",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"import 'firebase_options.dart';

const apiSecret = 'AKIAIOSFODNN7EXAMPLE';

String? validateConfirmPassword(String password, String confirmPassword) {
  if (confirmPassword.isEmpty) return 'Please confirm your password';
  if (password != confirmPassword) return 'Passwords do not match';
  return null;
}

void main() {
  print(web);
  print(apiSecret);
}
",
    )?;

    let (_, json) = run_json([
        "dart-decimate",
        "security",
        root(&fixture),
        "--format",
        "json",
    ])?;
    let categories = security_categories(&json);

    assert!(!categories.contains(&"firebase-api-key"));
    assert_eq!(
        categories
            .iter()
            .filter(|category| **category == "hardcoded-secret")
            .count(),
        1
    );
    assert!(
        json["security_candidates"]
            .as_array()
            .is_some_and(|candidates| {
                candidates.iter().all(|candidate| {
                    candidate["occurrences"]
                        .as_array()
                        .is_some_and(|occurrences| {
                            occurrences
                                .iter()
                                .all(|occurrence| occurrence["path"] != "lib/firebase_options.dart")
                        })
                })
            })
    );
    Ok(())
}

#[test]
fn flutter_assets_and_native_plugins_count_as_dependency_usage()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        r"name: app
dependencies:
  flutter:
    sdk: flutter
  cupertino_icons: ^1.0.0
  firebase_crashlytics: ^4.0.0
  asset_package: ^1.0.0
  unused_regular: ^1.0.0
flutter:
  assets:
    - packages/asset_package/images/logo.png
",
    )?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(
        &fixture,
        ".flutter-plugins-dependencies",
        r#"{
  "plugins": {
    "android": [
      {
        "name": "firebase_crashlytics",
        "path": "/tmp/firebase_crashlytics",
        "native_build": true,
        "dependencies": []
      }
    ]
  },
  "dependencyGraph": [
    {"name": "firebase_crashlytics", "dependencies": []}
  ],
  "diagnostics": {"name": "unused_regular"}
}
"#,
    )?;

    let (_, json) = run_json(["dart-decimate", "check", root(&fixture), "--format", "json"])?;
    let unused = unused_dependencies(&json);

    assert_eq!(unused, vec!["unused_regular"]);
    Ok(())
}

#[test]
fn allowed_feature_flags_remain_inventory_without_findings()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".dart-decimaterc",
        "[cli]\nformat = \"json\"\n\n[flags]\nallow = [\"SKIP_PERMISSION_PROMPT\"]\n",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "const skipPrompt = bool.fromEnvironment('SKIP_PERMISSION_PROMPT');\n\
const beta = bool.fromEnvironment('FEATURE_BETA');\n\
void main() { print(skipPrompt); print(beta); }\n",
    )?;

    let (_, json) = run_json(["dart-decimate", "check", root(&fixture)])?;
    let inventory = json["feature_flags"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|flag| flag["name"].as_str())
        .collect::<Vec<_>>();

    assert!(inventory.contains(&"SKIP_PERMISSION_PROMPT"));
    assert!(inventory.contains(&"FEATURE_BETA"));
    assert!(!finding_targets_symbol(
        &json,
        "dart-decimate/feature-flag",
        "SKIP_PERMISSION_PROMPT"
    ));
    assert!(finding_targets_symbol(
        &json,
        "dart-decimate/feature-flag",
        "FEATURE_BETA"
    ));
    Ok(())
}

#[test]
fn model_entity_mapping_boundary_is_not_a_clone() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/data/user_model.dart",
        r"import '../domain/user_entity.dart';

class UserModel {
  const UserModel({
    required this.id,
    required this.name,
    required this.email,
    required this.phone,
  });
  final String id;
  final String name;
  final String email;
  final String phone;

  UserEntity toEntity() => UserEntity(
    id: id,
    name: name,
    email: email,
    phone: phone,
  );
}
",
    )?;
    write(
        &fixture,
        "lib/domain/user_entity.dart",
        r"class UserEntity {
  const UserEntity({
    required this.id,
    required this.name,
    required this.email,
    required this.phone,
  });
  final String id;
  final String name;
  final String email;
  final String phone;
}
",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'data/user_model.dart';\nvoid main() { print(UserModel); }\n",
    )?;
    let real_clone = "void shared() {\n  final items = [1, 2, 3];\n  final active = items.where((item) => item > 1);\n  print(active.length);\n}\n";
    write(&fixture, "lib/a.dart", real_clone)?;
    write(&fixture, "lib/b.dart", real_clone)?;

    let (_, json) = run_json([
        "dart-decimate",
        "dupes",
        root(&fixture),
        "--format",
        "json",
        "--min-lines",
        "5",
        "--min-tokens",
        "10",
    ])?;

    assert_eq!(json["summary"]["code_duplications"], 1);
    assert_eq!(
        json["clone_groups"][0]["instances"][0]["path"],
        "lib/a.dart"
    );
    assert_eq!(
        json["clone_groups"][0]["instances"][1]["path"],
        "lib/b.dart"
    );
    Ok(())
}

fn security_categories(json: &Value) -> Vec<&str> {
    json["security_candidates"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|candidate| candidate["category"].as_str())
        .collect()
}

fn unused_dependencies(json: &Value) -> Vec<&str> {
    json["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|finding| {
            matches!(
                finding["rule_id"].as_str(),
                Some("dart-decimate/unused-dependency" | "dart-decimate/unused-dev-dependency")
            )
        })
        .filter_map(|finding| {
            finding["actions"]
                .as_array()
                .into_iter()
                .flatten()
                .find_map(|action| action["target_dependency"].as_str())
        })
        .collect()
}

fn finding_targets_symbol(json: &Value, rule_id: &str, name: &str) -> bool {
    json["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|finding| finding["rule_id"] == rule_id)
        .any(|finding| {
            finding["actions"].as_array().is_some_and(|actions| {
                actions.iter().any(|action| {
                    action["target_symbol"].as_str().is_some_and(|target| {
                        target == name || target.ends_with(&format!(".{name}"))
                    })
                })
            })
        })
}

fn run_json<I, S>(args: I) -> Result<(i32, Value), Box<dyn std::error::Error>>
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> + Clone,
{
    let mut output = Vec::new();
    let code = run_from(args, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    Ok((code, json))
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
