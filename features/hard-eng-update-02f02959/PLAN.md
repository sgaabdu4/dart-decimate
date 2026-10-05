# Update Hard Eng to 02f02959

Status: Ready

## Outcome + scope

The project runs Hard Eng 02f0295910c5a6b88f23e8c8c008a5de1865bf2c, the latest CI-verified revision, and its JSON reads and type assertions satisfy the new untrusted-input typing. The version moves to 0.0.65 because the change touches `npm/`, `scripts/`, `tests/` and `tsconfig.json`, so it is not scaffold-only. Non-goals: new dependencies, behaviour changes for valid input, Rust code.

## Repository context

Owners: `.hooks/`, `.agents/skills/he*`, `AGENTS.md` and `tsconfig.json` via the Hard Eng updater; `npm/scripts/postinstall.js`, `scripts/*.mjs`, `tests/js`, `tests/npm` and `tests/performance` for the JSON reads; version files Cargo.toml, Cargo.lock, package.json, README.md and docs/ci.md. Evidence: Hard Eng 02f02959 types `JSON.parse` and `Response.json()` as `unknown` and fails `as T` and JSDoc casts; `release.yml` publishes on a non-scaffold main push and rejects a reused 0.0.64; `hard-eng.gates.json` runs `version-bump` on every non-scaffold PR. The release-guard scripts and `postinstall.js` run without `node_modules`, so they narrow with built-in checks instead of importing a validator.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Autonomous. The owner asked for every Hard Eng project to be updated to 02f02959 as one PR, squash-merged when CI is green; the normal Release workflow then publishes 0.0.65.

## Acceptance + steps

- [ ] `.hooks/hard-eng-source.json` names 02f02959 → updater output "Updated Hard Eng to 02f02959…".
- [ ] Every JSON read and type assertion passes `types` and `typing-style` without a new dependency → `python3 .hooks/hard-eng.py check --base origin/main --plan-stage Complete` exits 0.
- [ ] Valid input behaves as before → JS tests, npm install tests and Rust tests pass in the same check.
- [ ] Version 0.0.65 is synchronized across Cargo.toml, Cargo.lock, package.json, README.md and docs/ci.md → `version-sync` and `version-bump` pass.

## Baseline + execution

Result: Passed
Evidence: Base main 2a744e8 runs Hard Eng 1b0cdd9 and released 0.0.64; npm and the local tags stop at 0.0.64. The updater has not yet run on this branch.
Execution: One change: migrate the JSON reads, bump, update, gate.

## Risks + recovery

The release publishes on merge and a published npm version cannot be reused. Recovery: revert the merge commit and ship a fix as 0.0.66.

## ux_reference

N/A — tooling update with no visual surface.

## Verification

Result: Pending
Evidence: Pending — the full gate has not run on the updated branch.
E2E: Required — npm install and npx journeys run in `tests/npm`; record Passed evidence after the full gate.

Delivery target: Merge
Delivery: Pending — PR checks green, squash merge, release run publishes 0.0.65.
