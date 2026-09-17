use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

#[test]
fn version_flag_reports_the_compiled_package_version() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_dart-decimate"))
        .arg("--version")
        .output()?;

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout)?.trim(),
        format!("dart-decimate {}", env!("CARGO_PKG_VERSION"))
    );
    Ok(())
}

#[test]
fn report_identifies_version_without_changing_the_v1_topology()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = install_parity_fixture();
    let output = Command::new(env!("CARGO_BIN_EXE_dart-decimate"))
        .current_dir(&fixture)
        .args(["check", "lib", "--threshold", "0", "--format", "json"])
        .output()?;

    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json = serde_json::from_slice::<Value>(&output.stdout)?;
    assert_eq!(json["schema_version"], "dart-decimate.report.v1");
    assert_eq!(
        json["tool"],
        format!("dart-decimate {}", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["code_duplications"], 0);
    assert_eq!(json["summary"]["unrendered_widgets"], 0);
    assert_eq!(json["summary"]["findings"], 0);
    assert_eq!(json["findings"], serde_json::json!([]));

    let object = json.as_object().ok_or("report object")?;
    for required in ["verdict", "summary", "findings"] {
        assert!(object.contains_key(required));
    }
    Ok(())
}

#[test]
fn report_schema_keeps_v1_and_accepts_the_versioned_tool_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_dart-decimate"))
        .args(["report-schema", "--format", "json"])
        .output()?;

    assert!(output.status.success());
    let schema = serde_json::from_slice::<Value>(&output.stdout)?;
    assert_eq!(schema["schema_version"], "dart-decimate.report.v1");
    let tool_pattern = schema["properties"]["tool"]["pattern"]
        .as_str()
        .ok_or("tool pattern")?;
    assert_eq!(
        tool_pattern,
        r"^dart-decimate [0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$"
    );
    for required in ["verdict", "summary", "findings"] {
        assert!(
            schema["required"]
                .as_array()
                .is_some_and(|fields| fields.iter().any(|field| field == required))
        );
    }
    Ok(())
}

fn install_parity_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/install-parity")
}
