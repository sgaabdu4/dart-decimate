use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn flutter_style_is_opt_in_and_advisory() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = flutter_fixture()?;
    let root = fixture.path().to_str().unwrap_or(".");
    let mut default_output = Vec::new();

    let default_code = run_from(
        ["dart-decimate", "health", root, "--format", "json"],
        &mut default_output,
    )?;
    let default_json = serde_json::from_slice::<Value>(&default_output)?;
    assert_eq!(default_code, 0);
    assert_eq!(default_json["verdict"], "pass");
    assert_eq!(default_json["summary"]["raw_flutter_style_values"], 0);
    assert_eq!(
        default_json["flutter_style"].as_array().map(Vec::len),
        Some(0)
    );

    let mut enabled_output = Vec::new();
    let enabled_code = run_from(
        [
            "dart-decimate",
            "health",
            root,
            "--format",
            "json",
            "--flutter-style",
        ],
        &mut enabled_output,
    )?;
    let enabled_json = serde_json::from_slice::<Value>(&enabled_output)?;
    assert_eq!(enabled_code, 0);
    assert_eq!(enabled_json["verdict"], "pass");
    assert_eq!(enabled_json["summary"]["raw_flutter_style_values"], 3);
    assert!(
        enabled_json["summary"]["near_duplicate_theme_tokens"]
            .as_u64()
            .is_some_and(|count| count >= 1)
    );
    assert!(
        enabled_json["summary"]["unused_theme_extension_tokens"]
            .as_u64()
            .is_some_and(|count| count >= 1)
    );
    let Some(findings) = enabled_json["flutter_style"].as_array() else {
        return Err("flutter style array missing".into());
    };
    assert!(
        findings
            .iter()
            .filter(|finding| { finding["kind"] == "raw-flutter-style-value" })
            .all(|finding| finding["path"] == "lib/main.dart")
    );
    let Some(raw) = findings
        .iter()
        .find(|finding| finding["kind"] == "raw-flutter-style-value")
    else {
        return Err("raw style finding missing".into());
    };
    assert_eq!(raw["rule_id"], "dart-decimate/raw-flutter-style-value");
    assert_eq!(raw["path"], "lib/main.dart");
    assert!(raw["nearest_token"]["name"].is_string());
    assert!(findings.iter().any(|finding| {
        finding["kind"] == "near-duplicate-theme-token"
            && finding["distance"] == "1.00"
            && finding["token"]["path"] == "lib/theme.dart"
            && finding["nearest_token"]["path"] == "lib/theme.dart"
    }));
    assert!(findings.iter().any(|finding| {
        finding["kind"] == "unused-theme-extension-token"
            && finding["token"]["name"] == "unusedTone"
    }));
    assert!(enabled_json["findings"].as_array().is_some_and(|rows| {
        rows.iter().all(|finding| {
            !finding["rule_id"]
                .as_str()
                .is_some_and(|rule| rule.contains("flutter-style"))
                || finding["severity"] == "warning"
        })
    }));
    let mut human_output = Vec::new();
    let human_code = run_from(
        ["dart-decimate", "health", root, "--flutter-style"],
        &mut human_output,
    )?;
    assert_eq!(human_code, 0);
    assert!(String::from_utf8(human_output)?.contains("Flutter Style (advisory)"));
    let mut repeated_output = Vec::new();
    let repeated_code = run_from(
        [
            "dart-decimate",
            "health",
            root,
            "--format",
            "json",
            "--flutter-style",
        ],
        &mut repeated_output,
    )?;
    assert_eq!(repeated_code, enabled_code);
    assert_eq!(repeated_output, enabled_output);

    Ok(())
}

#[test]
fn unresolved_nearest_token_abstains() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:flutter/material.dart';\nfinal color = Color(0xFF112233);\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "health",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--flutter-style",
        ],
        &mut output,
    )?;
    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["raw_flutter_style_values"], 0);
    assert_eq!(json["flutter_style"].as_array().map(Vec::len), Some(0));

    Ok(())
}

