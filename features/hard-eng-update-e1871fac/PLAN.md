# Update Hard Eng to e1871fac

Status: Complete

## Outcome + scope

The project runs Hard Eng e1871fac9032e10b7a9f30de353c3bbfe08ea39c, which requires Bun. Repository tooling moves from pnpm to Bun: lockfile, package scripts, JS tests, release guards, hooks and CI. The JS tests run scripts in the test process, because Bun reports coverage only for code loaded there and the gate requires every production file in the report. The version moves to 0.0.66 because `package.json`, `scripts/`, `tests/` and the workflows change, so the update is not scaffold-only. Non-goals: new dependencies, and any change to what the npm package ships or how consumers install it (`pnpm dlx` and npx stay supported).

## Repository context

Owners: `package.json`, `bun.lock`, `bunfig.toml`, `hard-eng.gates.json`, `.github/workflows/*.yml`, `.githooks/pre-commit`, `scripts/*.mjs`, `tests/js`, `tests/npm`; version files Cargo.toml, Cargo.lock, package.json, README.md and docs/ci.md. Evidence: the check refuses a pnpm project; `bun test` runs the existing `node:test` files; `bun --check` runs a script instead of parsing it, so the syntax check uses `bun build --no-bundle`; `bun pm pack` and `bun pm view` replace `pnpm pack` and `pnpm view`; Bun cannot publish with provenance, so the release job runs `bun x npm@12.2.0 publish` with the same flags.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Autonomous. The owner approved the Hard Eng update and its Bun migration; the normal Release workflow publishes 0.0.66 after merge.

## Acceptance + steps

- [x] `python3 .hooks/hard-eng.py check --base origin/main` and the full `check` exit 0.
- [x] The packed tarball installs and runs through the real postinstall → `postinstall-prebuilt`, `npx-prebuilt` and `npx-local` pass.
- [x] Version 0.0.66 is synchronized across the established owners → `version-sync` and `version-bump` pass.

## Baseline + execution

Result: Passed
Evidence: Base main 8dddd61 passes with pnpm; the updated scaffold fails with "Bun is required".
Execution: One builder migrates the tooling, then runs the gates.

## Risks + recovery

The release publishes on merge and a published npm version cannot be reused. Recovery: revert the merge commit and ship a fix as 0.0.67.

## ux_reference

N/A — tooling update with no visual surface.

## Verification

Result: Passed
Evidence: `python3 .hooks/hard-eng.py check --base origin/main` and the full `check` exit 0. `bun run test:coverage` runs 43 tests and lists all 10 files under `npm/` and `scripts/`. `bun audit --prod` reports no vulnerabilities. Not checked: the release and security workflows on a hosted runner, and the runner's default Node against `engines`.
E2E: Passed — `postinstall-prebuilt`, `npx-prebuilt` and `npx-local` install the packed tarball and run the CLI and MCP server in the same check.

Delivery target: Merge
Delivery: Pending — PR checks green, merge, release run publishes 0.0.66.
