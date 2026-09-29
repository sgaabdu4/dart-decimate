# Verify scaffold maintenance before skipping publication

Status: Complete

## Outcome + scope

Allow a canonical, committed Hard Eng update to retain the published package version after native verification, without rebuilding or publishing unchanged artifacts. Other changes retain every version, tag, quality, packaging, and publication guard. Release the policy as 0.0.62 while preserving the independently delivered final scaffold. Consolidate the two identical packed-install journeys into one native npx invocation and fresh installation, with cold MCP initialization first and CLI help second.

## Repository context

Owners: `.githooks/pre-commit` repeats the registry check before a committed snapshot exists. `.github/workflows/ci.yml` skips shared version assertions for documentation. `.github/workflows/release.yml` rejects published versions before the installed native `update.check_scaffold_update` can verify maintenance. Existing release workflow tests are in `tests/ci_version_gate.rs`; release scripts and their JavaScript regression tests remain unchanged. `tests/npm/test-npx-local.js` will own both packed entrypoint assertions, while `tests/npm_package.rs` and `tests/js/npx-cache-isolation.test.mjs` retain wiring and fresh-cache proof.

## Decisions + authorization

Blockers: None
Handoff: Ready for ship
Authority: The user authorized supported-tool cleanup, final scaffold adoption, repository delivery, and resulting native releases. The reviewed release-policy implementation is limited to the three existing flow owners, their existing tests/docs, and established version fields. The user also authorized removing the demonstrated duplicate cold installation through the existing test owner without changing production wrappers or the separate wrapper fallback tests. Preserve independently delivered upstream work and advance normally from published 0.0.61 to the verified unused 0.0.62. Integrated native verification passed in the coordinated slot; source97 is inherited through the upstream supported updater.

## Acceptance + steps

- [x] Normal commits retain version synchronization and migration checks; required committed-snapshot CI owns the registry assertion, including docs-only PRs.
- [x] The unconditional Rust and npm preparation job executes the existing native scaffold verifier before making a publication decision; only explicit true skips artifact jobs.
- [x] False, unknown, mixed, dirty, or failed proof cannot authorize skipped product/release checks; exact-tag repair, collision checks, and release scripts retain their behavior.
- [x] Existing tests execute the proof integration and preserved docs assertions; the final real updater commit passes native proof against its clean parent while the combined policy PR requires normal release verification.
- [x] Preserve the independently delivered Hard Eng scaffold, synchronize 0.0.62 across established version owners, and verify the package changes with native checks; retain the final committed-snapshot pre-push as a delivery requirement.
- [x] Adopt the verified Hard Eng selective-CI correction through the supported updater, then verify the final combined snapshot before shipping this same PR.
- [x] Refresh dependencies to their latest compatible releases: Cargo.lock takes nine patch/minor crate updates, every Cargo.toml requirement is already on its latest major, Biome is latest, and @types/node stays on 22.20.4 to match the supported Node 22.12 engine floor.
- [x] Document the exact verified maintenance exception without introducing an arbitrary unchanged-version waiver.
- [x] Pack once into a fresh disposable npm cache, require successful cold MCP initialization with clean JSON output, then run actual CLI help in the same native npx invocation; reject outer installation stdout and preserve cleanup and all existing assertions.

## Baseline + execution

Result: Passed
Evidence: Delivered main 966def44 passed its exact-head required Rust/npm check in 7m48s, security checks, four platform builds, and Release 36493063958. Tag v0.0.61 points to that merged commit, and canonical npm publication with matching registry provenance is confirmed. This is the starting implementation proof, not verification of the new policy.
Integration: PR #124's pnpm/setup 3.0.0 pins and completed plan are preserved. PR #131 delivered source97cc783afd75c81b08a28f0ce392414aa50cef7e through actual updater commit 396e01d8, whose clean parent is 1c22e150. That commit changes only the marker and two managed Python files; its following commit changes only the existing feature plan. The policy branch fast-forwarded without conflicts, and all prepared policy files plus incoming scaffold/plan files were byte-accounted. Version 0.0.62 had no remote tag, registry entry, or open PR reservation before selection.
Execution: One builder extends the existing owners and focused tests; an independent review checks fail-closed behavior. Reuse the retained task checkout and the real upstream updater commit for positive native proof, then verify the combined policy against current main. Do not repeat an already delivered source97 adoption.

## Risks + recovery

