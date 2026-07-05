use std::fs;
use std::process::Command;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn dupes_command_emits_json_contract() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_duplicate_pair(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "dupes",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "dart-decimate.report.v1");
    assert_eq!(json["command"], "dupes");
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["code_duplications"], 1);
    assert_eq!(json["summary"]["duplication_analyzed_lines"], 10);
    assert_eq!(json["summary"]["duplicated_lines"], 10);
    assert_eq!(
        json["summary"]["duplication_percentage_basis_points"],
        10000
    );
    assert_eq!(json["summary"]["duplication_threshold_exceeded"], false);
    assert_eq!(
        json["clone_groups"][0]["instances"][0]["path"],
        "lib/a.dart"
    );
    assert_eq!(
        json["clone_groups"][0]["instances"][1]["path"],
        "lib/b.dart"
    );
    assert_eq!(
        json["findings"][0]["rule_id"],
        "dart-decimate/code-duplication"
    );
    assert_eq!(json["findings"][0]["kind"], "code-duplication");
    assert_eq!(
        json["findings"][0]["fingerprint"],
        json["clone_groups"][0]["fingerprint"]
    );
    assert_eq!(json["findings"][0]["severity"], "warning");
    assert_eq!(json["findings"][0]["safe_to_delete"], false);
    assert_eq!(json["findings"][0]["actions"][0]["action"], "trace-clone");
    assert_eq!(json["findings"][0]["actions"][0]["type"], "trace-clone");
    assert_eq!(
        json["findings"][0]["actions"][0]["target_path"],
        "lib/a.dart"
    );
    assert!(
        json["findings"][0]["actions"][0]["command"]
            .as_str()
            .is_some_and(|command| command
                .starts_with("dart-decimate trace-clone --format json --fingerprint dup:"))
    );
    assert_eq!(json["next_steps"][0]["id"], "trace-code-duplication");
    assert!(
        json["next_steps"][0]["command"]
            .as_str()
            .is_some_and(|command| command
                .starts_with("dart-decimate trace-clone --format json --fingerprint dup:"))
    );

    Ok(())
}

#[test]
fn dupes_threshold_fails_only_when_percentage_is_exceeded() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_duplicate_pair(&fixture)?;
    let root = fixture.path().to_str().unwrap_or(".");
    let mut passing_output = Vec::new();

    let passing_code = run_from(
        [
            "dart-decimate",
            "dupes",
            root,
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
            "--threshold",
            "100",
        ],
        &mut passing_output,
    )?;
    let passing_json = serde_json::from_slice::<Value>(&passing_output)?;
    assert_eq!(passing_code, 0);
    assert_eq!(passing_json["verdict"], "pass");
    assert_eq!(
        passing_json["summary"]["duplication_threshold_basis_points"],
        10000
    );
    assert_eq!(
        passing_json["summary"]["duplication_threshold_exceeded"],
        false
    );

    let mut failing_output = Vec::new();
    let failing_code = run_from(
        [
            "dart-decimate",
            "dupes",
            root,
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
            "--threshold",
            "99",
        ],
        &mut failing_output,
    )?;
    let failing_json = serde_json::from_slice::<Value>(&failing_output)?;
    assert_eq!(failing_code, 1);
    assert_eq!(failing_json["verdict"], "fail");
    assert_eq!(
        failing_json["summary"]["duplication_threshold_basis_points"],
        9900
    );
    assert_eq!(
        failing_json["summary"]["duplication_threshold_exceeded"],
        true
    );

    Ok(())
}

#[test]
fn dupes_threshold_can_come_from_config() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, ".dart-decimaterc", "[dupes]\nthreshold = 99\n")?;
    write_duplicate_pair(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "dupes",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["duplication_threshold_basis_points"], 9900);
    assert_eq!(json["summary"]["duplication_threshold_exceeded"], true);

    Ok(())
}

#[test]
fn dupes_cross_language_is_rejected_for_dart_only_analysis()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_duplicate_pair(&fixture)?;
    let mut output = Vec::new();

    let error = match run_from(
        [
            "dart-decimate",
            "dupes",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--cross-language",
        ],
        &mut output,
    ) {
        Ok(code) => return Err(format!("cross-language should be rejected, got {code}").into()),
        Err(error) => error,
    };
    assert_eq!(
        error.to_string(),
        "dupes --cross-language is not supported for Dart-only analysis"
    );

    Ok(())
}

