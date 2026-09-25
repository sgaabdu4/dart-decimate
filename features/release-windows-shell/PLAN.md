# Fix the Windows release build shell

Status: Complete

## Outcome + scope

The release workflow builds every matrix asset again: its build job runs each step in bash, so `$TARGET` and `$RUST_TOOLCHAIN` expand on Windows, and 0.0.51 publishes in place of the never-published 0.0.50. Non-goals: other workflow changes.

## Repository context

Owners: `.github/workflows/release.yml` `build-assets`; `tests/ci_version_gate.rs` `release_workflow_builds_and_publishes_the_verified_tag`. Evidence: the release run for merge `88c68f6` failed at the Windows "Install Rust" step with `invalid toolchain name: ''`, because the step moved from `${{ matrix.target }}` to `"$TARGET"` without `shell: bash`, and Windows defaults to PowerShell. The publish job was skipped, so 0.0.50 was never published. The version-bump gate requires a version above main's, so the fix ships as 0.0.51.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Autonomous. The user asked to merge the Hard Eng install when green; this repairs the release that merge broke.

## Acceptance + steps

- [x] `build-assets` defaults every run step to bash → `release_workflow_builds_and_publishes_the_verified_tag` asserts it and fails without it.
- [x] Version 0.0.51 in Cargo.toml, Cargo.lock, package.json, README.md and docs/ci.md → version-sync and version-bump gates pass.

## Baseline + execution

Result: Passed
Evidence: Main `88c68f6` passed PR CI (30/30 Hard Eng gates), but its release run failed on the Windows build as above.
Execution: One change: the job-level shell default, the regression assertion and the version bump.

## Risks + recovery

Other build steps that relied on PowerShell would change behaviour; all of them already use bash or are uses-steps. Recovery: revert the default.

## ux_reference

N/A — CI workflow change with no visual surface.

## Verification

Result: Passed
Evidence: `cargo test --test ci_version_gate` → 7 passed; with the old release.yml, `release_workflow_builds_and_publishes_the_verified_tag` fails at the new assertion.
Full gate: `python3 .hooks/hard-eng.py check --base origin/main --plan-stage Ready` → 30/30 gates PASS.
E2E: N/A — the release journey runs only on main after merge; Delivery records it.

Delivery target: Merge
Delivery: Pending — PR checks green, squash merge, release run publishes 0.0.51.