The first PR changes release inputs, so it must take the normal versioned release path. A missing or unsuccessful canonical proof must never become permission to skip that path. Keep the exact-clean-tagged-HEAD helper, published-version checks, asset parity, and trusted-publisher permissions unchanged. The positive maintenance proof uses the actual updater commit and its immediate clean parent, not an invented source revision or package version. Preserve unrelated branches, plans, and shared caches. The combined installation must reject outer npx stdout, parse the complete cold MCP response separately, and preserve the CLI assertion. Its native exit cleanup owns only the disposable fixture, including nonzero exits; production wrapper fallback and release parity stay unchanged.

## ux_reference

N/A — this change affects Git hooks and CI publication decisions, with no product user interface.

## Verification

Result: Passed
Evidence: Direct execution of the actual workflow proof passed five controlled outcomes: boolean true authorizes maintenance, false/unknown/string results keep the normal path, and an exception fails without a success output. Four real Git fixtures passed the installed native verifier's negative paths for mixed package/configuration changes, dirty state, and unknown base. Parsed workflow wiring, actionlint 1.7.12, offline Zizmor 1.30.1, ShellCheck, Rust formatting, and diff whitespace pass. Version synchronization, the 0.0.61-to-0.0.62 bump, unpublished-release check, and migration assertions pass. Independent review found no material defect in the eight-file policy delta. The actual workflow proof verified real supported updater 396e01d8 against clean parent 1c22e150 as scaffold-only, then rejected the combined policy commit b04f2d22 against main 966def44; this native proof completed in 37.584 seconds. Before install consolidation, the integrated native check passed all 30 gates in 213.033 seconds: 1,005 Rust tests including both new workflow-boundary cases, 41 JavaScript tests, one performance test, strict scans, packaging, and native npm CLI/MCP installation journeys. JavaScript line coverage is 867/989 (87.66%) against the unchanged 70% minimum. The previous head 879f8d3 passed Complete validation and normal isolated pre-push in 395.79 seconds. That pre-push ran 30 gates, with 87.97% JavaScript coverage. This proof predates the installation-test consolidation; The first revised journey preserved assertions but two npx calls rebuilt the tarball twice (30.21 and 30.00 seconds), so it was not accepted as installation deduplication. The corrected native single `--call` journey passed in 43.833 seconds with exactly one Cargo release build (39.17 seconds), cold MCP version 0.0.62/protocol 2025-11-25, successful CLI help, empty outer stdout, and zero new fixture residue. The existing Rust package assertions passed two tests in 0.565 seconds; the JavaScript suite passed 40 tests in 7.830 seconds. Native report validation accounts for all ten production files and 867/989 covered lines (87.66%; unchanged 70% minimum). Changed-owner Biome checks passed. Independent review cleared the final command and cleanup behavior. The supported updater commit c2a1fae adopted Hard Eng 7eebdaf3 with no gate-list change; `project-typecheck` runs `node --check` over JavaScript files rather than wrapping `tsc --noEmit`, so it remains. After `cargo update` (cc 1.5.1, cfg-if 1.0.5, clap_lex 1.1.1, find-msvc-tools 0.1.14, rustix 1.1.5, syn 3.0.6, thiserror 2.0.21, unicode-ident 1.0.26), `cargo test --locked` passed 1,005 tests. The final combined `check --base origin/main` passed all 29 gates in 166 seconds, including 40 JavaScript tests at 867/989 lines (87.66%; unchanged 70% minimum), one performance test, and the consolidated native npx journey in 50.899 seconds. The existing cache fixture passes, and controlled replays reject outer stdout and preserve exit 7 while verifying cleanup. The installation consolidation adds no test case, dependency, production cache, or wrapper. Existing release scripts and real npm consumer compatibility contracts remain unchanged.
E2E: Passed — actual workflow proof accepted the real canonical updater against its clean parent and rejected the mixed policy branch. The final combined installation used one fresh npx context, initialized MCP before CLI execution, rejected unexpected outer stdout, and validated the complete MCP response and CLI help. Native execution confirmed one source build and cleanup; the hosted bootstrap release must still prove packaged/Cargo parity and publication.
Delivery target: Merge
Delivery: Pending — Complete validation and normal committed-snapshot pre-push, PR #132 current-head checks, exact-head required CI, guarded merge, merged-main checks, one normal bootstrap release, tag/four assets/registry provenance, and isolated published-binary proof. Future source-only publication savings require an actual maintenance merge; local positive proof does not claim that hosted outcome.
