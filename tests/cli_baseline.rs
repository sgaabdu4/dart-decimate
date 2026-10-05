use std::fs;
use std::path::Path;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn check_command_saves_identity_baseline() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'live.dart';\nvoid main() {}\n",
    )?;
    write(&fixture, "lib/live.dart", "void live() {}\n")?;
    write(&fixture, "lib/dead.dart", "// dead file\n")?;
    write_duplicate_pair(&fixture)?;
    let baseline_path = fixture.path().join(".dart-decimate/baseline.json");

    let (code, report) = run_json(vec![
        "dart-decimate",
        "check",
        &root(&fixture),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--min-lines",
        "5",
        "--min-tokens",
        "10",
        "--save-baseline",
        &path_arg(&baseline_path),
    ])?;

    let baseline = read_json(&baseline_path)?;
    assert_eq!(code, 1);
    assert_eq!(report["schema_version"], "dart-decimate.report.v1");
    assert_eq!(baseline["schema_version"], "dart-decimate.baseline.v1");
    assert_eq!(baseline["tool"], "dart-decimate");
    assert!(baseline_has_rule(&baseline, "dart-decimate/dead-file"));
    assert!(baseline_has_rule(
        &baseline,
        "dart-decimate/code-duplication"
    ));
    assert_eq!(
        baseline_fingerprint(&baseline, "dart-decimate/code-duplication"),
        report["clone_groups"][0]["fingerprint"]
    );

    Ok(())
}

#[test]
fn check_command_with_baseline_passes_when_all_findings_are_known()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(&fixture, "lib/dead.dart", "// dead file\n")?;
    let baseline_path = fixture.path().join("dart-decimate-baseline.json");
    let root = root(&fixture);
    let baseline = path_arg(&baseline_path);

    let (save_code, _) = run_json(vec![
        "dart-decimate",
        "check",
        &root,
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--save-baseline",
        &baseline,
    ])?;
    let (code, report) = run_json(vec![
        "dart-decimate",
        "check",
        &root,
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--baseline",
        &baseline,
    ])?;

    assert_eq!(save_code, 1);
    assert_eq!(code, 0);
    assert_eq!(report["verdict"], "pass");
    assert_eq!(report["summary"]["findings"], 0);
    assert!(report["findings"].as_array().is_some_and(Vec::is_empty));
    assert!(
        !report["next_steps"]
            .as_array()
            .is_some_and(|steps| steps.iter().any(|step| step["id"] == "trace-unused-export"))
    );

    Ok(())
}

