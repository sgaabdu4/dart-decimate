use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn typedef_platform_shadow_is_a_process_candidate() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'dart:io';\n\
typedef Platform = FakePlatform;\n\
class FakePlatform {}\n\
Future<Process> start() =>\n\
    Process.start(Platform.resolvedExecutable, const ['/path/to/snapshot']);\n",
    )?;

    let json = security(&fixture)?;
    assert_eq!(
        candidate(&json, "process-execution")["occurrences"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    Ok(())
}

#[test]
fn part_library_shadow_is_a_process_candidate() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'dart:io';\n\
part 'shadow.dart';\n\
final dartExe = Platform.resolvedExecutable;\n\
Future<Process> start() => Process.start(dartExe, const ['/path/to/snapshot']);\n",
    )?;
    write(
        &fixture,
        "lib/shadow.dart",
        "part of 'main.dart';\n\
class Platform {\n\
  static String get resolvedExecutable => 'dynamic';\n\
}\n",
    )?;

    let json = security(&fixture)?;
    assert_eq!(
        candidate(&json, "process-execution")["occurrences"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    Ok(())
}

#[test]
fn mixin_member_shadow_is_a_process_candidate() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'dart:io';\n\
final dartExe = Platform.resolvedExecutable;\n\
mixin RuntimeOverride {\n\
  String get dartExe => 'dynamic';\n\
}\n\
class Runner with RuntimeOverride {\n\
  Future<Process> start() => Process.start(dartExe, const ['/path/to/snapshot']);\n\
}\n",
    )?;

    let json = security(&fixture)?;
    assert_eq!(
        candidate(&json, "process-execution")["occurrences"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    Ok(())
}

#[test]
fn typed_and_variable_shell_arguments_are_process_candidates()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'dart:io';\n\
Future<ProcessResult> typed(String command) {\n\
  final typedArgs = <String>['-c', command];\n\
  return Process.run('/bin/bash', typedArgs, runInShell: false);\n\
}\n\
Future<ProcessResult> variable(String command) {\n\
  final args = ['-c', command];\n\
  return Process.run('/bin/bash', args, runInShell: false);\n\
}\n",
    )?;

    let json = security(&fixture)?;
    assert_eq!(
        candidate(&json, "process-execution")["occurrences"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    Ok(())
}

#[test]
fn direct_oauth_http_endpoint_is_insecure_transport_candidate()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "const tokenEndpoint = 'http://login.acme.com/oauth2/token';\n",
    )?;

    let json = security(&fixture)?;
    assert_eq!(
        candidate(&json, "insecure-transport")["occurrences"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    assert!(
        json["security_candidates"]
            .as_array()
            .is_some_and(|candidates| candidates
                .iter()
                .all(|candidate| candidate["category"] != "hardcoded-secret"))
    );
    Ok(())
}

#[test]
fn trace_codegen_evidence_is_distinct_from_script_evidence()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  json_annotation: ^4.0.0\n  riverpod_annotation: ^2.0.0\n\
dev_dependencies:\n  json_serializable: ^6.0.0\n  riverpod_generator: ^2.0.0\n",
    )?;
    write(
        &fixture,
        "lib/provider.dart",
        "import 'package:riverpod_annotation/riverpod_annotation.dart';\n\
part 'provider.g.dart';\n\
@riverpod\nString provider(Ref ref) => 'value';\n",
    )?;

    let json_trace = trace_dependency(&fixture, "json_serializable")?;
    assert_eq!(json_trace["is_used"], false, "{json_trace:#}");
    assert_eq!(json_trace["used_in_scripts"], false, "{json_trace:#}");
    assert_eq!(json_trace["used_in_codegen"], false, "{json_trace:#}");

    let riverpod_trace = trace_dependency(&fixture, "riverpod_generator")?;
    assert_eq!(riverpod_trace["is_used"], true, "{riverpod_trace:#}");
    assert_eq!(
        riverpod_trace["used_in_scripts"], false,
        "{riverpod_trace:#}"
    );
    assert_eq!(
        riverpod_trace["used_in_codegen"], true,
        "{riverpod_trace:#}"
    );
    Ok(())
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

fn trace_dependency(
    fixture: &TempDir,
    dependency: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    run_json([
        "dart-decimate",
        "trace-dependency",
        root(fixture),
        "--format",
        "json",
        "--dependency",
        dependency,
    ])
}

fn run_json<const N: usize>(args: [&str; N]) -> Result<Value, Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let _code = run_from(args, &mut output)?;
    Ok(serde_json::from_slice(&output)?)
}

fn candidate<'a>(json: &'a Value, category: &str) -> &'a Value {
    json["security_candidates"]
        .as_array()
        .and_then(|candidates| {
            candidates
                .iter()
                .find(|candidate| candidate["category"] == category)
        })
        .unwrap_or_else(|| panic!("missing {category} candidate: {json:#}"))
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
