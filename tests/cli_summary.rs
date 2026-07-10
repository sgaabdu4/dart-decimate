mod common;

use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn summary_flag_is_supported_by_report_commands() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() { print('ok'); }\n")?;
    commit_all(&fixture)?;

    let commands: [(&str, &[&str]); 7] = [
        ("check", &[]),
        ("audit", &["--base", "HEAD"]),
        ("dead-code", &[]),
        ("cycles", &[]),
        ("dupes", &["--min-lines", "5", "--min-tokens", "10"]),
        ("health", &[]),
        ("flags", &[]),
    ];

    for (command, extra) in commands {
        let (code, json) = run_json(report_args(command, &fixture, extra, true))?;
        assert_eq!(code, 0, "{command} --summary should pass on clean fixture");
        assert_eq!(json["schema_version"], "dart-decimate.report.v1");
        assert_eq!(json["command"], command);
        assert_eq!(json["summary"]["findings"], 0);
    }

    Ok(())
}

#[test]
fn non_security_summary_preserves_json_items_and_exit_code()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_duplicate_pair(&fixture)?;
    let extra = ["--min-lines", "5", "--min-tokens", "10"];

    let (full_code, full_json) = run_json(report_args("dupes", &fixture, &extra, false))?;
    let (summary_code, summary_json) = run_json(report_args("dupes", &fixture, &extra, true))?;

    assert_eq!(summary_code, full_code);
    assert_eq!(summary_code, 0);
    assert_eq!(summary_json["summary"], full_json["summary"]);
    assert_eq!(summary_json["findings"], full_json["findings"]);
    assert_eq!(summary_json["clone_groups"], full_json["clone_groups"]);
    assert_eq!(summary_json["next_steps"], full_json["next_steps"]);

    Ok(())
}

fn run_json(args: Vec<String>) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(args, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    Ok((code, json))
}

fn report_args(command: &str, fixture: &TempDir, extra: &[&str], summary: bool) -> Vec<String> {
    let mut args = vec![
        "dart-decimate".to_owned(),
        command.to_owned(),
        fixture.path().display().to_string(),
        "--format".to_owned(),
        "json".to_owned(),
    ];
    if summary {
        args.push("--summary".to_owned());
    }
    args.extend(extra.iter().map(|value| (*value).to_owned()));
    args
}

fn write_duplicate_pair(fixture: &TempDir) -> Result<(), std::io::Error> {
    let source = "void shared() {
  final items = [1, 2, 3];
  final active = items.where((item) => item > 1);
  print(active.length);
}
";
    write(fixture, "lib/a.dart", source)?;
    write(fixture, "lib/b.dart", source)
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
    let output = common::isolated_git_command()
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
