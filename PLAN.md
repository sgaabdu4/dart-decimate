# pnpm tooling and Claude/Codex scaffold migration

Status: Complete

## Outcome + scope

Use pnpm for repository tooling, generated CI and hooks, and native publication while retaining npm consumer compatibility and the existing Cargo/npm release parity contract. Adopt the released Hard Eng scaffold and retire obsolete agent wiring.

## Repository context

Owners: `package.json`, `scripts/`, and `hard-eng.gates.json` own checks; `src/ci_template.rs` and `src/hooks/templates.rs` generate commands; `tests/` owns native regression and package compatibility contracts. Existing CI, Security, and Release workflows own PR verification, audits, and tagged publication. Unrelated feature work and the open dependency update remain outside this PR.

## Decisions + authorization

Blockers: None
Handoff: Ready-for-ship
Authority: The user authorized one combined repository cleanup PR, pnpm migration, scaffold update, merge, and the resulting native package release. Keep the existing quality and compatibility assertions; add no alternate runner or artificial tests.

## Acceptance + steps

- [x] Migrate supported npm/npx producers to pnpm while preserving npm registry metadata and actual npm/npx consumer tests.
- [x] Preserve published-version, source/tag, provenance, and Cargo/npm parity assertions for version 0.0.55.
- [x] Use native pnpm bootstrap in generated CI and local-only pnpm execution in agent hooks.
- [x] Audit the actual PR/push comparison and block native failure verdicts in generated agent hooks.
- [x] Retire obsolete no-mistakes configuration and unsupported agent/tool wiring through the released installer.
- [x] Preserve all native checks, benchmark, and coverage; review the final public diff.

## Baseline + execution

Result: Passed
Evidence: The committed 0.0.55 candidate passed all 30 native gates against published Hard Eng d2085f745de39214aaaf6b34192378c9d1094a3d before installation. The initial baseline had passed native tests and scans but refused installation because 0.0.54 was unchanged and already published; its target remained unchanged.
Additional baseline evidence: A native two-commit fixture proved that comparing HEAD with itself skips a newly introduced finding. A real cyclic-import audit returned exit 1 and a failure verdict, but the existing generated agent hook returned 0. Bare pnpm exec also installed dependencies and ran root postinstall before executing its command.
Execution: Bumped the authorized release version, adapted existing native producers and assertions, and committed the reviewed application changes before the successful updater run. Its isolated candidate starts from committed HEAD; an uncommitted retry was stopped after identifying its stale 0.0.54 snapshot and is not verification of this change. The released installer then committed the verified scaffold update.

## Risks + recovery

pnpm pack returns one metadata object; release parity must install that actual tarball with npm. Native pnpm publishing uses existing OIDC and provenance permissions; the already-verified detached release checkout needs only publication's `--no-git-checks`. Registry transport/authentication failures remain blocking. The existing rejection test uses a native HTTP 401 response to protect the owned fail-closed lookup outcome without waiting for pnpm's HTTP 500 backoff; production retry behavior is unchanged. Its temporary projects and caches now use native test cleanup. Retain existing tagged-source repair guards; never overwrite an unrelated release or treat dry-run packaging as hosted publication proof.

## ux_reference

N/A — this migration changes CLI installation, generated commands, and repository checks; it does not alter the rendered HTML report.

## Verification

Result: Passed
Evidence: Integrated candidate passed 996 Rust tests, 37 JavaScript tests, and the existing performance check (13.5ms median, 17.5ms maximum over 15 runs). Line coverage is 818/932 (87.77%); the first actual pre-push passed in 267.97 seconds. After the fixture change, all 22 existing release-guard tests pass in 2.844 seconds with zero new owned temporary-directory residue; native registry rejection takes 125.6ms rather than the previous 70.16-second retry wait. All native scans, format/type checks, version/registry guards, package checks, and prebuilt/npx CLI/MCP compatibility passed before this test-only follow-up. The existing generated-shell regression proves failure verdict exit 1 blocks with exit 2 while runtime errors remain allowed. Independent release and hook review found no remaining defect; the final public diff preserves the native assertions and removes only obsolete registrations and configuration.
E2E: Passed — isolated native pnpm bootstrap and pinned dlx resolved the binary with Cargo disabled; the local 0.0.55 tarball passed the existing CLI and MCP consumer journeys. Native before-commit and empty-tree comparisons detect an injected finding. A real cycle blocks the hook, a runtime error remains allowed, and mismatched package-manager/runtime pins cause no bootstrap, lockfile, or postinstall.
Delivery target: Merge
Delivery: Pending — require native pre-push, current PR and merged-commit checks, then verify the native GitHub release/tag and npm version produced by the exact merged revision. The retained release workflow must pass Cargo/npm install parity and hosted OIDC publication; local package proof does not replace those checks.
