# Adopt pnpm/setup v3 in CI workflows

Status: Complete

## Outcome + scope

The CI, Security and Release workflows install Node and pnpm with `pnpm/setup` v3.0.0 (Dependabot PR #124) with unchanged runtimes and no dependency install. Non-goals: the `dart-decimate ci-template` output and `docs/ci.md` example, which keep their own pin.

## Repository context

Owners: `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `.github/workflows/security.yml`. Evidence: v3's breaking changes are a run-scoped cache key and automatic Node version-file detection. Each step sets an explicit `runtime: node@<version>`, which v3 applies before version files, with `install: false` and no `cache` input. The repository has no `.node-version`, `.nvmrc` or `.tool-versions`. Hard Eng's `migrate_workflow_pins` only rewrites the older v2.0.0 pin.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Autonomous. On 2026-09-29 the owner asked to merge all open PRs where needed; Hard Eng requires a plan and version bump for this non-docs change.

## Acceptance + steps

- [x] CI and Security install Node and pnpm with v3 → PR CI `Install pnpm` and `Install Node and pnpm` steps succeed.
- [x] Workflow lint and security checks accept the v3 pin → `actionlint` and `zizmor` gates pass.
- [x] Full gate passes → `python3 .hooks/hard-eng.py check --base origin/main --plan-stage Complete`.

## Baseline + execution

Result: Passed
Evidence: `python3 .hooks/hard-eng.py check --base origin/main --plan-stage Draft` on the PR branch merged with main `3ce7d66` → 28/30; only `version-bump` and `release-version` failed, because the branch had no version bump. `actionlint` and `zizmor` passed with the v3 pin.
Execution: One builder: this plan and version 0.0.60 on the Dependabot branch; merge after PR #130 takes 0.0.59.

## Risks + recovery

A changed runtime or an unexpected install would alter every workflow. The explicit runtime input and `install: false` keep both unchanged. Recovery: revert the commit.

## ux_reference

N/A — CI workflow configuration only; no rendered interface.

## Verification

Result: Passed
Evidence: Version 0.0.60 added; `python3 .hooks/hard-eng.py check --base origin/main --plan-stage Complete` → exit 0, 30/30 PASS.
E2E: Passed — PR #124 CI run 36448280678: `Install pnpm` with v3 succeeded (the job then stopped only at the missing plan), and the Security `Dependency audit` job's `Install Node and pnpm` and `pnpm audit` succeeded.

Delivery target: Merge
Delivery: Pending — PR checks green, squash merge, Release run installs Node and pnpm with v3 and publishes 0.0.60.
