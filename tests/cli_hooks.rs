use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn hooks_status_reports_missing_git_hook() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "hooks",
            "status",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "dart-decimate.hooks.v1");
    assert_eq!(json["command"], "hooks status");
    assert_eq!(json["files"][0]["path"], ".git/hooks/pre-commit");
    assert_eq!(json["files"][0]["installed"], false);
    assert_eq!(json["files"][0]["managed"], false);

    Ok(())
}

#[test]
fn hooks_install_writes_managed_executable_git_hook() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "hooks",
            "install",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--branch",
            "origin/dev",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let hook = fixture.path().join(".git/hooks/pre-commit");
    let source = fs::read_to_string(&hook)?;
    assert_eq!(code, 0);
    assert_eq!(json["command"], "hooks install");
    assert_eq!(json["files"][0]["action"], "created");
    assert_eq!(json["files"][0]["managed"], true);
    assert!(source.contains("dart-decimate-managed-hook"));
    assert!(source.contains("origin/dev"));
    assert!(source.contains("dart-decimate audit . --base \"$BASE\" --format json --summary"));

    Ok(())
}

#[test]
fn hooks_install_refuses_unmanaged_hook_without_force() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    let hook = fixture.path().join(".git/hooks/pre-commit");
    fs::write(&hook, "#!/bin/sh\necho custom\n")?;

    let error = match run_from(
        [
            "dart-decimate",
            "hooks",
            "install",
            fixture.path().to_str().unwrap_or("."),
        ],
        &mut Vec::new(),
    ) {
        Ok(code) => panic!("install should refuse unmanaged hook, got {code}"),
        Err(error) => error,
    };

    assert!(
        error
            .to_string()
            .contains("refusing to overwrite unmanaged hook")
    );
    assert!(fs::read_to_string(hook)?.contains("echo custom"));

    Ok(())
}

#[test]
fn hooks_uninstall_removes_only_managed_hook() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    run_from(
        [
            "dart-decimate",
            "hooks",
            "install",
            fixture.path().to_str().unwrap_or("."),
        ],
        &mut Vec::new(),
    )?;
    let hook = fixture.path().join(".git/hooks/pre-commit");
    assert!(hook.is_file());
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "hooks",
            "uninstall",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["files"][0]["action"], "removed");
    assert_eq!(json["files"][0]["installed"], false);
    assert!(!hook.exists());

    Ok(())
}

#[test]
fn hooks_install_agent_manages_claude_gate_and_agents_block()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "hooks",
            "install",
            fixture.path().to_str().unwrap_or("."),
            "--target",
            "agent",
            "--branch",
            "origin/dev",
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let script = fixture.path().join(".claude/hooks/dart-decimate-gate.sh");
    let settings = fixture.path().join(".claude/settings.json");
    let agents = fixture.path().join("AGENTS.md");
    assert_eq!(code, 0);
    assert_eq!(json["target"], "agent");
    assert_eq!(json["files"].as_array().map_or(0, Vec::len), 3);
    let source = fs::read_to_string(&script)?;
    assert!(source.contains("dart-decimate-managed-hook"));
    assert!(source.contains("pnpm exec dart-decimate"));
    assert!(fs::read_to_string(settings)?.contains("dart-decimate-gate.sh"));
    assert!(fs::read_to_string(agents)?.contains("origin/dev"));

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        use std::process::{Command, Stdio};

        let binaries = fixture.path().join("bin");
        fs::create_dir(&binaries)?;
        let scanner = binaries.join("dart-decimate");
        for (report, scanner_exit, hook_exit) in
            [(r#"{"verdict":"fail"}"#, 1, 2), (r#"{"error":true}"#, 2, 0)]
        {
            fs::write(
                &scanner,
                format!("#!/bin/sh\nprintf '%s\\n' '{report}'\nexit {scanner_exit}\n"),
            )?;
            fs::set_permissions(&scanner, fs::Permissions::from_mode(0o755))?;
            let mut child = Command::new("sh")
                .arg(&script)
                .env("PATH", format!("{}:/usr/bin:/bin", binaries.display()))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            child
                .stdin
                .take()
                .ok_or("missing hook input")?
                .write_all(b"git push")?;
            let output = child.wait_with_output()?;
            assert_eq!(output.status.code(), Some(hook_exit));
        }
    }

    Ok(())
}

#[test]
fn hooks_uninstall_agent_removes_managed_agent_surfaces() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    run_from(
        [
            "dart-decimate",
            "hooks",
            "install",
            fixture.path().to_str().unwrap_or("."),
            "--target",
            "agent",
        ],
        &mut Vec::new(),
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "hooks",
            "uninstall",
            fixture.path().to_str().unwrap_or("."),
            "--target",
            "agent",
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let script = fixture.path().join(".claude/hooks/dart-decimate-gate.sh");
    let settings = fs::read_to_string(fixture.path().join(".claude/settings.json"))?;
    let agents = fs::read_to_string(fixture.path().join("AGENTS.md"))?;
    assert_eq!(code, 0);
    assert!(!script.exists());
    assert!(!settings.contains("dart-decimate-gate.sh"));
    assert!(!agents.contains("dart-decimate-managed-hook:start"));
    assert_eq!(json["files"][0]["action"], "removed");

    Ok(())
}

#[test]
fn setup_hooks_alias_installs_agent_hook() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "setup-hooks",
            fixture.path().to_str().unwrap_or("."),
            "--agent",
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["target"], "agent");
    assert!(
        fixture
            .path()
            .join(".claude/hooks/dart-decimate-gate.sh")
            .exists()
    );

    Ok(())
}

#[test]
fn setup_hooks_dry_run_is_read_only() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let code = run_from(
        [
            "dart-decimate",
            "setup-hooks",
            fixture.path().to_str().unwrap_or("."),
            "--dry-run",
            "--format",
            "json",
        ],
        &mut Vec::new(),
    )?;

    assert_eq!(code, 0);
    assert!(
        !fixture
            .path()
            .join(".claude/hooks/dart-decimate-gate.sh")
            .exists()
    );

    Ok(())
}

fn git_fixture() -> Result<TempDir, std::io::Error> {
    let fixture = tempfile::tempdir()?;
    fs::create_dir_all(fixture.path().join(".git/hooks"))?;
    Ok(fixture)
}