#[test]
fn dupes_command_accepts_ignore_imports_alias_as_positive_override()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        ".dart-decimaterc",
        "[dupes]\nignore_imports = false\n",
    )?;
    write_import_only_duplicate_pair(&fixture)?;
    let root = fixture.path().to_str().unwrap_or(".");
    let mut counted_output = Vec::new();

    let counted_code = run_from(
        [
            "dart-decimate",
            "dupes",
            root,
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "5",
        ],
        &mut counted_output,
    )?;
    let counted_json = serde_json::from_slice::<Value>(&counted_output)?;
    assert_eq!(counted_code, 0);
    assert_eq!(counted_json["summary"]["code_duplications"], 1);

    let mut ignored_output = Vec::new();
    let ignored_code = run_from(
        [
            "dart-decimate",
            "dupes",
            root,
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "5",
            "--ignore-imports",
        ],
        &mut ignored_output,
    )?;

    let ignored_json = serde_json::from_slice::<Value>(&ignored_output)?;
    assert_eq!(ignored_code, 0);
    assert_eq!(ignored_json["summary"]["code_duplications"], 0);
    assert_eq!(
        ignored_json["clone_groups"].as_array().map(Vec::len),
        Some(0)
    );

    Ok(())
}

#[test]
fn check_command_includes_code_duplication_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_duplicate_pair(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(findings) = json["findings"].as_array() else {
        panic!("findings array");
    };
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["code_duplications"], 1);
    assert!(
        findings
            .iter()
            .any(|finding| finding["rule_id"] == "dart-decimate/code-duplication")
    );

    Ok(())
}

#[test]
fn dupes_ignores_interface_declaration_clones() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    let declarations = "\
  Future<List<Item>> loadItems(String ownerId);
  Future<Item?> findItem(String id);
  Future<void> saveItem(Item item);
  Stream<List<Item>> watchItems(String ownerId);
  Future<void> deleteItem(String id);
";
    write(
        &fixture,
        "lib/repository.dart",
        &format!("abstract interface class ItemRepository {{\n{declarations}}}\nclass Item {{}}\n"),
    )?;
    write(
        &fixture,
        "lib/datasource.dart",
        &format!("abstract interface class ItemDatasource {{\n{declarations}}}\n"),
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "dupes",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["code_duplications"], 0);
    assert_eq!(json["clone_groups"].as_array().map(Vec::len), Some(0));

    Ok(())
}

#[test]
fn dupes_reports_executable_call_blocks_that_look_like_signatures()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    let calls = "\
  analytics.trackView(userId);
  analytics.identifyUser(userId);
  analytics.setUserProperty(userId);
  analytics.flushEvents(userId);
  analytics.trackComplete(userId);
";
    write(
        &fixture,
        "lib/a.dart",
        &format!("void first(analytics, userId) {{\n{calls}}}\n"),
    )?;
    write(
        &fixture,
        "lib/b.dart",
        &format!("void second(analytics, userId) {{\n{calls}}}\n"),
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "dupes",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["code_duplications"], 1);
    assert_eq!(json["clone_groups"].as_array().map(Vec::len), Some(1));

    Ok(())
}

#[test]
fn dupes_collapses_copied_local_package_mirrors_to_canonical_source_clone()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    let source = pagination_clone_source();
    write(&fixture, "functions/shared/pubspec.yaml", "name: shared\n")?;
    write(&fixture, "functions/shared/lib/pagination.dart", source)?;
    for function in ["delete_user", "squad_operations"] {
        write(
            &fixture,
            &format!("functions/{function}/shared/pubspec.yaml"),
            "name: shared\n",
        )?;
        write(
            &fixture,
            &format!("functions/{function}/shared/lib/pagination.dart"),
            source,
        )?;
    }
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "dupes",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "8",
            "--min-tokens",
            "20",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["code_duplications"], 1);
    let instances = json["clone_groups"][0]["instances"].as_array().unwrap();
    assert_eq!(instances.len(), 2);
    assert!(instances.iter().all(|instance| {
        instance["path"] == "functions/shared/lib/pagination.dart"
            && instance["start_line"].as_u64().is_some_and(|line| line > 1)
    }));
    assert!(instances.iter().all(|instance| {
        !instance["path"].as_str().is_some_and(|path| {
            path.contains("/delete_user/") || path.contains("/squad_operations/")
        })
    }));

    Ok(())
}

#[test]
fn dupes_collapses_shifted_copied_package_mirrors_to_canonical_source_ranges()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    let source = pagination_clone_source();
    write(&fixture, "functions/shared/pubspec.yaml", "name: shared\n")?;
    write(&fixture, "functions/shared/lib/pagination.dart", source)?;
    for function in ["delete_user", "squad_operations"] {
        write(
            &fixture,
            &format!("functions/{function}/shared/pubspec.yaml"),
            "name: shared\n",
        )?;
        write(
            &fixture,
            &format!("functions/{function}/shared/lib/pagination.dart"),
            &format!("// mirrored package header\n{source}"),
        )?;
    }
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "dupes",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "8",
            "--min-tokens",
            "20",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["code_duplications"], 1);
    let instances = json["clone_groups"][0]["instances"].as_array().unwrap();
    assert_eq!(instances.len(), 2);
    assert!(
        instances
            .iter()
            .all(|instance| instance["path"] == "functions/shared/lib/pagination.dart")
    );
    let starts = instances
        .iter()
        .map(|instance| instance["start_line"].as_u64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(starts, vec![2, 15]);

    Ok(())
}

