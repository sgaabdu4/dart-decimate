use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn primary_constructor_private_type_leaks_are_reported() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "\
class Api(final _Hidden hidden);
class _Hidden {}
",
    )?;

    let (code, json) = run_json([
        "dart-decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/package.dart",
        "--private-type-leaks",
    ])?;

    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "dart-decimate/private-type-leak")
    }) else {
        panic!("primary constructor private type leak finding");
    };
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["private_type_leaks"], 1);
    assert_eq!(finding["kind"], "private-type-leak");
    assert_eq!(finding["path"], "lib/package.dart");
    assert_eq!(finding["line"], 1);
    assert_eq!(finding["safe_to_delete"], false);
    assert_eq!(finding["actions"][0]["action"], "review-public-api");
    assert_eq!(finding["actions"][0]["target_symbol"], "Api");

    Ok(())
}

#[test]
fn dart_3_13_syntax_is_accepted_across_cli_surfaces() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: modern_app\nenvironment:\n  sdk: ^3.13.0\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
enum Axis { center }

class Point(final int x, final int y);

class Box {
  new() {}
  new named() : this();
  const new empty();
}

class FactoryBox {
  factory() => Child();
  factory namedFactory() { return Child(); }
  factory redirect() = Child;
  const factory cached() = Child.cached;
}

class Child implements FactoryBox {
  Child();
  const Child.cached();
}

extension type UserId(int value) {}

Axis alignment = .center;
const axes = <Axis>[.center];

void main() {
  Point(1, 2);
  Box.named();
  UserId(1);
}
",
    )?;

    for args in [
        vec![
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
        vec![
            "dart-decimate",
            "inspect",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
            "--file",
            "lib/main.dart",
        ],
    ] {
        let mut output = Vec::new();
        let code = run_from(args, &mut output)?;
        assert!(code <= 1);
        let json = serde_json::from_slice::<Value>(&output)?;
        assert_ne!(json["error"], true);
    }

    let mut output = Vec::new();
    let code = run_from(
        [
            "dart-decimate",
            "human",
            fixture.path().to_str().unwrap_or("."),
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;
    assert!(code <= 1);
    assert!(String::from_utf8(output)?.contains("Dart Decimate check:"));

    Ok(())
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

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
