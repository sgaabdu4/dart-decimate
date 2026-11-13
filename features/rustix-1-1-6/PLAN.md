# Rustix 1.1.6

Status: Complete

## Outcome + scope

The locked `rustix` crate moves from 1.1.5 to 1.1.6, the first release that fixes RUSTSEC-2026-0486 (GHSA-jh2m-fhpj-3vr7). The version moves to 0.0.74 because the version gates need a version above the base. Non-goals: other dependency updates and source changes.

## Repository context

Owners: `Cargo.lock` (locked versions); version files Cargo.toml, Cargo.lock, package.json, README.md and docs/ci.md. Evidence: only `tempfile` pulls in `rustix`, and no source file calls it. `cargo update -p rustix --precise 1.1.6` changes only `rustix`.
Same cause: N/A — one transitive crate; `cargo tree -i rustix` shows one version.
Single source: N/A — the locked version lives once in Cargo.lock.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: The package owner approved releasing this package when an update needs it; the coordinator asked for this fix in the session.

## Acceptance + steps

- [x] The lock file holds `rustix` 1.1.6 → N/A — a locked version has no behavior to test; `cargo tree -i rustix` and the OSV scan of Cargo.lock prove it.
- [x] Version 0.0.74 is synchronized across the version files → `tests/js/release-guards.test.mjs::version sync accepts matching manifests and lockfile`.

## Baseline + execution

Result: Passed
Evidence: Base main 145e1c1 locks `rustix` 1.1.5 and holds 0.0.73.
Execution: Single builder: one commit.

## Risks + recovery

The release publishes on merge and a published npm version cannot be reused. Recovery: revert the merge commit and ship a fix as the next patch version.

## ux_reference

N/A — lock file change with no visual surface.

## Verification

Result: Passed
Evidence: the full check exits 0, `cargo tree -i rustix` shows 1.1.6 and the OSV scan of Cargo.lock is clean.
E2E: N/A — no code path changes.
Retro: the advisory is `unsound` only, so `cargo audit` warned and no gate blocked 1.1.5 → no change; the audit step reports it as a warning by design.

Delivery target: Merge
Delivery: Pending — PR checks green, merge, Release run publishes 0.0.74.
