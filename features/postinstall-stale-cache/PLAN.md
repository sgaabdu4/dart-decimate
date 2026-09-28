# Reject prebuilt archives without binaries even when the cache is stale

Status: Complete

## Outcome + scope

Postinstall clears the cached binaries before it extracts a prebuilt archive, so an archive without the CLI binaries fails even when an earlier install left binaries behind. Non-goals: download, fallback build and runner behavior.

## Repository context

Owners: `npm/scripts/postinstall.js` (`installPrebuilt`); `tests/js/postinstall.test.mjs` already states the requirement. Evidence: `activateCachedBinaries` checks `existsSync`, which stale files in `npm/bin-cache` satisfy. The pre-commit hook's `pnpm run` runs the root postinstall, which fills that cache in a checkout installed with `--ignore-scripts`. After any commit, the local `tests` gate failed.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Autonomous. AGENTS.md requires repairing this reported gate failure before the issue 126 fix ships; the owner asked to merge that PR.

## Acceptance + steps

- [x] Archive without binaries fails with stale cached binaries present → `node --test tests/js/postinstall.test.mjs`, 6 passed.
- [x] Coverage still includes postinstall → `pnpm run test:coverage`, `npm/scripts/postinstall.js` reported; `npm/` + `scripts/` line coverage 87.49%.
- [x] Prebuilt install journeys unchanged → `test-postinstall-prebuilt.js` and `test-npx-prebuilt.js` exit 0.

## Baseline + execution

Result: Passed
Evidence: With binaries in `npm/bin-cache`, `postinstall rejects an asset without the CLI binaries` failed (exit 0, expected 127); the Complete gate reported `FAIL tests`.
Execution: One builder; its own commit before shipping the issue 126 fix.

## Risks + recovery

The two bad-archive tests now remove a checkout's cached binaries. They already extract into that cache, and `npm/bin/runner.js` falls back to `target/` or `cargo run`. Recovery: revert the commit.

## ux_reference

N/A — install tooling only; no rendered interface.

## Verification

Result: Passed
Evidence: Acceptance commands above passed with the stale cache present. Full gate on the combined branch: `python3 .hooks/hard-eng.py check --base origin/main --plan-stage Complete` → exit 0, 30/30 PASS.
E2E: Passed — `node tests/npm/test-postinstall-prebuilt.js` and `node tests/npm/test-npx-prebuilt.js` installed from packed tarballs and exited 0.

Delivery target: Merge
Delivery: Pending — ships with the issue 126 PR; PR CI and the Release run must pass.