#[test]
fn check_baseline_suppresses_known_unused_member() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/live.dart';\nvoid main() { runLive(); }\n",
    )?;
    write(
        &fixture,
        "lib/src/live.dart",
        "class Live { void _unused() {} }\nvoid runLive() { Live(); }\n",
    )?;
    let baseline_path = fixture.path().join("dart-decimate-baseline.json");
    let root = root(&fixture);
    let baseline = path_arg(&baseline_path);

    let (save_code, saved) = run_json(vec![
        "dart-decimate",
        "check",
        &root,
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--save-baseline",
        &baseline,
    ])?;
    let (code, report) = run_json(vec![
        "dart-decimate",
        "check",
        &root,
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--baseline",
        &baseline,
    ])?;

    assert_eq!(save_code, 1);
    assert_eq!(saved["summary"]["unused_class_members"], 1);
    assert_eq!(code, 0);
    assert_eq!(report["verdict"], "pass");
    assert_eq!(report["summary"]["unused_class_members"], 0);
    assert_eq!(report["summary"]["findings"], 0);
    assert!(report["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn check_command_with_baseline_reports_only_new_identity() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(&fixture, "lib/dead.dart", "// dead file\n")?;
    let baseline_path = fixture.path().join("dart-decimate-baseline.json");
    let root = root(&fixture);
    let baseline = path_arg(&baseline_path);

    run_json(vec![
        "dart-decimate",
        "check",
        &root,
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--save-baseline",
        &baseline,
    ])?;
    write(&fixture, "lib/new_dead.dart", "// new dead file\n")?;

    let (code, report) = run_json(vec![
        "dart-decimate",
        "check",
        &root,
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--baseline",
        &baseline,
    ])?;

    assert_eq!(code, 1);
    assert_eq!(report["summary"]["findings"], 1);
    assert_eq!(report["findings"][0]["path"], "lib/new_dead.dart");
    assert_eq!(report["findings"][0]["rule_id"], "dart-decimate/dead-file");

    Ok(())
}

#[test]
fn health_baseline_identity_survives_line_shift() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_complex_source(&fixture, "lib/main.dart", "route")?;
    let baseline_path = fixture.path().join("health-baseline.json");
    let root = root(&fixture);
    let baseline = path_arg(&baseline_path);

    run_json(health_args(&root, &["--save-baseline", &baseline]))?;
    let original = fs::read_to_string(fixture.path().join("lib/main.dart"))?;
    write(
        &fixture,
        "lib/main.dart",
        &format!("// shifted by one line\n{original}"),
    )?;

    let (code, report) = run_json(health_args(&root, &["--baseline", &baseline]))?;

    assert_eq!(code, 0);
    assert_eq!(report["verdict"], "pass");
    assert_eq!(report["summary"]["findings"], 0);
    assert!(report["complexity"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn health_baseline_does_not_suppress_changed_symbol() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_complex_source(&fixture, "lib/main.dart", "route")?;
    let baseline_path = fixture.path().join("health-baseline.json");
    let root = root(&fixture);
    let baseline = path_arg(&baseline_path);

    run_json(health_args(&root, &["--save-baseline", &baseline]))?;
    write_complex_source(&fixture, "lib/main.dart", "differentRoute")?;

    let (code, report) = run_json(health_args(&root, &["--baseline", &baseline]))?;

    assert_eq!(code, 1);
    assert_eq!(report["summary"]["findings"], 1);
    assert_eq!(report["findings"][0]["path"], "lib/main.dart");
    assert_eq!(report["complexity"][0]["symbol"], "differentRoute");

    Ok(())
}

#[test]
fn baseline_errors_for_missing_or_malformed_file() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let missing = fixture.path().join("missing-baseline.json");
    let malformed = fixture.path().join("malformed-baseline.json");
    fs::write(&malformed, "{ not json")?;

    let missing_error = run_error(vec![
        "dart-decimate",
        "check",
        &root(&fixture),
        "--format",
        "json",
        "--baseline",
        &path_arg(&missing),
    ]);
    let malformed_error = run_error(vec![
        "dart-decimate",
        "check",
        &root(&fixture),
        "--format",
        "json",
        "--baseline",
        &path_arg(&malformed),
    ]);

    assert!(missing_error.contains("failed to read baseline"));
    assert!(missing_error.contains("missing-baseline.json"));
    assert!(malformed_error.contains("failed to parse baseline"));
    assert!(malformed_error.contains("malformed-baseline.json"));

    Ok(())
}

#[test]
fn check_command_saves_regression_baseline_counts() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(&fixture, "lib/dead.dart", "// dead file\n")?;
    let baseline_path = fixture.path().join(".dart-decimate/regression.json");

    let (code, report) = run_json(vec![
        "dart-decimate",
        "check",
        &root(&fixture),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
        "--save-regression-baseline",
        &path_arg(&baseline_path),
    ])?;

    let baseline = read_json(&baseline_path)?;
    assert_eq!(code, 1);
    assert_eq!(report["summary"]["findings"], 1);
    assert_eq!(
        baseline["schema_version"],
        "dart-decimate.regression-baseline.v1"
    );
    assert_eq!(baseline["tool"], "dart-decimate");
    assert_eq!(baseline["command"], "check");
    assert_eq!(baseline["counts"]["findings"], 1);
    assert_eq!(baseline["counts"]["rules"]["dart-decimate/dead-file"], 1);

    Ok(())
}

#[test]
fn fail_on_regression_passes_when_counts_match() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = regression_fixture()?;
    let baseline_path = fixture.path().join("regression.json");
    let root = root(&fixture);
    let baseline = path_arg(&baseline_path);

    run_json(regression_args(
        &root,
        &["--save-regression-baseline", &baseline],
    ))?;
    let (code, report) = run_json(regression_args(
        &root,
        &[
            "--regression-baseline",
            &baseline,
            "--fail-on-regression",
            "--tolerance",
            "0",
        ],
    ))?;

    assert_eq!(code, 0);
    assert_eq!(report["verdict"], "fail");
    assert_eq!(report["summary"]["findings"], 1);

    Ok(())
}

#[test]
fn fail_on_regression_fails_when_counts_increase() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = regression_fixture()?;
    let baseline_path = fixture.path().join("regression.json");
    let root = root(&fixture);
    let baseline = path_arg(&baseline_path);

    run_json(regression_args(
        &root,
        &["--save-regression-baseline", &baseline],
    ))?;
    write(&fixture, "lib/new_dead.dart", "// new dead file\n")?;
    let (code, report) = run_json(regression_args(
        &root,
        &[
            "--regression-baseline",
            &baseline,
            "--fail-on-regression",
            "--tolerance",
            "0",
        ],
    ))?;

    assert_eq!(code, 1);
    assert_eq!(report["summary"]["findings"], 2);

    Ok(())
}

