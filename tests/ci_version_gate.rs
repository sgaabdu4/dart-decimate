use std::fs;
use std::path::Path;
use std::process::Command;

fn missing(needle: &str) -> std::io::Error {
    std::io::Error::other(format!("missing {needle}"))
}

fn index_of(contents: &str, needle: &str) -> Result<usize, Box<dyn std::error::Error>> {
    contents.find(needle).ok_or_else(|| missing(needle).into())
}

fn section_between<'a>(
    contents: &'a str,
    start: &str,
    end: &str,
) -> Result<&'a str, Box<dyn std::error::Error>> {
    let start_index = index_of(contents, start)?;
    let tail = &contents[start_index..];
    let end_index = index_of(tail, end)?;
    Ok(&tail[..end_index])
}

fn write_versions(
    root: &Path,
    cargo_version: &str,
    package_version: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(
        root.join("Cargo.toml"),
        format!("[package]\nname = \"dart-decimate\"\nversion = \"{cargo_version}\"\n"),
    )?;
    fs::write(
        root.join("package.json"),
        format!("{{\"name\":\"dart-decimate\",\"version\":\"{package_version}\"}}\n"),
    )?;
    Ok(())
}

fn run_git(root: &Path, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git").args(args).current_dir(root).output()?;
    if output.status.success() {
        return Ok(());
    }

    Err(std::io::Error::other(format!(
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    ))
    .into())
}

fn run_pr_bump_script(root: &Path) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/check-pr-version-bump.mjs");
    Ok(Command::new("node")
        .arg(script)
        .arg("origin/main")
        .current_dir(root)
        .output()?)
}

#[test]
fn pull_request_ci_requires_bumped_unpublished_versions() -> Result<(), Box<dyn std::error::Error>>
{
    let ci = fs::read_to_string(".github/workflows/ci.yml")?;
    let package = fs::read_to_string("package.json")?;
    let template = fs::read_to_string(".github/pull_request_template.md")?;
    let release_check = section_between(
        &ci,
        "      - name: Check release version",
        "      - name: Check formatting",
    )?;

    assert!(ci.contains("if: github.event_name == 'pull_request'"));
    assert!(release_check.contains(
        "git fetch --no-tags --depth=1 origin \"$GITHUB_BASE_REF:refs/remotes/origin/$GITHUB_BASE_REF\""
    ));
    assert!(release_check.contains("npm run version:bump:check -- \"origin/$GITHUB_BASE_REF\""));
    assert!(release_check.contains("npm run release:check"));
    assert!(
        index_of(
            release_check,
            "npm run version:bump:check -- \"origin/$GITHUB_BASE_REF\""
        )? < index_of(release_check, "npm run release:check")?
    );
    assert!(package.contains("\"version:bump:check\": \"node scripts/check-pr-version-bump.mjs\""));
    assert!(!ci.contains("Package versions unchanged; skipping release version check."));
    assert!(template.contains(
        "This PR bumps both `Cargo.toml` and `package.json` to an unpublished `dart-decimate` version"
    ));
    assert!(!template.contains("Version is intentionally unchanged"));

    Ok(())
}

#[test]
fn pr_version_bump_script_rejects_unchanged_or_downgraded_versions()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let root = fixture.path();

    run_git(root, &["init"])?;
    run_git(root, &["config", "user.email", "test@example.com"])?;
    run_git(root, &["config", "user.name", "Test User"])?;
    write_versions(root, "1.2.3", "1.2.3")?;
    run_git(root, &["add", "Cargo.toml", "package.json"])?;
    run_git(root, &["commit", "-m", "base"])?;
    run_git(root, &["update-ref", "refs/remotes/origin/main", "HEAD"])?;

    write_versions(root, "1.2.4", "1.2.4")?;
    let output = run_pr_bump_script(root)?;
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    write_versions(root, "1.2.3", "1.2.4")?;
    let output = run_pr_bump_script(root)?;
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Cargo.toml version must be bumped: 1.2.3 -> 1.2.3")
    );

    write_versions(root, "1.2.2", "1.2.4")?;
    let output = run_pr_bump_script(root)?;
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("Cargo.toml version must be bumped: 1.2.3 -> 1.2.2")
    );

    write_versions(root, "1.2.4", "1.2.3")?;
    let output = run_pr_bump_script(root)?;
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("package.json version must be bumped: 1.2.3 -> 1.2.3")
    );

    write_versions(root, "not-semver", "1.2.3")?;
    run_git(root, &["add", "Cargo.toml", "package.json"])?;
    run_git(root, &["commit", "-m", "invalid base"])?;
    run_git(root, &["update-ref", "refs/remotes/origin/main", "HEAD"])?;
    write_versions(root, "1.2.4", "1.2.4")?;
    let output = run_pr_bump_script(root)?;
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("base Cargo.toml has invalid semver: not-semver")
    );

    Ok(())
}

#[test]
fn release_workflow_checks_existing_state_before_release_version()
-> Result<(), Box<dyn std::error::Error>> {
    let release = fs::read_to_string(".github/workflows/release.yml")?;
    let state_index = index_of(&release, "      - name: Check existing release state")?;
    let release_check_index = index_of(&release, "      - name: Check release version")?;
    let release_check = section_between(
        &release,
        "      - name: Check release version",
        "      - name: Validate release candidate",
    )?;
    let validate = section_between(
        &release,
        "      - name: Validate release candidate",
        "      - name: Download release assets",
    )?;

    assert!(state_index < release_check_index);
    assert!(release_check.contains("if: steps.state.outputs.npm_exists == 'false'"));
    assert!(release_check.contains("run: npm run release:check"));

    assert!(validate.contains("npm run version:check"));
    assert!(!validate.contains("npm run release:check"));
    assert!(validate.contains("npm run migration:check"));

    Ok(())
}

#[test]
fn release_workflow_rejects_fresh_reused_versions_but_allows_repairs()
-> Result<(), Box<dyn std::error::Error>> {
    let release = fs::read_to_string(".github/workflows/release.yml")?;
    let state = section_between(
        &release,
        "      - name: Check existing release state",
        "      - name: Reject reused release tag",
    )?;
    let reused_version = section_between(
        &release,
        "      - name: Reject reused package version",
        "      - name: Check release version",
    )?;

    assert!(state.contains("tag_points_at_head=true"));
    assert!(state.contains("tag_points_at_head=false"));
    assert!(state.contains("npm_exists=true"));
    assert!(state.contains("npm_exists=false"));

    assert!(reused_version.contains("steps.state.outputs.npm_exists == 'true'"));
    assert!(reused_version.contains("steps.state.outputs.tag_points_at_head == 'false'"));
    assert!(!reused_version.contains("github.event_name == 'push'"));
    assert!(!reused_version.contains("github.run_attempt"));
    assert!(
        reused_version.contains(
            "dart-decimate@$VERSION is already published; bump Cargo.toml and package.json"
        )
    );

    Ok(())
}
