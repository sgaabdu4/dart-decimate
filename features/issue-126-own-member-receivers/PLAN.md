# Clear typed back-edges whose screen uses its own this.<field>

Status: Complete

## Outcome + scope

A typed GoRouter back-edge stays cleared when the helper's initializing formals (`this.x`, `super.x`) or `this.x` / `super.x` expressions share a name with a route-registry member. Non-goals: other receivers such as `widget.user` (see the Draft plan `features/typed-route-member-ownership/`).

## Repository context

Owners: `src/output/route_cycle/classification.rs` (`identifier_is_declaration_name`, `helper_references_imported_api`), `registry_api.rs` (visible extension members) and `tests/cli_issue_26_false_positives.rs`. Evidence: [issue 126](https://github.com/sgaabdu4/dart-decimate/issues/126); the parser gives `constructor_param` / `super_formal_parameter` an unnamed `identifier` child, and `this.x` / `super.x` member nodes have no `object` field.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Autonomous. The owner asked to fix issue 126 without false positives or negatives, then merge the PR.

## Acceptance + steps

- [x] Issue 126 fixture reports no cycle → CLI test, 0 `circular-dependency` findings.
- [x] `super.user` super-parameter plus `this.user` assignment, `??` and null-aware reads and `super.build` clear → CLI test, 0 findings.
- [x] `this.user` inside a helper extension on a route class still reports → CLI test, 1 error.
- [x] `this.x` that resolves to a registry extension member still reports → CLI test, 1 error.
- [x] Existing static and extension member import errors stay → the full `cli_issue_26_false_positives` suite passes.
- [x] Full gate passes → `python3 .hooks/hard-eng.py check --plan-stage Complete`.

## Baseline + execution

Result: Passed
Evidence: Draft gate on main `0150e1e` passed every check except the expected `version-bump`. Built 0.0.55 CLI on the issue fixture → 1 `circular-dependency` error; renaming the screen field → 0 findings.
Execution: One builder: classifier change, CLI tests, version bump to 0.0.56; after the release parity repair commit.

## Risks + recovery

Exempting `this.x` inside an extension on a route class would hide a real route-data read; the exemption skips extension bodies and a test pins it. Recovery: revert the commit.

## ux_reference

N/A — CLI findings only; no rendered interface changes.

## Verification

Result: Passed
Evidence: New tests failed before the fix (1 error each for the two clear cases); `cargo test --test cli_issue_26_false_positives` → 82 passed after it. Disabling each branch in turn (initializing formals, `this`/`super` receivers, route-class header, extension members) fails its matching test. Known gap: a `State` reading `widget.user` still reports; that receiver belongs to `features/typed-route-member-ownership/`.
E2E: Passed — built CLI on the issue's exact files → `pass`, 0 cycles; the renamed-field control → 0 cycles.

Delivery target: Merge
Delivery: Pending — PR checks green, squash merge, release run publishes 0.0.56.
