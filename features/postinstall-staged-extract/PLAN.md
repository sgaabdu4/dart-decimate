# Validate prebuilt archives before they replace cached binaries

Status: Complete

## Outcome + scope

Postinstall extracts a prebuilt archive into a staging directory, checks both CLI binaries are there, and only then moves them into the cache. An archive without the binaries still fails, and cached binaries are never deleted up front. Non-goals: download, fallback build and runner behavior.

## Repository context

Owners: `npm/scripts/postinstall.js` (`installPrebuilt`, `extractArchive`, `activateCachedBinaries`). Evidence: the 0.0.56 fix deleted cached binaries before extraction. The bad-archive tests then removed `npm/bin-cache` binaries while `tests/js/npm-wrapper.test.mjs` ran them in parallel, and the wrapper tests failed with status `null`.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Autonomous. AGENTS.md requires repairing this reported gate failure; it ships with the approved member-ownership PR.

## Acceptance + steps

- [x] JS suite passes with populated cached binaries → `node --test tests/js/*.test.mjs` six times, 37 passed each; cached binaries remain.
- [x] Archive without binaries still fails with stale binaries present → `postinstall rejects an asset without the CLI binaries` passes.
- [x] Prebuilt installs still activate binaries → `test-postinstall-prebuilt.js` and `test-npx-prebuilt.js` exit 0.

## Baseline + execution

Result: Passed
Evidence: With binaries copied into `npm/bin-cache`, `node --test tests/js/*.test.mjs` failed 3 of 5 runs on an npm wrapper test (status `null`); the Complete gate reported `FAIL tests`.
Execution: One builder; its own commit on the member-ownership branch.

## Risks + recovery

A move across filesystems would fail, so staging lives inside the cache directory. Recovery: revert the commit.

## ux_reference

N/A — install tooling only; no rendered interface.

## Verification

Result: Passed
Evidence: Acceptance commands above passed; no staging directory remains in `npm/bin-cache`.
E2E: Passed — `node tests/npm/test-postinstall-prebuilt.js` and `node tests/npm/test-npx-prebuilt.js` installed from packed tarballs and exited 0.

Delivery target: Merge
Delivery: Pending — ships with the member-ownership PR; PR CI and the Release run must pass.
