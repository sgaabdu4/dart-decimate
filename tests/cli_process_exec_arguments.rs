use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;

type Row = (&'static str, &'static str, bool);

const ROWS: [Row; 12] = [
    (
        "value after its own fixed option word",
        "Future<Process> start(String dir) => Process.start('mytool', ['--store', dir, '--json']);\n",
        false,
    ),
    (
        "executable is a final field set from a literal",
        "class Tool {\n  final String executable = 'mytool';\n\n  Future<Process> send(String message) =>\n      Process.start(executable, ['--message', message]);\n}\n",
        false,
    ),
    (
        "variable after a fixed double dash",
        "Future<ProcessResult> log(String path) => Process.run('git', ['log', '--', path]);\n",
        false,
    ),
    (
        "value attached to its option word",
        "Future<ProcessResult> commit(String text) => Process.run('git', ['commit', '--message=$text']);\n",
        false,
    ),
    (
        "spread of a list whose elements follow the rule",
        "Future<Process> start(String to) {\n  final base = ['--store', 'data', '--json'];\n  return Process.start('mytool', [...base, '--to', to]);\n}\n",
        false,
    ),
    (
        "variable executable",
        "Future<Process> start(String tool, String value) => Process.start(tool, ['--x', value]);\n",
        true,
    ),
    (
        "executable is a final field set by the constructor",
        "class Tool {\n  Tool(this.executable);\n\n  final String executable;\n\n  Future<Process> send(String message) =>\n      Process.start(executable, ['--message', message]);\n}\n",
        true,
    ),
    (
        "run in shell on",
        "Future<Process> start(String dir) => Process.start('mytool', ['--store', dir], runInShell: true);\n",
        true,
    ),
    (
        "variable first element",
        "Future<ProcessResult> run(String name) => Process.run('git', [name, '--all']);\n",
        true,
    ),
    (
        "variable after a fixed word that is not an option",
        "Future<ProcessResult> log(String name) => Process.run('git', ['log', name]);\n",
        true,
    ),
    (
        "spread of a variable list",
        "Future<ProcessResult> run(List<String> extra) => Process.run('git', ['status', '--short', ...extra]);\n",
        true,
    ),
    (
        "command shell with a variable command",
        "Future<ProcessResult> run(String command) => Process.run('sh', ['-c', command]);\n",
        true,
    ),
];

#[test]
fn process_calls_are_safe_only_with_fixed_executables_and_placed_variable_arguments()
-> Result<(), Box<dyn std::error::Error>> {
    for (case, body, flagged) in ROWS {
        if std::env::var("ROW").is_ok_and(|only| only != case) {
            continue;
        }
        let fixture = tempfile::tempdir()?;
        fs::write(fixture.path().join("pubspec.yaml"), "name: app\n")?;
        fs::create_dir_all(fixture.path().join("lib"))?;
        fs::write(
            fixture.path().join("lib/main.dart"),
            format!("import 'dart:io';\n\n{body}"),
        )?;
        let mut output = Vec::new();
        run_from(
            [
                "dart-decimate",
                "security",
                fixture.path().to_str().unwrap_or("."),
                "--format",
                "json",
            ],
            &mut output,
        )?;
        let json = serde_json::from_slice::<Value>(&output)?;
        let found = json["security_candidates"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|candidate| candidate["category"] == "process-execution");

        assert_eq!(found, flagged, "{case}: {json:#}");
    }

    Ok(())
}
