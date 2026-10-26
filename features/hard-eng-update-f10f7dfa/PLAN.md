# Update Hard Eng to f10f7dfa

Status: Complete

## Outcome + scope

The project runs Hard Eng f10f7dfa985c6804e6ad2522cef0a12b1fdc7b59, the latest CI-verified revision. The update adds a `freshness` gate to `hard-eng.gates.json`, so it is not scaffold-only and the merge publishes a release; the version moves to 0.0.69. Non-goals: dependency upgrades, source changes and behavior changes.

## Repository context

Owners: `.hooks/`, `.agents/skills/he*` and `hard-eng.gates.json` via the Hard Eng updater; version files Cargo.toml, Cargo.lock, package.json, README.md and docs/ci.md. Evidence: the first `update-pr` push was refused by the pre-push gate with `version-bump` ("Cargo.toml version must be bumped: 0.0.68 -> 0.0.68") and `release-version` ("dart-decimate@0.0.68 is already published"); every other gate passed. `hard-eng.gates.json` is not an updater-installed file, so `release.yml` treats the merge as a release.
Same cause: Fix → version bump for a non-scaffold Hard Eng update, as in `features/hard-eng-update-02f02959/PLAN.md` (helped: 0.0.65 released cleanly). The open Dependabot PR 151 fails the same version check; it gets its own bump to 0.0.70 after this merges, because both cannot release 0.0.69.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Autonomous scheduled maintenance. The owner approved merging and releases for this task; AGENTS.md requires the Hard Eng update PR, which merges on green.

## Acceptance + steps

- [x] `.hooks/hard-eng-source.json` names f10f7dfa → N/A — the updater output "Hard Eng update f10f7dfa985c6804e6ad2522cef0a12b1fdc7b59 ready" proves it; no repository test reads the marker.
- [x] Version 0.0.69 is synchronized across the version files → `tests/js/release-guards.test.mjs::version sync accepts matching manifests and lockfile`.
- [x] 0.0.69 is not yet on npm → `tests/js/release-guards.test.mjs::release check accepts a version the registry does not have`.
- [x] Existing behavior is unchanged → N/A — no source file changes; the full check reruns the Rust, JS and npm install tests.

## Baseline + execution

Result: Passed
Evidence: Base main b6c0a10 runs Hard Eng c567397 and released 0.0.68. The first pre-push run failed only `version-bump` and `release-version`; the second needed this plan because package.json changed.
Execution: Single builder: updater commit, then one version-bump commit with this plan.

## Risks + recovery

The release publishes on merge and an npm version cannot be reused. Recovery: revert the merge commit and ship a fix as the next patch version.

## ux_reference

N/A — tooling update with no visual surface.

## Verification

Result: Passed
Evidence: `python3 .hooks/hard-eng.py check --base origin/main --plan-stage Complete` on this branch: 30 gates PASS, including version-sync, version-bump, release-version, tests and rust-tests. The new `freshness` gate reports @biomejs/biome 2.5.15 → 2.6.3 and @types/node 22.20.5 → 26.6.5; the update PR skips it by design, and the baseline repair goes in the next PR. The pre-push gate and PR CI rerun the full check.
E2E: Passed — `postinstall-prebuilt`, `npx-prebuilt` and `npx-local` install the packed tarball through the real postinstall and run the CLI in the same check.

Delivery target: Merge
Delivery: Pending — PR checks green, squash merge, release run publishes 0.0.69.
