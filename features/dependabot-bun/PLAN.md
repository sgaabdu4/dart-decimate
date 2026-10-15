# Dependabot on Bun

Status: Complete

## Outcome + scope

Dependabot updates the JavaScript dependencies again after the move from pnpm to Bun. Out of scope: the Cargo and GitHub Actions entries, grouping, cooldown and ignore rules.

## Repository context

Owners: `.github/dependabot.yml`.
Evidence: After the Bun migration merged, the Dependabot run for the `npm` entry failed: "This project has a `bun.lock` file but no `package-lock.json`, `yarn.lock` or `pnpm-lock.yaml`, which means it is managed by Bun."

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Follow-up to the owner's Hard Eng update request for this repository; merge when CI is green.
Decisions: Change only the ecosystem name to `bun`; keep the same directory, schedule, cooldown, group and ignore rules.

## Acceptance + steps

- [x] The JavaScript entry uses the `bun` ecosystem → `.github/dependabot.yml`.
- [x] Repository checks pass → `python3 .hooks/hard-eng.py check --base origin/main`.

## Baseline + execution

Result: Passed
Evidence: Base `main` after the Bun migration merge.
Execution: Single builder.

## Risks + recovery

Dependabot validates the file on push; a rejected key shows on the Dependabot run. Recovery = revert the PR.

## ux_reference

N/A — repository tooling only.

## Verification

Result: Passed
Evidence: Hard Eng check on the branch.
E2E: N/A — the next Dependabot run on `main` proves the entry; checked after merge.

Delivery target: Merge
Delivery: Pending — PR merged into main with CI passing, then a Dependabot run on main succeeds.