#[test]
fn ignored_test_and_example_paths_do_not_add_style_findings()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = flutter_fixture()?;
    write(
        &fixture,
        "test/style_test.dart",
        "const ignored = Color(0xFF77889B);\n",
    )?;
    write(
        &fixture,
        "example/example.dart",
        "const ignored = Color(0xFF77889B);\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "health",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--flutter-style",
        ],
        &mut output,
    )?;
    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["raw_flutter_style_values"], 3);
    assert!(json["flutter_style"].as_array().is_some_and(|findings| {
        findings.iter().all(|finding| {
            !finding["path"]
                .as_str()
                .is_some_and(|path| path.starts_with("test/") || path.starts_with("example/"))
        })
    }));

    Ok(())
}

#[test]
fn static_app_theme_tokens_are_theme_definers() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = flutter_fixture()?;
    write(
        &fixture,
        "lib/app_theme.dart",
        r"
import 'package:flutter/material.dart';

abstract final class AppTheme {
  static const Color surface = Color(0xFF070707);
  static const Color accent = Color(0xFFDDFE52);
  static const Color transparent = Colors.transparent;
  static const TextStyle title = TextStyle(fontSize: 20);
}
",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "health",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--flutter-style",
        ],
        &mut output,
    )?;
    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["raw_flutter_style_values"], 3);
    assert!(json["flutter_style"].as_array().is_some_and(|findings| {
        findings
            .iter()
            .all(|finding| finding["path"] != "lib/app_theme.dart")
    }));

    Ok(())
}

#[test]
fn static_theme_owner_tokens_are_available_as_replacements()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'package:flutter/material.dart';

abstract final class AppTheme {
  static const Color primary = Color(0xFF112233);
}

final raw = Color(0xFF112234);
",
    )?;

    let json = flutter_style_json(&fixture)?;
    assert_eq!(json["summary"]["raw_flutter_style_values"], 1, "{json:#}");
    assert!(json["flutter_style"].as_array().is_some_and(|findings| {
        findings.iter().any(|finding| {
            finding["kind"] == "raw-flutter-style-value"
                && finding["nearest_token"]["name"] == "AppTheme.primary"
        })
    }));

    Ok(())
}

#[test]
fn prefixed_documented_color_constructors_are_analyzed() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'package:flutter/material.dart' as m;

final scheme = m.ColorScheme.light(
  primary: m.Color.fromRGBO(17, 34, 51, 1),
);
final raw = m.Color.from(
  alpha: 1,
  red: 17 / 255,
  green: 34 / 255,
  blue: 52 / 255,
);
",
    )?;

    let json = flutter_style_json(&fixture)?;
    assert_eq!(json["summary"]["raw_flutter_style_values"], 1);
    assert!(json["flutter_style"].as_array().is_some_and(|findings| {
        findings.iter().any(|finding| {
            finding["kind"] == "raw-flutter-style-value"
                && finding["nearest_token"]["name"] == "colorScheme.primary"
        })
    }));

    Ok(())
}

#[test]
fn documented_color_literal_forms_are_analyzed() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'package:flutter/material.dart';

abstract final class AppColors {
  static const Color primary = Color(0xFF112233);
}

final unnamed = Color.new(0xFF112234);
final separated = Color(0xFF_11_22_35);
final channels = Color.fromARGB(0xFF, 0x11, 0x22, 0x36);
final lowerBits = Color.fromARGB(0x1FF, 0x111, 0x122, 0x134);
final negative = Color(-0x00EEDDCC);
",
    )?;

    let json = flutter_style_json(&fixture)?;
    assert_eq!(json["summary"]["raw_flutter_style_values"], 5, "{json:#}");
    assert!(json["flutter_style"].as_array().is_some_and(|findings| {
        findings.iter().all(|finding| {
            finding["kind"] != "raw-flutter-style-value"
                || finding["nearest_token"]["name"] == "AppColors.primary"
        })
    }));

    Ok(())
}

#[test]
fn floating_color_components_follow_flutter_value_clamping()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'package:flutter/material.dart';

abstract final class AppColors {
  static const Color primary = Color(0xFF112233);
}

