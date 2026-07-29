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
fn part_library_without_shadow_preserves_fixed_dart_runtime_exemption()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'dart:io';\n\
part 'support.dart';\n\
final dartExe = Platform.resolvedExecutable;\n\
Future<Process> start() => Process.start(dartExe, const ['/path/to/snapshot']);\n",
    )?;
    write(
        &fixture,
        "lib/support.dart",
        "part of 'main.dart';\n\
String describe() => 'support';\n",
    )?;

    let json = security(&fixture)?;
    assert!(
        json["security_candidates"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "{json:#}"
    );
    assert!(
        json["security_blind_spots"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "{json:#}"
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
fn synchronous_dynamic_process_execution_is_a_candidate() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'dart:io';\n\
ProcessResult run(String bin, List<String> args) =>\n\
    Process.runSync(bin, args, runInShell: true);\n",
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
fn process_detection_requires_real_dart_io_provenance_and_tracks_dynamic_arguments()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'dart:io' as io;

const documentation = 'Process.runSync(command, arguments)';

class Process {
  static void runSync(String executable, List<String> arguments) {}
}

io.ProcessResult runReal(List<String> arguments) =>
    io.Process.runSync('/usr/bin/tool', arguments, runInShell: false);

io.ProcessResult runSafe() =>
    io.Process.runSync('/bin/echo', const ['ok']);

const shellDisabled = false;

io.ProcessResult runSafeWithConstShellFlag() =>
    io.Process.runSync('/bin/echo', const ['ok'], runInShell: shellDisabled);

void runLocal(List<String> arguments) =>
    Process.runSync('/usr/bin/tool', arguments);
",
    )?;

    let json = security(&fixture)?;
    let occurrences = candidate(&json, "process-execution")["occurrences"]
        .as_array()
        .ok_or("process occurrences")?;
    assert_eq!(occurrences.len(), 1, "{json:#}");
    assert_eq!(occurrences[0]["expression"], "Process.runSync");
    assert!(
        occurrences[0]["evidence"]
            .as_str()
            .is_some_and(|evidence| evidence.contains("io.Process.runSync"))
    );

    Ok(())
}

#[test]
fn process_manager_detection_resolves_typed_receivers() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  process: ^5.0.5\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'package:process/process.dart';
import 'package:flutter/material.dart';

final ProcessManager localManager = LocalProcessManager();

Future<void> invoke(
  ProcessManager injectedManager,
  List<Object> command,
) async {
  localManager.runSync(command);
  await injectedManager.start(const ['git', 'status']);
  await localManager.run(const ['sh', '-c', 'echo safe']);
}

class FakeRunner {
  void run(List<Object> command) {}
}

void unrelated(FakeRunner runner, List<Object> command) => runner.run(command);
",
    )?;

    let json = security(&fixture)?;
    let occurrences = candidate(&json, "process-execution")["occurrences"]
        .as_array()
        .ok_or("process occurrences")?;
    assert_eq!(occurrences.len(), 2, "{json:#}");
    assert!(occurrences.iter().any(|occurrence| {
        occurrence["evidence"]
            .as_str()
            .is_some_and(|evidence| evidence.contains("localManager.runSync(command)"))
    }));
    assert!(occurrences.iter().any(|occurrence| {
        occurrence["evidence"]
            .as_str()
            .is_some_and(|evidence| evidence.contains("localManager.run(const"))
    }));
    assert!(
        json["security_blind_spots"]
            .as_array()
            .is_some_and(Vec::is_empty),
        "{json:#}"
    );

    Ok(())
}

#[test]
fn process_tearoffs_and_unresolved_part_provenance_are_explicit_blind_spots()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'dart:io';
import 'package:process/process.dart';
part 'runner.dart';

final runProcess = Process.run;
final ProcessManager localManager = LocalProcessManager();
final runManagedProcess = localManager.run;
dynamic processManager;

Future<ProcessResult> invoke(String executable, List<String> arguments) =>
    runProcess(executable, arguments);

void invokeManager(List<Object> command) =>
    processManager.runSync(command);

Future<ProcessResult> invokeManaged(List<Object> command) =>
    runManagedProcess(command);
",
    )?;
    write(
        &fixture,
        "lib/runner.dart",
        r"
part of 'main.dart';

Future<ProcessResult> invokeFromPart(
  String executable,
  List<String> arguments,
) => Process.run(executable, arguments);
",
    )?;
    write(
        &fixture,
        "lib/orphan.dart",
        r"
part of 'missing.dart';

Future<ProcessResult> invokeFromOrphan(
  String executable,
  List<String> arguments,
) => Process.run(executable, arguments);
",
    )?;

    let json = security(&fixture)?;
    let process_occurrences = candidate(&json, "process-execution")["occurrences"]
        .as_array()
        .ok_or("process occurrences")?;
    assert_eq!(process_occurrences.len(), 1, "{json:#}");
    assert_eq!(process_occurrences[0]["path"], "lib/runner.dart");
    let blind_spots = json["security_blind_spots"]
        .as_array()
        .ok_or("security blind spots")?;
    assert_eq!(blind_spots.len(), 4, "{json:#}");
    assert!(blind_spots.iter().any(|spot| {
        spot["reason"] == "unflattened-call"
            && spot["evidence"]
                .as_str()
                .is_some_and(|evidence| evidence.contains("runProcess("))
    }));
    assert!(blind_spots.iter().any(|spot| {
        spot["reason"] == "unflattened-call"
            && spot["evidence"]
                .as_str()
                .is_some_and(|evidence| evidence.contains("runManagedProcess("))
    }));
    assert!(blind_spots.iter().any(|spot| {
        spot["reason"] == "ambiguous-call-provenance" && spot["path"] == "lib/orphan.dart"
    }));
    assert!(blind_spots.iter().any(|spot| {
        spot["reason"] == "ambiguous-call-provenance"
            && spot["evidence"]
                .as_str()
                .is_some_and(|evidence| evidence.contains("processManager.runSync"))
    }));

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
