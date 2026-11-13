# CodeQL upload-sarif 4.39.3

Status: Complete

## Outcome + scope

The security workflow uses `github/codeql-action/upload-sarif` 4.39.3 instead of 4.39.0. The version moves to 0.0.75 because the version gates need a version above the base. Non-goals: other action updates and source changes.

## Repository context

Owners: `.github/workflows/security.yml` (the action pin); version files Cargo.toml, Cargo.lock, package.json, README.md and docs/ci.md. Evidence: Dependabot changed only the pin in the one workflow that uploads SARIF.
Same cause: N/A — one action pin; `grep -r upload-sarif .github` shows one use.
Single source: N/A — the pin lives once in the workflow.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: The package owner approved releasing this package when an update needs it; the coordinator asked for this fix in the session.

## Acceptance + steps

- [x] The workflow pins upload-sarif 4.39.3 → N/A — a pinned action has no behavior to test; the Security workflow run on the PR proves it.
- [x] Version 0.0.75 is synchronized across the version files → `tests/js/release-guards.test.mjs::version sync accepts matching manifests and lockfile`.

## Baseline + execution

Result: Passed
Evidence: Base main 6abacfd holds 0.0.74 and pins upload-sarif 4.39.0.
Execution: Single builder: one commit.

## Risks + recovery

The release publishes on merge and a published npm version cannot be reused. Recovery: revert the merge commit and ship a fix as the next patch version.

## ux_reference

N/A — workflow pin change with no visual surface.

## Verification

Result: Passed
Evidence: the full check exits 0 and the PR checks are green.
E2E: N/A — no code path changes.
Retro: the Dependabot branch was built on 0.0.73 and failed only the version gate → no change; the gate works as designed.

Delivery target: Merge
Delivery: Pending — PR checks green, merge, Release run publishes 0.0.75.
