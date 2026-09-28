# Update Hard Eng to 97cc783

Status: Complete

## Outcome + scope

The project runs Hard Eng 97cc783, the latest verified revision, which the freshness check requires before shipping or completion. The version moves to 0.0.61 because every non-docs PR must bump. Non-goals: project code changes.

## Repository context

Owners: `.hooks/`, `.agents/skills/he*` and `AGENTS.md`, updated by the Hard Eng updater; version files Cargo.toml, Cargo.lock, package.json, README.md and docs/ci.md. Evidence: `ship --stage delivered` for PR #124 stopped on "newer verified revision 97cc783". The updater's commit runs the project's pre-commit, which rejects the published 0.0.60, so the bump is committed first.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Autonomous. On 2026-09-29 the owner asked to finish all shipping work; the freshness check requires the supported updater.

## Acceptance + steps

- [x] `.hooks/hard-eng-source.json` names 97cc783 → updater output "Updated Hard Eng to 97cc783…".
- [x] Full gate passes with the new hooks → `python3 .hooks/hard-eng.py check --base origin/main --plan-stage Complete`.

## Baseline + execution

Result: Passed
Evidence: Main `e27fc08` released 0.0.60 with Release and Security green; its content passed the full gate 30/30 on the PR #124 branch. The first updater run failed only on the pre-commit version check.
Execution: One change: bump, update, gate.

## Risks + recovery

New checks can surface findings; they are fixed in their own commits. Recovery: revert the update commit.

## ux_reference

N/A — tooling update with no visual surface.

## Verification

Result: Passed
Evidence: Updater: "Updated Hard Eng to 97cc783afd75c81b08a28f0ce392414aa50cef7e"; `python3 .hooks/hard-eng.py check --base origin/main --plan-stage Complete` → exit 0, 30/30 PASS.
E2E: N/A — no product journey changes; PR CI and the release run cover delivery.

Delivery target: Merge
Delivery: Pending — PR checks green, squash merge, release run publishes 0.0.61.
