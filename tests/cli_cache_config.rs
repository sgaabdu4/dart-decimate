use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn config_output_serializes_cache_settings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".dart-decimaterc",
        "[cache]\nenabled = true\npath = \".dart-decimate/cache\"\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "config",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "dart-decimate.config.v1");
    assert_eq!(json["config"]["cache"]["enabled"], true);
    assert_eq!(json["config"]["cache"]["path"], ".dart-decimate/cache");

    Ok(())
}

#[test]
fn config_schema_exposes_cache_settings() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        ["dart-decimate", "config-schema", "--format", "json"],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["properties"]["cache"]["type"], "object");
    assert_eq!(json["properties"]["cache"]["additionalProperties"], false);
    assert_eq!(
        json["properties"]["cache"]["properties"]["enabled"]["type"],
        "boolean"
    );
    assert_eq!(
        json["properties"]["cache"]["properties"]["path"]["type"],
        "string"
    );

    Ok(())
}

#[test]
fn unknown_cache_config_keys_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, ".dart-decimaterc", "[cache]\nenabeld = true\n")?;
    let mut output = Vec::new();

    let error = match run_from(
        [
            "dart-decimate",
            "config",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    ) {
        Ok(code) => panic!("unknown cache config key should fail, got exit code {code}"),
        Err(error) => error,
    };

    let message = error.to_string();
    assert!(message.contains(".dart-decimaterc"));
    assert!(message.contains("enabeld"));
    assert!(message.contains("unknown"));
    assert_eq!(output, b"");

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
