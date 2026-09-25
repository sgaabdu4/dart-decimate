# Install Hard Eng and migrate dev tooling to pnpm

Status: Complete

## Outcome + scope

Hard Eng is installed, `python3 .hooks/hard-eng.py check` passes, and every check the old pre-push hook and `.no-mistakes.yaml` ran is still enforced; non-goals: changing analyzer behavior, the published npm package's install flow, or the release workflow.

## Repository context

Owners: `package.json` + `pnpm-lock.yaml` + `pnpm-workspace.yaml` (dev tooling), `hard-eng.gates.json` (gates), `.githooks/` (hooks via `core.hooksPath`), `.github/workflows/ci.yml` + `security.yml` (CI), `npm/` (published wrapper and postinstall), `scripts/` (version and release guards), `docs/ci.md` (developer commands).

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Autonomous; the user asked to install Hard Eng, migrate dev tooling to pnpm, carry the old hook checks into gates, make the full gate pass, bump to 0.0.50 and open a PR against `main` without merging.

## Acceptance + steps

- [x] Dev installs use pnpm → `pnpm install --frozen-lockfile --ignore-scripts` passes from a clean `node_modules` without running the root postinstall, as the old hook's `npm ci --ignore-scripts` did.
- [x] Old hook checks stay enforced → each pre-push and `.no-mistakes.yaml` command maps to a Hard Eng gate or the retained pre-commit hook.
- [x] Full gate → `python3 .hooks/hard-eng.py check` exits 0.
- [x] Version bump → `npm run version:check` and `npm run version:bump:check -- origin/main` pass at 0.0.50.
- [x] Release path → `npm run pack:check`, `npm run test:postinstall:prebuilt` and `npm run test:npx:prebuilt` pass.

## Baseline + execution

Result: Passed
Evidence: `python3 .hooks/hard-eng.py check --base main --plan-stage Draft` at 96c3b58 (Hard Eng f1ac2ba) exited 0 in 164s with all 30 gates passing; JS line coverage 810/925 (87.57%, minimum 70%), branch 104/146 (71.23%, informational); performance samples `check median 20.8ms, max 26.6ms` and `median 53.3ms, max 117.6ms` over 15 runs each on the Flutter fixture app.
Execution: One builder in the existing task worktree; first slice is gate configuration so the full gate can run.

## Risks + recovery

Moving the npm install to pnpm could break CI or release jobs that expect `package-lock.json`; recovery is to fix the failing job at its install step, verified by the PR's CI run.

## ux_reference

N/A — tooling and CI only; no report, CLI or HTML output changes.

## Verification

Result: Passed
Evidence: every acceptance step was run at 96c3b58 or 26d3ca6:
- Dev installs: in a fresh clone at 96c3b58, `pnpm install --frozen-lockfile --ignore-scripts` exited 0 with no postinstall output and no `target/`; without `--ignore-scripts` the root postinstall builds the release binary.
- Old hook checks: `git diff --check` → diff-whitespace; `npm ci --ignore-scripts` → lockfile; `npm run lint` → version-sync, migration-guard, format-lint and rust-format; `version:bump:check` → version-bump; `release:check` → release-version; `cargo fmt --check` → rust-format; clippy → rust-clippy; `fallow audit` → fallow-audit; `cargo test` → rust-tests; `npm test` → tests; `pack:check`, `test:postinstall:prebuilt`, `test:npx:prebuilt`, `test:npx:local` and `test:npx:mcp:local` → pack, postinstall-prebuilt, npx-prebuilt, npx-local and npx-mcp-local; `.githooks/pre-commit` keeps version, release and migration checks.
- Full gate: `python3 .hooks/hard-eng.py check --base main --plan-stage Ready` at 26d3ca6 exited 0 with all 30 gates passing; performance `check median 44.7ms, max 80.3ms` over 15 runs.
- Version bump: `npm run version:check` → `version ok: 0.0.50`; `npm run version:bump:check -- origin/main` → `Cargo.toml 0.0.49 -> 0.0.50; package.json 0.0.49 -> 0.0.50`.
- Release path: pack, postinstall-prebuilt and npx-prebuilt gates passed in the same run.
E2E: Passed — the npx-local, npx-mcp-local, npx-prebuilt and postinstall-prebuilt gates installed and ran the CLI and MCP wrapper through npx and the prebuilt postinstall in the 26d3ca6 gate run.

Delivery target: PR
Delivery: Pending — branch not pushed; PR CI must pass.
