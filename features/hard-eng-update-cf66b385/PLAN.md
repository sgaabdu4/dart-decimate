# Update Hard Eng to cf66b385

Status: Complete

## Outcome + scope

The project runs Hard Eng cf66b3851c79ef226694e82085c8c86aa9394a49, the latest CI-verified revision. The update rewrites the CI tool-folder lines in `.github/workflows/ci.yml`, so it is not scaffold-only and the merge publishes a release; the version moves to 0.0.71. Non-goals: dependency upgrades, source changes and behavior changes.

## Repository context

Owners: `.hooks/`, `.agents/skills/he*`, `.claude/` and `.github/workflows/ci.yml` via the Hard Eng updater; version files Cargo.toml, Cargo.lock, package.json, README.md and docs/ci.md. Evidence: the first `update-pr` push was refused by the pre-push gate with `version-bump` ("Cargo.toml version must be bumped: 0.0.70 -> 0.0.70") and `release-version` ("dart-decimate@0.0.70 is already published"); every other gate passed. `check_scaffold_update` against origin/main returns False, so `release.yml` treats the merge as a release.
Same cause: Fix → version bump for a non-scaffold Hard Eng update, as in `features/hard-eng-update-f10f7dfa/PLAN.md` (0.0.69 released cleanly).

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: The owner gave full authority on 2026-10-31 to land the Hard Eng update in every repository, including merges; AGENTS.md requires the Hard Eng update PR, which merges on green.

## Acceptance + steps

- [x] `.hooks/hard-eng-source.json` names cf66b385 → N/A — the updater output "Update Hard Eng to cf66b3851c79ef226694e82085c8c86aa9394a49" proves it; no repository test reads the marker.
- [x] Version 0.0.71 is synchronized across the version files → `tests/js/release-guards.test.mjs::version sync accepts matching manifests and lockfile`.
- [x] 0.0.71 is not yet on npm → `tests/js/release-guards.test.mjs::release check accepts a version the registry does not have`.
- [x] Existing behavior is unchanged → N/A — no source file changes; the full check reruns the Rust, JS and npm install tests.

## Baseline + execution

Result: Passed
Evidence: Base main cafc41c released 0.0.70. The first pre-push run failed only `version-bump` and `release-version`. npm lists 0.0.68, 0.0.69 and 0.0.70, not 0.0.71.
Execution: Single builder: updater commit, then one version-bump commit with this plan.

## Risks + recovery

The release publishes on merge and an npm version cannot be reused. Recovery: revert the merge commit and ship a fix as the next patch version.

## ux_reference

N/A — tooling update with no visual surface.

## Verification

Result: Passed
Evidence: the pre-push gate reruns the full check, including version-sync, version-bump, release-version, tests and rust-tests, on this commit; PR CI repeats it.
E2E: Passed — the pre-push gate's `postinstall-prebuilt`, `npx-prebuilt` and `npx-local` checks install the packed tarball and run the CLI.
