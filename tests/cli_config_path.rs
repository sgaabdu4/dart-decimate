use std::fs;
use std::process::Command;

use dart_decimate::cli::run_from;

#[test]
fn config_path_exits_three_when_no_config_is_found() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "config",
            fixture.path().to_str().unwrap_or("."),
            "--path",
        ],
        &mut output,
    )?;

    assert_eq!(code, 3);
    assert_eq!(output, b"");

    let binary = Command::new(env!("CARGO_BIN_EXE_dart-decimate"))
        .args(["config", fixture.path().to_str().unwrap_or("."), "--path"])
        .output()?;

    assert_eq!(binary.status.code(), Some(3));
    assert_eq!(binary.stdout, b"");
    assert_eq!(binary.stderr, b"");

    Ok(())
}

#[test]
fn config_path_prints_discovered_config() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let config = fixture.path().join(".dart-decimaterc");
    write(&fixture, ".dart-decimaterc", "[cli]\nformat = \"json\"\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "config",
            fixture.path().to_str().unwrap_or("."),
            "--path",
        ],
        &mut output,
    )?;

    assert_eq!(code, 0);
    assert_eq!(
        String::from_utf8(output)?,
        format!("{}\n", config.display())
    );

    Ok(())
}

fn write(fixture: &tempfile::TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
