# Remove the scheduled Codex maintenance workflow

Status: Complete

## Outcome + scope

The repository no longer runs the nightly Codex maintenance workflow; version moves to 0.0.52 because every non-docs PR must bump. Non-goals: other workflows.

## Repository context

Owners: `.github/workflows/codex-maintenance.yml` (self-contained: nightly cron, `codex-maintenance` environment, Gmail variables and secret). Nothing else references it; the repository has no `codex-maintenance` environment, secrets or variables configured.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Autonomous. The owner asked to remove scheduled maintenance.

## Acceptance + steps

- [x] The workflow file is gone and nothing references it → `git grep codex-maintenance` finds only this plan.
- [x] Version 0.0.52 in Cargo.toml, Cargo.lock, package.json, README.md and docs/ci.md → version-sync and version-bump gates pass.

## Baseline + execution

Result: Passed
Evidence: Main `5ced0c3` released 0.0.51 with every release job green.
Execution: One change: delete the workflow and bump the version.

## Risks + recovery

Maintenance PRs no longer open automatically. Recovery: restore the file from git history.

## ux_reference

N/A — CI workflow removal with no visual surface.

## Verification

Result: Passed
Evidence: `python3 .hooks/hard-eng.py check --base origin/main --plan-stage Ready` → exit 0, 30/30 gates PASS, 194s; `git grep codex-maintenance` finds only this plan.
E2E: N/A — removing a scheduled workflow has no runtime journey; PR CI and the release run cover delivery.

Delivery target: Merge
Delivery: Pending — PR checks green, squash merge, release run publishes 0.0.52.
