# Verify scaffold maintenance before skipping publication

Status: Complete

## Outcome + scope

Allow a canonical, committed Hard Eng update to retain the published package version after native verification, without rebuilding or publishing unchanged artifacts. Other changes retain every version, tag, quality, packaging, and publication guard. Release the policy as 0.0.62 while preserving the independently delivered final scaffold.

## Repository context

Owners: `.githooks/pre-commit` repeats the registry check before a committed snapshot exists. `.github/workflows/ci.yml` skips shared version assertions for documentation. `.github/workflows/release.yml` rejects published versions before the installed native `update.check_scaffold_update` can verify maintenance. Existing release workflow tests are in `tests/ci_version_gate.rs`; release scripts and their JavaScript regression tests remain unchanged.

## Decisions + authorization

Blockers: None
Handoff: Ready for ship
Authority: The user authorized supported-tool cleanup, final scaffold adoption, repository delivery, and resulting native releases. The reviewed implementation is limited to the three existing flow owners, their existing tests/docs, and established version fields. Preserve independently delivered upstream work and advance normally from published 0.0.61 to the verified unused 0.0.62. Integrated native verification passed in the coordinated slot; source97 is inherited through the upstream supported updater.

## Acceptance + steps

- [x] Normal commits retain version synchronization and migration checks; required committed-snapshot CI owns the registry assertion, including docs-only PRs.
- [x] The unconditional Rust and npm preparation job executes the existing native scaffold verifier before making a publication decision; only explicit true skips artifact jobs.
- [x] False, unknown, mixed, dirty, or failed proof cannot authorize skipped product/release checks; exact-tag repair, collision checks, and release scripts retain their behavior.
- [x] Existing tests execute the proof integration and preserved docs assertions; the final real updater commit passes native proof against its clean parent while the combined policy PR requires normal release verification.
- [x] Preserve the supported final Hard Eng update, synchronize 0.0.62 across established version owners, and pass the required integrated native checks; retain the normal committed-snapshot pre-push as a delivery requirement.
- [x] Document the exact verified maintenance exception without introducing an arbitrary unchanged-version waiver.

## Baseline + execution

Result: Passed
Evidence: Delivered main 966def44 passed its exact-head required Rust/npm check in 7m48s, security checks, four platform builds, and Release 36493063958. Tag v0.0.61 points to that merged commit, and canonical npm publication with matching registry provenance is confirmed. This is the starting implementation proof, not verification of the new policy.
Integration: PR #124's pnpm/setup 3.0.0 pins and completed plan are preserved. PR #131 delivered source97cc783afd75c81b08a28f0ce392414aa50cef7e through actual updater commit 396e01d8, whose clean parent is 1c22e150. That commit changes only the marker and two managed Python files; its following commit changes only the existing feature plan. The policy branch fast-forwarded without conflicts, and all prepared policy files plus incoming scaffold/plan files were byte-accounted. Version 0.0.62 had no remote tag, registry entry, or open PR reservation before selection.
Execution: One builder extends the existing owners and focused tests; an independent review checks fail-closed behavior. Reuse the retained task checkout and the real upstream updater commit for positive native proof, then verify the combined policy against current main. Do not repeat an already delivered source97 adoption.

## Risks + recovery

The first PR changes release inputs, so it must take the normal versioned release path. A missing or unsuccessful canonical proof must never become permission to skip that path. Keep the exact-clean-tagged-HEAD helper, published-version checks, asset parity, and trusted-publisher permissions unchanged. The positive maintenance proof uses the actual updater commit and its immediate clean parent, not an invented source revision or package version. Preserve unrelated branches, plans, and shared caches.

## ux_reference

N/A — this change affects Git hooks and CI publication decisions, with no product user interface.

## Verification

Result: Passed
Evidence: Direct execution of the actual workflow proof passed five controlled outcomes: boolean true authorizes maintenance, false/unknown/string results keep the normal path, and an exception fails without a success output. Four real Git fixtures passed the installed native verifier's negative paths for mixed package/configuration changes, dirty state, and unknown base. Parsed workflow wiring, actionlint 1.7.12, offline Zizmor 1.30.1, ShellCheck, Rust formatting, and diff whitespace pass. Version synchronization, the 0.0.61-to-0.0.62 bump, unpublished-release check, and migration assertions pass. Independent review found no material defect in the eight-file policy delta. The actual workflow proof verified real supported updater 396e01d8 against clean parent 1c22e150 as scaffold-only, then rejected the combined policy commit b04f2d22 against main 966def44; this native proof completed in 37.584 seconds. The integrated native check passed all 30 gates in 213.033 seconds: 1,005 Rust tests including both new workflow-boundary cases, 41 JavaScript tests, one performance test, strict scans, packaging, and native npm CLI/MCP installation journeys. JavaScript line coverage is 867/989 (87.66%) against the unchanged 70% minimum. Complete plan validation and its selected secret checks passed. Existing release scripts and real npm consumer compatibility contracts remain unchanged.
E2E: Passed — actual workflow execution accepted the real canonical updater commit against its clean parent and kept the mixed policy branch on the full path. Controlled outcomes and unsafe Git fixtures passed. Integrated native npm CLI and MCP installation journeys passed in 31.760 and 44.469 seconds; the hosted bootstrap release must still prove packaged/Cargo parity and publication.
Delivery target: Merge
Delivery: Pending — the normal committed-snapshot pre-push, one follow-up PR, exact-head required CI, guarded merge, merged-main checks, one normal bootstrap release, tag/four assets/registry provenance, and isolated published-binary proof. Future source-only publication savings require an actual maintenance merge; local positive proof does not claim that hosted outcome.