#[test]
fn regression_tolerance_allows_absolute_and_percent_growth()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(&fixture, "lib/dead_one.dart", "// dead\n")?;
    write(&fixture, "lib/dead_two.dart", "// dead\n")?;
    let baseline_path = fixture.path().join("regression.json");
    let root = root(&fixture);
    let baseline = path_arg(&baseline_path);

    run_json(regression_args(
        &root,
        &["--save-regression-baseline", &baseline],
    ))?;
    write(&fixture, "lib/dead_three.dart", "// dead\n")?;
    let (absolute_code, _) = run_json(regression_args(
        &root,
        &[
            "--regression-baseline",
            &baseline,
            "--fail-on-regression",
            "--tolerance",
            "1",
        ],
    ))?;
    let (percent_code, _) = run_json(regression_args(
        &root,
        &[
            "--regression-baseline",
            &baseline,
            "--fail-on-regression",
            "--tolerance",
            "50%",
        ],
    ))?;

    assert_eq!(absolute_code, 0);
    assert_eq!(percent_code, 0);

    Ok(())
}

#[test]
fn regression_baseline_errors_for_missing_or_bad_tolerance()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = regression_fixture()?;
    let missing = fixture.path().join("missing-regression.json");
    let missing_error = run_error(regression_args(
        &root(&fixture),
        &[
            "--regression-baseline",
            &path_arg(&missing),
            "--fail-on-regression",
        ],
    ));
    let tolerance_error = run_error(regression_args(
        &root(&fixture),
        &["--fail-on-regression", "--tolerance", "many"],
    ));

    assert!(missing_error.contains("failed to read baseline"));
    assert!(missing_error.contains("missing-regression.json"));
    assert!(tolerance_error.contains("invalid regression tolerance"));

    Ok(())
}

#[test]
fn audit_rejects_global_regression_baseline_flags() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let mut output = Vec::new();

    let error = run_from(
        [
            "dart-decimate",
            "audit",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--base",
            "HEAD",
            "--save-regression-baseline",
            "regression.json",
        ],
        &mut output,
    )
    .err()
    .map(|error| error.to_string())
    .unwrap_or_default();

    assert_eq!(output, b"");
    assert!(
        error.contains("unexpected argument")
            || error.contains("unrecognized")
            || error.contains("save-regression-baseline")
    );

    Ok(())
}

fn run_json(args: Vec<&str>) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(args, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    Ok((code, json))
}

fn run_error(args: Vec<&str>) -> String {
    let mut output = Vec::new();
    let error = run_from(args, &mut output)
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert_eq!(output, b"");
    error
}

fn read_json(path: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    let bytes = fs::read(path)?;
    Ok(serde_json::from_slice::<Value>(&bytes)?)
}

fn baseline_has_rule(baseline: &Value, rule_id: &str) -> bool {
    baseline["findings"]
        .as_array()
        .is_some_and(|findings| findings.iter().any(|finding| finding["rule_id"] == rule_id))
}

fn baseline_fingerprint(baseline: &Value, rule_id: &str) -> Value {
    baseline["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|finding| finding["rule_id"] == rule_id)
        .map_or(Value::Null, |finding| finding["fingerprint"].clone())
}

fn regression_fixture() -> Result<TempDir, std::io::Error> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(&fixture, "lib/dead.dart", "// dead file\n")?;
    Ok(fixture)
}

fn regression_args<'a>(root: &'a str, tail: &'a [&'a str]) -> Vec<&'a str> {
    let mut args = vec![
        "dart-decimate",
        "check",
        root,
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
    ];
    args.extend_from_slice(tail);
    args
}

fn health_args<'a>(root: &'a str, tail: &'a [&'a str]) -> Vec<&'a str> {
    let mut args = vec![
        "dart-decimate",
        "health",
        root,
        "--format",
        "json",
        "--max-cyclomatic",
        "3",
        "--max-cognitive",
        "3",
    ];
    args.extend_from_slice(tail);
    args
}

fn write_duplicate_pair(fixture: &TempDir) -> Result<(), std::io::Error> {
    let source = "void shared() {\n  final items = [1, 2, 3];\n  final active = items.where((item) => item > 1);\n  print(active.length);\n}\n";
    write(fixture, "lib/a.dart", source)?;
    write(fixture, "lib/b.dart", source)
}

fn write_complex_source(fixture: &TempDir, path: &str, symbol: &str) -> Result<(), std::io::Error> {
    write(
        fixture,
        path,
        &format!(
            "void calm() {{}}

String {symbol}(List<int> items) {{
  if (items.isEmpty) return 'none';
  for (final item in items) {{
    if (item.isEven) return 'even';
  }}
  return 'odd';
}}
"
        ),
    )
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}

fn root(fixture: &TempDir) -> String {
    fixture.path().display().to_string()
}

fn path_arg(path: &Path) -> String {
    path.display().to_string()
}
