use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn large_flutter_build_methods_are_advisory_inventory() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        &large_build_method_source("Dashboard", "StatelessWidget", 61),
    )?;
    let root = fixture.path().to_str().unwrap_or(".");
    let mut json_output = Vec::new();

    let code = run_from(
        ["dart-decimate", "health", root, "--format", "json"],
        &mut json_output,
    )?;

    let json = serde_json::from_slice::<Value>(&json_output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["findings"], 0);
    assert_eq!(json["summary"]["large_functions"], 1);
    assert_eq!(
        json["large_functions"][0]["rule_id"],
        "dart-decimate/large-function"
    );
    assert_eq!(json["large_functions"][0]["kind"], "flutter-build-method");
    assert_eq!(json["large_functions"][0]["line_count"], 61);
    assert_eq!(json["large_functions"][0]["max_unit_size"], 60);
    assert!(
        json["large_functions"][0]["guidance"]
            .as_str()
            .is_some_and(|guidance| guidance.contains("Flutter build method"))
    );

    let mut human_output = Vec::new();
    let human_code = run_from(["dart-decimate", "health", root], &mut human_output)?;
    let human = String::from_utf8(human_output)?;
    assert_eq!(human_code, 0);
    assert!(human.contains("Large Functions (advisory)"));
    assert!(human.contains("build"));
    assert!(human.contains("Flutter build method"));

    Ok(())
}

fn large_build_method_source(class_name: &str, base: &str, lines: usize) -> String {
    let mut source = vec![
        format!("class {class_name} extends {base} {{"),
        "  Widget build(BuildContext context) {".to_owned(),
    ];
    source.extend(
        (0..lines.saturating_sub(2)).map(|index| format!("    final value{index} = {index};")),
    );
    source.push("  }".to_owned());
    source.push("}".to_owned());
    source.join("\n")
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
