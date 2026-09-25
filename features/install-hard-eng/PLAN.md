# Install Hard Eng and migrate dev tooling to pnpm

Status: Draft

## Outcome + scope

Hard Eng is installed, `python3 .hooks/hard-eng.py check` passes, and every check the old pre-push hook and `.no-mistakes.yaml` ran is still enforced; non-goals: changing analyzer behavior, the published npm package's install flow, or the release workflow.

## Repository context

Owners: `package.json` + `pnpm-lock.yaml` + `pnpm-workspace.yaml` (dev tooling), `hard-eng.gates.json` (gates), `.githooks/` (hooks via `core.hooksPath`), `.github/workflows/ci.yml` + `security.yml` (CI), `npm/` (published wrapper and postinstall), `scripts/` (version and release guards), `docs/ci.md` (developer commands).

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Autonomous; the user asked to install Hard Eng, migrate dev tooling to pnpm, carry the old hook checks into gates, make the full gate pass, bump to 0.0.50 and open a PR against `main` without merging.

## Acceptance + steps

- [ ] Dev installs use pnpm → `pnpm install --frozen-lockfile` passes from a clean `node_modules` without running the root postinstall.
- [ ] Old hook checks stay enforced → each pre-push and `.no-mistakes.yaml` command maps to a Hard Eng gate or the retained pre-commit hook.
- [ ] Full gate → `python3 .hooks/hard-eng.py check` exits 0.
- [ ] Version bump → `npm run version:check` and `npm run version:bump:check -- origin/main` pass at 0.0.50.
- [ ] Release path → `npm run pack:check`, `npm run test:postinstall:prebuilt` and `npm run test:npx:prebuilt` pass.

## Baseline + execution

Result: Pending
Evidence: Pending — `python3 .hooks/hard-eng.py check --plan-stage Draft` on the installed scaffold.
Execution: One builder in the existing task worktree; first slice is gate configuration so the full gate can run.

## Risks + recovery

Moving the npm install to pnpm could break CI or release jobs that expect `package-lock.json`; recovery is to fix the failing job at its install step, verified by the PR's CI run.

## ux_reference

N/A — tooling and CI only; no report, CLI or HTML output changes.

## Verification

Result: Pending
Evidence: Pending
E2E: N/A — no user-facing runtime change; the npm and npx install journeys are covered by the existing `test:npx:*` and `test:postinstall:prebuilt` checks.

Delivery target: PR
Delivery: Pending — PR CI must pass.
