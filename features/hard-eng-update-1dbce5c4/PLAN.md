# Update Hard Eng to 1dbce5c4

Status: Complete

## Outcome + scope

The project runs Hard Eng 1dbce5c4e3098107d717cc4fcacba6c8649ff4a8. Two scripts build their constant patterns once at module level, because the new security rule flags patterns built inside functions. The version moves to 0.0.72 because `package.json` and `npm/scripts/` change, so the update is not scaffold-only. Non-goals: dependency upgrades, behavior changes and new tests.

## Repository context

Owners: `.hooks/`, `.agents/skills/he*` and `.claude/` via the Hard Eng updater; `npm/scripts/postinstall.js` and `scripts/check-pr-version-bump.mjs` for the patterns; version files Cargo.toml, Cargo.lock, package.json, README.md and docs/ci.md. Evidence: the first `update-pr` push was refused by the pre-push gate with `version-bump` ("Cargo.toml version must be bumped: 0.0.71 -> 0.0.71"), `release-version` ("dart-decimate@0.0.71 is already published") and `security` (4 findings of `regex-built-in-function-javascript`).
Same cause: Fix → the same version bump for a non-scaffold Hard Eng update, as in `features/hard-eng-update-cf66b385/PLAN.md`; the only other constant patterns built inside functions were the 4 findings, now fixed.
Single source: N/A — each pattern moved to one module-level constant in the file that uses it; no copy exists in another file.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: The package owner approved the 0.0.72 release for this update in the session; the normal Release workflow publishes it after merge.

## Acceptance + steps

- [x] `.hooks/hard-eng-source.json` names 1dbce5c4 → N/A — the updater commit "Update Hard Eng to 1dbce5c4e3098107d717cc4fcacba6c8649ff4a8" proves it; no repository test reads the marker.
- [x] Version 0.0.72 is synchronized across the version files → `tests/js/release-guards.test.mjs::version sync accepts matching manifests and lockfile`.
- [x] The version-bump script still compares versions as before → `tests/js/release-guards.test.mjs::version bump rejects manifests that disagree`.
- [x] The install script still reports release asset errors → `tests/js/postinstall.test.mjs::postinstall reports a missing release asset and the Rust fallback`.

## Baseline + execution

Result: Passed
Evidence: Base main 20e9d45 released 0.0.71. The first pre-push run failed only `version-bump`, `release-version` and `security`.
Execution: Single builder: updater commit, two pattern commits, one version commit with this plan.

## Risks + recovery

The release publishes on merge and a published npm version cannot be reused. Recovery: revert the merge commit and ship a fix as the next patch version.

## ux_reference

N/A — tooling update with no visual surface.

## Verification

Result: Passed
Evidence: after the pattern commits the full check passed every gate except `version-bump` and `release-version`; the check on the version commit is rerun with `--plan-stage Complete`. Not checked: the release workflow on a hosted runner.
E2E: Passed — the pre-push gate's `postinstall-prebuilt`, `npx-prebuilt` and `npx-local` checks install the packed tarball and run the CLI.
Retro: security finding → `npm/scripts/postinstall.js` and `scripts/check-pr-version-bump.mjs` module-level patterns, checked by the `security` gate; version gate failures → the version bump commit and `tests/js/release-guards.test.mjs::version bump rejects manifests that disagree`.

Delivery target: Merge
Delivery: Pending — PR checks green, merge, release run publishes 0.0.72.