#[test]
fn workspace_scope_prunes_clone_group_instances() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: root\nworkspace:\n  - packages/*\n",
    )?;
    write(&fixture, "packages/app/pubspec.yaml", "name: app\n")?;
    write(&fixture, "packages/shared/pubspec.yaml", "name: shared\n")?;
    let source = "void shared() {\n  final items = [1, 2, 3];\n  final active = items.where((item) => item > 1);\n  print(active.length);\n}\n";
    write(&fixture, "packages/app/lib/a.dart", source)?;
    write(&fixture, "packages/shared/lib/b.dart", source)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "dupes",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
            "--workspace",
            "app",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["code_duplications"], 1);
    assert_eq!(
        json["clone_groups"][0]["instances"][0]["path"],
        "packages/app/lib/a.dart"
    );
    assert_eq!(
        json["clone_groups"][0]["instances"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );

    Ok(())
}

#[test]
fn changed_workspaces_scope_prunes_clone_group_instances() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = git_fixture()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: root\nworkspace:\n  - packages/*\n",
    )?;
    write(&fixture, "packages/app/pubspec.yaml", "name: app\n")?;
    write(&fixture, "packages/shared/pubspec.yaml", "name: shared\n")?;
    let source = "void shared() {\n  final items = [1, 2, 3];\n  final active = items.where((item) => item > 1);\n  print(active.length);\n}\n";
    write(&fixture, "packages/app/lib/a.dart", source)?;
    write(&fixture, "packages/shared/lib/b.dart", source)?;
    commit_all(&fixture)?;
    write(&fixture, "packages/app/README.md", "changed\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "dupes",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
            "--changed-workspaces",
            "HEAD",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["code_duplications"], 1);
    assert_eq!(
        json["clone_groups"][0]["instances"][0]["path"],
        "packages/app/lib/a.dart"
    );
    assert_eq!(
        json["clone_groups"][0]["instances"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );

    Ok(())
}

#[test]
fn trace_clone_command_reports_matching_group() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_duplicate_pair(&fixture)?;
    let mut dupes_output = Vec::new();
    run_from(
        [
            "dart-decimate",
            "dupes",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
        ],
        &mut dupes_output,
    )?;
    let dupes_json = serde_json::from_slice::<Value>(&dupes_output)?;
    let Some(fingerprint) = dupes_json["clone_groups"][0]["fingerprint"].as_str() else {
        panic!("fingerprint string");
    };
    let mut trace_output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "trace-clone",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
            "--fingerprint",
            fingerprint,
        ],
        &mut trace_output,
    )?;

    let json = serde_json::from_slice::<Value>(&trace_output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "dart-decimate.trace.v1");
    assert_eq!(json["kind"], "trace-clone");
    assert_eq!(json["command"], "trace-clone");
    assert_eq!(json["found"], true);
    assert_eq!(json["clone_groups"][0]["fingerprint"], fingerprint);
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

fn write_duplicate_pair(fixture: &TempDir) -> Result<(), std::io::Error> {
    let source = "void shared() {\n  final items = [1, 2, 3];\n  final active = items.where((item) => item > 1);\n  print(active.length);\n}\n";
    write(fixture, "lib/a.dart", source)?;
    write(fixture, "lib/b.dart", source)
}

fn write_import_only_duplicate_pair(fixture: &TempDir) -> Result<(), std::io::Error> {
    let source = "import 'dart:async';\nimport 'dart:collection';\nimport 'dart:convert';\nimport 'dart:io';\nimport 'dart:math';\n";
    write(fixture, "lib/a.dart", source)?;
    write(fixture, "lib/b.dart", source)
}

fn pagination_clone_source() -> &'static str {
    r"Future<List<String>> fetchAllRowIds(Pages pages) async {
  final values = <String>[];
  var cursor = '';
  do {
    final page = await pages.list(cursor: cursor);
    for (final row in page.rows) {
      values.add(row.id);
    }
    cursor = page.nextCursor;
  } while (cursor.isNotEmpty);
  return values;
}

Future<List<String>> fetchAllFieldValues(Pages pages) async {
  final values = <String>[];
  var cursor = '';
  do {
    final page = await pages.list(cursor: cursor);
    for (final row in page.rows) {
      values.add(row.id);
    }
    cursor = page.nextCursor;
  } while (cursor.isNotEmpty);
  return values;
}
"
}

fn git_fixture() -> Result<TempDir, Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    git(&fixture, ["init", "-q"])?;
    git(
        &fixture,
        ["config", "user.email", "dart-decimate@example.com"],
    )?;
    git(&fixture, ["config", "user.name", "Dart Decimate Tests"])?;
    Ok(fixture)
}

fn commit_all(fixture: &TempDir) -> Result<(), Box<dyn std::error::Error>> {
    git(fixture, ["add", "."])?;
    git(fixture, ["commit", "-m", "initial", "-q"])
}

fn git<const N: usize>(
    fixture: &TempDir,
    args: [&str; N],
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(fixture.path())
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )
    .into())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
