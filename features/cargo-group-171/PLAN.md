# Cargo group update, with tree-sitter-dart held at 0.2

Status: Complete

## Outcome + scope

The locked crates `noyalib` 0.0.60, `toml` 1.2.1, `tree-sitter` 0.27.2 and the patch updates Dependabot proposed move in. `tree-sitter-dart` stays at 0.2, because 0.3 renames and restructures grammar nodes such as `class_member`, `enum_constant`, `constructor_param` and `initialized_variable_definition`, and 21 of 411 unit tests fail on it. The version moves to 0.0.81 because the version gates need a version above the base. Non-goals: the tree-sitter-dart 0.3 grammar port and other source changes.

## Sources + coverage

Sources: N/A — the daily dependency run's request is the only source.
Deliverables: None

## Repository context

Owners: Cargo.toml and Cargo.lock (dependency versions); version files Cargo.toml, Cargo.lock, package.json, README.md and docs/ci.md. Evidence: with the 0.3 grammar the extractor, widget and symbol tests fail; with 0.2 all 411 unit tests pass.
Same cause: N/A — one dependency pin.
Single source: N/A — the grammar version lives once in Cargo.toml.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: The package owner approved releasing this package when an update needs it; the daily dependency run asked for this fix.

## Acceptance + steps

- [x] The lock file holds the updated crates and `tree-sitter-dart` 0.2.0 → N/A — a locked version has no behavior to test; `cargo test` passing proves the parser still works.
- [x] Version 0.0.81 is synchronized across the version files → `tests/js/release-guards.test.mjs::version sync accepts matching manifests and lockfile`.

## Baseline + execution

Result: Passed
Evidence: Base main f60190c holds 0.0.80 and passes its Release workflow.
Execution: Single builder: one commit for the pin and version, one for this plan.

## Risks + recovery

The release publishes on merge and a published npm version cannot be reused. Recovery: revert the merge commit and ship a fix as the next patch version. The 0.3 grammar port stays open for the package owner.

## ux_reference

N/A — dependency change with no visual surface.

## Verification

Result: Passed
Evidence: `cargo test` passes 411 unit tests and all integration tests with `tree-sitter-dart` 0.2.0.
E2E: N/A — no code path changes.
Real service: N/A — no changed code calls an outside service.
Retro: Dependabot proposed a grammar major version that needs a port → no change.

Delivery target: Merge
Delivery: Pending — PR checks green, merge, Release run publishes 0.0.81.
