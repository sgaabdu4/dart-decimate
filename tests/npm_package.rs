use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

#[test]
fn npm_package_exposes_dart_decimate_bin() -> Result<(), Box<dyn std::error::Error>> {
    let source = fs::read_to_string("package.json")?;
    let package = serde_json::from_str::<Value>(&source)?;

    assert_eq!(package["name"], "dart-decimate");
    assert_eq!(package["bin"]["dart-decimate"], "npm/bin/dart-decimate.js");
    assert_eq!(
        package["bin"]["dart-decimate-mcp"],
        "npm/bin/dart-decimate-mcp.js"
    );
    assert_eq!(
        package["scripts"]["postinstall"],
        "node npm/scripts/postinstall.js"
    );
    assert_eq!(
        package["scripts"]["test:npx:local"],
        "node npm/scripts/test-npx-local.js"
    );
    assert!(
        package["scripts"]["test:npm:mcp"]
            .as_str()
            .is_some_and(|script| script.contains("node npm/bin/dart-decimate-mcp.js"))
    );
    assert!(
        package["scripts"]["test:npx:mcp:local"]
            .as_str()
            .is_some_and(|script| script.contains("npm/scripts/test-npx-mcp-local.js"))
    );
    assert!(
        package["scripts"]["test:postinstall:prebuilt"]
            .as_str()
            .is_some_and(|script| script.contains("npm/scripts/test-postinstall-prebuilt.js"))
    );
    assert!(
        package["scripts"]["test:npx:prebuilt"]
            .as_str()
            .is_some_and(|script| script.contains("npm/scripts/test-npx-prebuilt.js"))
    );
    assert!(Path::new("npm/bin/dart-decimate.js").is_file());
    assert!(Path::new("npm/bin/dart-decimate-mcp.js").is_file());
    assert!(Path::new("npm/bin/runner.js").is_file());
    assert!(Path::new("npm/scripts/postinstall.js").is_file());
    assert!(Path::new("npm/scripts/test-postinstall-prebuilt.js").is_file());
    assert!(Path::new("npm/scripts/test-npx-prebuilt.js").is_file());
    assert!(Path::new("npm/scripts/test-release-install-parity.js").is_file());
    assert!(Path::new("npm/scripts/test-npx-local.js").is_file());
    assert!(Path::new("npm/scripts/test-npx-mcp-local.js").is_file());
    assert!(
        package["scripts"]["test:release:parity"]
            .as_str()
            .is_some_and(|script| script.contains("test-release-install-parity.js"))
    );
    let mcp_script = fs::read_to_string("npm/scripts/test-npx-mcp-local.js")?;
    assert!(mcp_script.contains("2025-11-25"));
    assert!(mcp_script.contains("dart-decimate-mcp"));
    let readme = fs::read_to_string("README.md")?;
    let version = package["version"].as_str().ok_or("package version")?;
    assert!(readme.contains(&format!("dart-decimate@{version}")));
    assert!(readme.contains(&format!("--tag v{version} --locked")));
    assert!(readme.contains("dart-decimate --version"));

    Ok(())
}

#[test]
fn release_parity_script_rejects_a_missing_asset_cleanly() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    let output = Command::new("node")
        .arg("npm/scripts/test-release-install-parity.js")
        .env("DART_DECIMATE_RELEASE_ASSET_DIR", fixture.path())
        .env("TMPDIR", fixture.path())
        .output()?;

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("missing release asset"));
    Ok(())
}