final fromComponents = Color.from(
  alpha: 2,
  red: 17 / 255,
  green: 34 / 255,
  blue: 56 / 255,
);
final fromRgba = Color.fromRGBO(17, 34, 56, 1.2);
",
    )?;

    let json = flutter_style_json(&fixture)?;
    assert_eq!(json["summary"]["raw_flutter_style_values"], 2, "{json:#}");
    assert!(json["flutter_style"].as_array().is_some_and(|findings| {
        findings.iter().all(|finding| {
            finding["kind"] == "raw-flutter-style-value"
                && finding["nearest_token"]["name"] == "AppColors.primary"
        })
    }));

    Ok(())
}

#[test]
fn unrelated_same_named_property_does_not_mark_theme_extension_token_used()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'package:flutter/material.dart';

abstract class AppTokens extends ThemeExtension<AppTokens> {
  final Color unusedTone;
  const AppTokens(this.unusedTone);
}

class Model {
  int get unusedTone => 0;
}

final unrelated = Model().unusedTone;
",
    )?;

    let json = flutter_style_json(&fixture)?;
    assert!(json["flutter_style"].as_array().is_some_and(|findings| {
        findings.iter().any(|finding| {
            finding["kind"] == "unused-theme-extension-token"
                && finding["token"]["name"] == "unusedTone"
        })
    }));

    Ok(())
}

#[test]
fn config_enables_flutter_style_for_check() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = flutter_fixture()?;
    let root = fixture.path().to_str().unwrap_or(".");
    let mut default_output = Vec::new();
    let default_code = run_from(
        ["dart-decimate", "check", root, "--format", "json"],
        &mut default_output,
    )?;
    assert_eq!(default_code, 0);
    write(
        &fixture,
        "dart-decimate.toml",
        "[health]\nflutterStyle = true\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            root,
            "--config",
            fixture
                .path()
                .join("dart-decimate.toml")
                .to_str()
                .unwrap_or("dart-decimate.toml"),
            "--format",
            "json",
        ],
        &mut output,
    )?;
    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, default_code);
    assert!(
        json["flutter_style"]
            .as_array()
            .is_some_and(|findings| !findings.is_empty())
    );
    assert!(json["findings"].as_array().is_some_and(|findings| {
        findings.iter().all(|finding| {
            !finding["rule_id"]
                .as_str()
                .is_some_and(|rule| rule.contains("theme-token") || rule.contains("flutter-style"))
                || finding["severity"] == "warning"
        })
    }));
    let mut rejected_output = Vec::new();
    assert!(
        run_from(
            ["dart-decimate", "check", root, "--flutter-style"],
            &mut rejected_output,
        )
        .is_err()
    );

    Ok(())
}

fn flutter_fixture() -> Result<TempDir, std::io::Error> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )?;
    write(
        &fixture,
        "lib/theme.dart",
        r"
import 'package:flutter/material.dart';

class AppTokens extends ThemeExtension<AppTokens> {
  final Color accent;
  final Color accentSoft;
  final Color unusedTone;
  final TextStyle title;

  const AppTokens({
    required this.accent,
    required this.accentSoft,
    required this.unusedTone,
    required this.title,
  });
}

final appTheme = ThemeData(
  colorScheme: const ColorScheme.light(
    primary: Color(0xFF112233),
    secondary: Color(0xFF334455),
    tertiary: Colors.red,
  ),
  textTheme: const TextTheme(
    titleLarge: TextStyle(fontSize: 20),
  ),
  extensions: const [
    AppTokens(
      accent: Color(0xFF778899),
      accentSoft: Color(0xFF77889A),
      unusedTone: Color(0xFF445566),
      title: TextStyle(fontSize: 18),
    ),
  ],
);
",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'package:flutter/material.dart';
import 'theme.dart';

Widget buildCard(BuildContext context) {
  final accent = Theme.of(context).extension<AppTokens>()!.accent;
  return Container(
    color: const Color(0xFF77889B),
    foregroundDecoration: const BoxDecoration(color: Colors.red),
    child: Text(
      'Hello',
      style: const TextStyle(fontSize: 19),
    ),
  );
}
void main() {}
",
    )?;
    Ok(fixture)
}

fn flutter_style_json(fixture: &TempDir) -> Result<Value, Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(
        [
            "dart-decimate",
            "health",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--flutter-style",
        ],
        &mut output,
    )?;
    assert_eq!(code, 0);
    Ok(serde_json::from_slice(&output)?)
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
