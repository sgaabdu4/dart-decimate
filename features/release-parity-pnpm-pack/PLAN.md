# Fix the release parity tarball path after the pnpm migration

Status: Complete

## Outcome + scope

The release parity check installs the packed npm tarball again, so main can publish. Non-goals: other release steps.

## Repository context

Owners: `tests/npm/test-release-install-parity.js` (`packNpmPackage`). Evidence: main's Release run for 0.0.55 failed in "Verify Cargo and npm install parity" with `ENOENT` on a doubled `/tmp/<dir>/tmp/<dir>/dart-decimate-0.0.55.tgz` path; `pnpm pack --json` returns an absolute `filename`, where `npm pack --json` returned a bare name.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Autonomous. AGENTS.md requires this repair before the issue 126 fix ships; the owner asked to merge that PR.

## Acceptance + steps

- [x] Parity script finds the packed tarball → local run with a darwin-arm64 release asset reports `release install parity ok`.
- [x] Missing-asset handling stays clean → `release_parity_script_rejects_a_missing_asset_cleanly` passes.

## Baseline + execution

Result: Passed
Evidence: Unmodified script with a local darwin-arm64 asset → `npm install` ENOENT on the doubled `$TMPDIR/<dir>/$TMPDIR/<dir>/dart-decimate-0.0.55.tgz`, matching main's Release run. Draft gate on main `0150e1e` passed every check except the expected `version-bump`.
Execution: One builder; its own commit before the issue 126 fix.

## Risks + recovery

N/A — one path expression in a release-only test script; the next Release run reruns it.

## ux_reference

N/A — release tooling only; no rendered interface.

## Verification

Result: Passed
Evidence: `node tests/npm/test-release-install-parity.js` with the local asset → exit 0, `release install parity ok: Cargo source and npm 0.0.55 emitted identical reports`; `cargo test --test npm_package` → 2 passed.
E2E: Passed — ran `node tests/npm/test-release-install-parity.js` against a locally built release asset and got the parity success line.

Delivery target: Merge
Delivery: Pending — ships with the issue 126 PR; its Release run must pass the parity step.
