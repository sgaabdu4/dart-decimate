use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
#[allow(clippy::too_many_lines)]
fn theme_extension_aliases_and_proven_factories_mark_owned_fields_used()
-> Result<(), Box<dyn std::error::Error>> {
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
  final Color info;
  final Color success;
  final Color warning;
  final Color loopTone;
  final Color collisionTone;
  final Color unusedTone;

  const AppTokens({
    required this.info,
    required this.success,
    required this.warning,
    required this.loopTone,
    required this.collisionTone,
    required this.unusedTone,
  });

  static AppTokens of(BuildContext context) =>
      Theme.of(context).extension<AppTokens>()!;
}

class AppTheme {
  static AppTokens colors(BuildContext context) {
    return Theme.of(context).extension<AppTokens>()!;
  }
}

class CollisionFactory {
  static AppTokens colors(BuildContext context) =>
      Theme.of(context).extension<AppTokens>()!;
}
",
    )?;
    write(
        &fixture,
        "lib/collision.dart",
        r"
import 'package:flutter/material.dart';

class Model {
  Color get collisionTone => Colors.black;
}

class CollisionFactory {
  static Model colors(BuildContext context) => Model();
}
",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'package:flutter/material.dart';
import 'collision.dart' as collision;
import 'theme.dart';

class Model {
  Color get unusedTone => Colors.black;
  Color get loopTone => Colors.black;
}

class WrongFactory {
  static Model colors(BuildContext context) {
    Theme.of(context).extension<AppTokens>();
    return Model();
  }
}

Widget buildCard(BuildContext context) {
  final colors = Theme.of(context).extension<AppTokens>() ??
      const AppTokens(
        info: Colors.blue,
        success: Colors.green,
        warning: Colors.orange,
        loopTone: Colors.purple,
        collisionTone: Colors.teal,
        unusedTone: Colors.black,
      );
  final directFactory = AppTokens.of(context);
  final wrappedFactory = AppTheme.colors(context);
  final wrongFactory = WrongFactory.colors(context);
  final loop = Model();
  for (
    final loop = Theme.of(context).extension<AppTokens>()!;
    false;
  ) {}
  print(loop.loopTone);
  final collisionFactory = collision.CollisionFactory.colors(context);
  print(collisionFactory.collisionTone);
  {
    final colors = Model();
    print(colors.unusedTone);
  }
  return ColoredBox(
    color: colors.info,
    child: ColoredBox(
      color: directFactory.success,
      child: ColoredBox(
        color: wrappedFactory.warning,
        child: ColoredBox(color: wrongFactory.unusedTone),
      ),
    ),
  );
}
",
    )?;

    let json = flutter_style_json(&fixture)?;
    let unused = json["flutter_style"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|finding| finding["kind"] == "unused-theme-extension-token")
        .filter_map(|finding| finding["token"]["name"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        unused,
        ["loopTone", "collisionTone", "unusedTone"],
        "{json:#}"
    );

    Ok(())
}

#[test]
fn duplicate_extension_names_preserve_each_declaration() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )?;
    write(
        &fixture,
        "lib/first.dart",
        r"
import 'package:flutter/material.dart';

class SharedTokens extends ThemeExtension<SharedTokens> {
  final Color firstOnly;
  const SharedTokens(this.firstOnly);
}
",
    )?;
    write(
        &fixture,
        "lib/second.dart",
        r"
import 'package:flutter/material.dart';

class SharedTokens extends ThemeExtension<SharedTokens> {
  final TextStyle secondOnly;
  const SharedTokens(this.secondOnly);
}
",
    )?;

    let json = flutter_style_json(&fixture)?;
    let unused = json["flutter_style"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|finding| finding["kind"] == "unused-theme-extension-token")
        .filter_map(|finding| {
            Some((
                finding["path"].as_str()?,
                finding["token"]["name"].as_str()?,
            ))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        unused,
        [
            ("lib/first.dart", "firstOnly"),
            ("lib/second.dart", "secondOnly"),
        ],
        "{json:#}"
    );

    Ok(())
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
