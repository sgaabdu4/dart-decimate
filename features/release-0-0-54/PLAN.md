# Release 0.0.54 with the latest Hard Eng and measured budgets

Status: Ready

## Outcome + scope

The project runs Hard Eng a7693e7, its shipping budgets match measured runs, and pending dependency updates land:
- `ci_seconds` 900 and `pre_push_seconds` 600 replace the 1800 placeholders (measured: CI about 7.6 minutes, pre-push 200–307s);
- `github/codeql-action/upload-sarif` moves to v4.38.1;
- `@types/node` stays on 22.x, the oldest supported Node in `engines.node`, and Dependabot skips its major updates.

The version moves to 0.0.54 because every non-docs PR must bump. Non-goals: project code changes.

## Repository context

Owners: `.hooks/`, `.agents/skills/he*` and `AGENTS.md` (updated by the Hard Eng updater); `hard-eng.gates.json` shipping budgets; `.github/dependabot.yml`; `.github/workflows/security.yml`; version files Cargo.toml, Cargo.lock, package.json, README.md and docs/ci.md.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Autonomous. The owner asked to set the budgets, update Hard Eng and release.

## Acceptance + steps

- [ ] `.hooks/hard-eng-source.json` names a7693e7 → updater output.
- [ ] Full gate passes with the new hooks and budgets → `python3 .hooks/hard-eng.py check --base origin/main`.

## Baseline + execution

Result: Passed
Evidence: Main `bb4e49e` released 0.0.53 with every release job green; CI took 7.6 minutes; Dependabot PRs #118 and #119 fail only because Hard Eng needs a plan.
Execution: One change: bump, budgets and dependencies, update, gate.

## Risks + recovery

A budget below a slow run fails that run; the budgets leave about 2x headroom over measured runs. Recovery: revert the commit.

## ux_reference

N/A — tooling update with no visual surface.

## Verification

Result: Pending
Evidence: Pending
E2E: N/A — no product journey changes; PR CI and the release run cover delivery.

Delivery target: Merge
Delivery: Pending — PR checks green, squash merge, release run publishes 0.0.54.
