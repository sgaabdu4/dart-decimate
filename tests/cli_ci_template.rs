use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;

#[test]
fn github_ci_template_emits_yaml_workflow() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        ["dart-decimate", "ci-template", "github", "--format", "yaml"],
        &mut output,
    )?;

    let yaml = String::from_utf8(output)?;
    assert_eq!(code, 0);
    assert!(yaml.contains("name: Dart Decimate"));
    assert!(yaml.contains("pull_request:"));
    assert!(yaml.contains(&pinned_audit_command()));
    assert!(yaml.contains("--strict"));

    Ok(())
}

#[test]
fn gitlab_ci_template_emits_yaml_template() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        ["dart-decimate", "ci-template", "gitlab", "--format", "yaml"],
        &mut output,
    )?;

    let yaml = String::from_utf8(output)?;
    assert_eq!(code, 0);
    assert!(yaml.contains("stages:"));
    assert!(yaml.contains("dart-decimate:"));
    assert!(yaml.contains(&pinned_audit_command()));
    assert!(yaml.contains("--strict"));

    Ok(())
}

#[test]
fn ci_template_json_envelope_lists_target_path_and_content()
-> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        ["dart-decimate", "ci-template", "gitlab", "--format", "json"],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "dart-decimate.ci-template.v1");
    assert_eq!(json["kind"], "ci-template");
    assert_eq!(json["platform"], "gitlab");
    assert_eq!(json["vendor"], false);
    assert_eq!(json["files"][0]["path"], ".gitlab-ci.yml");
    assert!(
        json["files"][0]["content"]
            .as_str()
            .is_some_and(|content| content.contains(&pinned_audit_command()))
    );

    Ok(())
}

#[test]
fn gitlab_vendor_writes_scoped_files_and_refuses_overwrite()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let root = fixture.path().to_str().unwrap_or(".");
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "ci-template",
            "gitlab",
            "--format",
            "json",
            "--vendor",
            "--root",
            root,
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["vendor"], true);
    assert!(fixture.path().join("ci/gitlab-ci.yml").is_file());
    assert!(fixture.path().join("ci/scripts/review.sh").is_file());
    assert!(fixture.path().join("ci/scripts/comment.sh").is_file());
    assert!(
        fs::read_to_string(fixture.path().join("ci/scripts/review.sh"))?
            .contains(&pinned_audit_command())
    );

    let error = match run_from(
        [
            "dart-decimate",
            "ci-template",
            "gitlab",
            "--vendor",
            "--root",
            root,
        ],
        &mut Vec::new(),
    ) {
        Ok(code) => panic!("vendor should refuse overwrite, got exit code {code}"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("refusing to overwrite"));

    Ok(())
}

fn pinned_audit_command() -> String {
    format!(
        "npx --yes dart-decimate@{} audit --format json --base",
        env!("CARGO_PKG_VERSION")
    )
}

#[test]
fn ci_reconcile_review_dry_run_extracts_fingerprints() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let envelope = fixture.path().join("review.json");
    fs::write(
        &envelope,
        serde_json::json!({
            "schema_version": "dart-decimate.review-github.v1",
            "findings": [
                { "fingerprint": "dart-decimate:a" },
                { "fingerprint": "dart-decimate:b" },
                { "fingerprint": "dart-decimate:a" }
            ]
        })
        .to_string(),
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "ci",
            "reconcile-review",
            "--provider",
            "github",
            "--repo",
            "owner/repo",
            "--pr",
            "12",
            "--envelope",
            envelope.to_str().unwrap_or("review.json"),
            "--dry-run",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(
        json["schema_version"],
        "dart-decimate.ci-reconcile-review.v1"
    );
    assert_eq!(json["kind"], "ci-reconcile-review");
    assert_eq!(json["provider"], "github");
    assert_eq!(json["summary"]["envelope_fingerprints"], 2);
    assert_eq!(json["fingerprints"][0], "dart-decimate:a");
    assert_eq!(json["fingerprints"][1], "dart-decimate:b");

    Ok(())
}

#[test]
fn ci_reconcile_review_requires_dry_run() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let envelope = fixture.path().join("review.json");
    fs::write(&envelope, r#"{"findings":[]}"#)?;

    let error = match run_from(
        [
            "dart-decimate",
            "ci",
            "reconcile-review",
            "--envelope",
            envelope.to_str().unwrap_or("review.json"),
        ],
        &mut Vec::new(),
    ) {
        Ok(code) => panic!("reconcile-review should require dry-run, got {code}"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("dry-run only"));
    Ok(())
}

#[test]
fn manifest_lists_ci_template_command() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(["dart-decimate", "schema", "--format", "json"], &mut output)?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(
        json["schemas"]["ci_reconcile_review"],
        "dart-decimate.ci-reconcile-review.v1"
    );
    assert_eq!(
        json["schemas"]["ci_template"],
        "dart-decimate.ci-template.v1"
    );
    assert!(json["commands"].as_array().is_some_and(|commands| {
        commands.iter().any(|command| {
            command["name"] == "ci-template" && command["schema"] == "dart-decimate.ci-template.v1"
        })
    }));
    assert!(json["commands"].as_array().is_some_and(|commands| {
        commands.iter().any(|command| {
            command["name"] == "ci" && command["schema"] == "dart-decimate.ci-reconcile-review.v1"
        })
    }));

    Ok(())
}
