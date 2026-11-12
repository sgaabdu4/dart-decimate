# Release arm64 runner

Status: Complete

## Outcome + scope

The macOS arm64 release job runs on `macos-15`, so the Release workflow can build and publish again. The version moves to 0.0.73 because the version gates need a version above the base. Version 0.0.72 was never published: its Release run was cancelled and no tag `v0.0.72` exists. The change also tracks the `post-checkout` launcher that setup wrote into the tracked hooks folder and lists it in the shell gate, because the full check in the main checkout called the untracked file a big change. Non-goals: other runner changes, dependency updates and source changes.

## Repository context

Owners: `.github/workflows/release.yml` (build matrix); version files Cargo.toml, Cargo.lock, package.json, README.md and docs/ci.md. Evidence: GitHub answered "The configuration 'macos-14-arm64-us-default' is not supported" for the `Build dart-decimate-darwin-arm64.tar.gz` job, because it retired the macOS 14 images on 2026-11-02. A search of the repository finds `macos-14` only in that matrix entry; the Intel job already uses `macos-15-intel`.
Same cause: Fix → retired runner labels in workflows; searched all files for `macos-14`, `macos-13` and `macos-12`; only the one matrix entry used a retired label, now fixed.
Single source: N/A — the runner label lives once per matrix entry in `release.yml`; no other file repeats it.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: The package owner approved releasing this package when an update needs it; the coordinator asked for this repair in the session.

## Acceptance + steps

- [x] The arm64 build job uses a supported runner → N/A — a hosted runner label cannot be proven by a local test; the Release run after merge is the proof.
- [x] Version 0.0.73 is synchronized across the version files → `tests/js/release-guards.test.mjs::version sync accepts matching manifests and lockfile`.
- [x] The version is above the base version → `tests/js/release-guards.test.mjs::version bump rejects manifests that disagree`.
- [x] The worktree hook is tracked and the shell gate lints it → N/A — a tracked launcher and a gate line have no behavior of their own to test; the `shellcheck` gate of the full check reads the file.

## Baseline + execution

Result: Passed
Evidence: Base main 7bcc070 holds 0.0.72. The Release run on that commit ended cancelled at the arm64 build job.
Execution: Single builder: one runner commit, one version commit with this plan, one hook commit.

## Risks + recovery

The release publishes on merge and a published npm version cannot be reused. Recovery: revert the merge commit and ship a fix as the next patch version.

## ux_reference

N/A — workflow change with no visual surface.

## Verification

Result: Passed
Evidence: the full check with `--plan-stage Complete` exits 0 on the final commit. Not checked: the Release run on a hosted runner, which only runs after merge.
E2E: N/A — the change affects only the hosted release build, which no local journey can run.
Retro: the Release run was cancelled by a retired runner label → this release run after merge, plus the `Same cause:` search above; no repository test can read a hosted runner label.

Delivery target: Merge
Delivery: Pending — PR checks green, merge, Release run publishes 0.0.73.
