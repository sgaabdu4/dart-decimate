# pnpm tooling and Claude/Codex scaffold migration

Status: Draft

## Outcome + scope

Use pnpm for repository tooling, generated CI and hooks, and native publication while retaining npm consumer compatibility and the existing Cargo/npm release parity contract. Adopt the released Hard Eng scaffold and retire obsolete agent wiring.

## Repository context

Owners: `package.json`, `scripts/`, and `hard-eng.gates.json` own checks; `src/ci_template.rs` and `src/hooks/templates.rs` generate commands; `tests/` owns native regression and package compatibility contracts. Existing CI, Security, and Release workflows own PR verification, audits, and tagged publication. Unrelated feature work and the open dependency update remain outside this PR.

## Decisions + authorization

Blockers: Integrated verification against published Hard Eng revision d2085f745de39214aaaf6b34192378c9d1094a3d is pending.
Handoff: Clarification
Authority: The user authorized one combined repository cleanup PR, pnpm migration, scaffold update, merge, and the resulting native package release. Keep the existing quality and compatibility assertions; add no alternate runner or artificial tests.

## Acceptance + steps

- [ ] Migrate supported npm/npx producers to pnpm while preserving npm registry metadata and actual npm/npx consumer tests.
- [ ] Preserve published-version, source/tag, provenance, and Cargo/npm parity assertions for version 0.0.55.
- [ ] Use native pnpm bootstrap in generated CI and local-only pnpm execution in agent hooks.
- [ ] Audit the actual PR/push comparison and block native failure verdicts in generated agent hooks.
- [ ] Retire obsolete no-mistakes configuration and unsupported agent/tool wiring through the released installer.
- [ ] Preserve all native checks, benchmark, and coverage; verify the final public diff and exact required CI results.

## Baseline + execution

Result: Failed
Evidence: Fresh released updater candidate passed 37 JavaScript tests at 87.89% line coverage, Rust format/clippy/all-target tests, performance (13.0ms median, 17.5ms max), native scans, and prebuilt/npx CLI/MCP compatibility. It refused installation because 0.0.54 was unchanged and already published. The candidate overlapped another local suite; no timing assertion failed. The target remained unchanged.
Additional baseline evidence: A native two-commit fixture proved that comparing HEAD with itself skips a newly introduced finding. A real cyclic-import audit returned exit 1 and a failure verdict, but the existing generated agent hook returned 0. Bare pnpm exec also installed dependencies and ran root postinstall before executing its command.
Execution: Bump the authorized release version, adapt existing native producers and assertions, and commit the reviewed application changes before running the updater, whose isolated candidate starts from committed HEAD. An uncommitted retry was stopped after identifying its stale 0.0.54 snapshot and is not verification of this change. Repeat the candidate gate in a coordinated full-suite slot before final pre-push verification.

## Risks + recovery

pnpm pack returns one metadata object; release parity must install that actual tarball with npm. Native pnpm publishing uses existing OIDC and provenance permissions; the already-verified detached release checkout needs only publication's `--no-git-checks`. Registry transport/authentication failures remain blocking. pnpm 12.6 ignores the configured retry count in `view`; the real server-error test retains its roughly 70-second retry delay. Retain existing tagged-source repair guards; never overwrite an unrelated release or treat dry-run packaging as hosted publication proof.

## ux_reference

N/A — this migration changes CLI installation, generated commands, and repository checks; it does not alter the rendered HTML report.

## Verification

Result: Pending
Evidence: Existing release-guard tests pass all 22 cases through pnpm; published/unpublished cases also pass with the parent repair exception enabled. Native pnpm bootstrap and pinned dlx passed with isolated state, including Cargo-disabled binary resolution. Pack dry-run confirmed the 0.0.55 metadata object. Native before-commit and empty-tree comparisons detect the injected finding. The revised hook blocks a native cycle verdict with exit 2 and preserves runtime-error handling with exit 0; deliberately mismatched package-manager/runtime pins cause no bootstrap, lockfile, or postinstall. Generated YAML/actionlint, shellcheck, JavaScript syntax/Biome, Rust format and comment policy pass. Integrated updated-source checks remain to run.
E2E: Required — generated pnpm commands, local agent-hook outcomes, package CLI/MCP initialization, and the existing Cargo/npm install parity must pass through their native boundaries.
Delivery target: Merge
Delivery: Pending — require current PR and merged-commit checks, then verify the native GitHub release/tag and npm version produced by the exact merged revision. Hosted OIDC publication remains unproven until the release workflow succeeds.
