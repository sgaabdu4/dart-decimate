# CodeQL upload-sarif 4.39.0 and dev tool freshness

Status: Complete

## Outcome + scope

The Scorecard workflow uploads its SARIF with `github/codeql-action/upload-sarif` 4.39.0, and the dev tools meet the new `freshness` gate: @biomejs/biome 2.6.3 and @types/node 26.6.5. The version moves to 0.0.70, because every merge releases. Non-goals: source changes, other dependency moves and new features.

## Repository context

Owners: `.github/workflows/security.yml`, `.github/dependabot.yml`, `package.json`, `bun.lock`; version files Cargo.toml, Cargo.lock, package.json, README.md and docs/ci.md. Evidence: Dependabot PR 151 failed "Rust and npm checks" at `check-pr-version-bump.mjs` ("Cargo.toml version must be bumped: 0.0.68 -> 0.0.68"). Hard Eng f10f7dfa (merged as 0.0.69) adds `freshness`, which reported "@biomejs/biome 2.5.15 → 2.6.3; @types/node 22.20.5 → 26.6.5". The `undici-types` 6.23.0 override existed only because @types/node 22.x ranges resolved untrusted 6.19-6.21 releases; @types/node 26.6.5 needs `~8.9.0`, and 8.9.0 has SLSA provenance, so the override is removed. The Dependabot rule that held @types/node at major 22 now conflicts with `freshness`, so it is removed.
Same cause: Fix → version bump on every merge, as in `features/dependabot-bun/PLAN.md` and `features/hard-eng-update-f10f7dfa/PLAN.md` (helped: both released cleanly). No other open PR carries an unbumped change.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Autonomous scheduled maintenance. The owner approved fixing failing Dependabot PRs, merging and releasing for this task.
Decisions: Upgrade @types/node to the newest major instead of a freshness hold, because the owner does not allow check escape hatches. engines.node stays at >=22.12.0.

## Acceptance + steps

- [x] The SARIF upload step pins 4.39.0 → N/A — the Security workflow run on main after merge proves the action; no repository test runs workflows.
- [x] Version 0.0.70 is synchronized across the version files → `tests/js/release-guards.test.mjs::version sync accepts matching manifests and lockfile`.
- [x] 0.0.70 is not yet on npm → `tests/js/release-guards.test.mjs::release check accepts a version the registry does not have`.
- [x] Scripts still type-check and parse with @types/node 26 → `tests/js/typecheck-js.test.mjs::JS syntax check passes valid scripts in every checked root`.
- [x] Direct dev dependencies are on their newest release at least 2 days old → N/A — the `freshness` gate itself proves it in the same check.

## Baseline + execution

Result: Passed
Evidence: Base main 5ad1f09 (Hard Eng f10f7dfa, 0.0.69). PR 151 failed only the version-bump step; a local check on the update branch passed every gate except `freshness`.
Execution: Single builder: rebase PR 151 on main, upgrade the two dev tools, remove the stale override and Dependabot rule, bump, check.

## Risks + recovery

@types/node 26 can describe APIs that Node 22 lacks; the scripts use only long-standing `node:` modules and the npm install tests run the real scripts. The release publishes on merge. Recovery: revert the merge commit and ship a fix as the next patch version.

## ux_reference

N/A — CI and dev tool update with no visual surface.

## Verification

Result: Passed
Evidence: `python3 .hooks/hard-eng.py check --base origin/main --plan-stage Complete` on this branch; the pre-push gate and PR CI rerun the full check.
E2E: Passed — `postinstall-prebuilt`, `npx-prebuilt` and `npx-local` install the packed tarball through the real postinstall and run the CLI in the same check.

Delivery target: Merge
Delivery: Pending — PR checks green, squash merge, release run publishes 0.0.70.
