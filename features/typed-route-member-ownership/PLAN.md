# Resolve typed-route registry member ownership in cycle checks

Status: Complete

## Outcome + scope

Clear typed navigation back-edges when an unrelated object's member happens to share a name with a route-data member, including a `State` reading `widget.<field>`, while retaining real non-navigation reads of imported route data and conservative treatment of unknown receivers. Non-goals: inferring types from untyped initializers or prefixed registry imports.

## Repository context

Owners: `src/output/route_cycle/classification.rs` (member ownership), `navigation.rs` (receiver type resolution, which already types `context` in a `State`), `state_context.rs` (the `State<W>` widget type) and `tests/cli_issue_26_false_positives.rs`. Evidence: after the issue 126 fix, a `State` reading `widget.user` still reports a cycle when a route declares `user`; the classifier counts any `receiver.<name>` whose name matches a registry member.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Autonomous. The owner approved this plan on 2026-09-28 and asked to fix `widget.user` too; delivery follows the issue 126 PR, stacked on its branch.

## Acceptance + steps

- [x] `widget.user` in a `State` of a local widget that declares `user` clears → CLI test, 0 findings.
- [x] Typed receivers of a local DTO and of a model from another library clear → CLI test, 0 findings.
- [x] A receiver typed as a route class, or as a local type extending one, still reports → CLI test, 1 error each.
- [x] Unknown receivers still report: untyped local, `dynamic` and type-parameter receivers → CLI test, 1 error each.
- [x] Existing typed-route cycle behavior holds → `cargo test --test cli_issue_26_false_positives` passes.
- [x] Full gate passes → `python3 .hooks/hard-eng.py check --base origin/main --plan-stage Complete`.

## Baseline + execution

Result: Passed
Evidence: Branch base `bdd2633` (issue 126 PR head) passed the full Complete gate, 30/30. The built CLI reports 1 cycle error for a `State` reading `widget.user`.
Execution: One builder: failing CLI tests first, then receiver typing and ownership, version 0.0.57.

## Risks + recovery

Clearing a receiver whose type is guessed wrong would hide a real registry read. Only declared types clear; untyped, `dynamic`, type-parameter and route-class receivers, and local types extending a route class, still count, as do registry extension members on any receiver. Recovery: revert the commit.

## ux_reference

N/A — this changes static CLI findings, not a rendered interface.

## Verification

Result: Passed
Evidence: Both clearing tests failed before the change (1 error each); `cargo test --test cli_issue_26_false_positives` → 85 passed and `cargo test --all-targets` → 1003 passed after it. Disabling each condition in turn fails a test: `widget` typing, route-class owner, local owner extending a route class, registry extension member, `dynamic`, type parameter, and unknown receivers. Full gate on the branch merged with main: `python3 .hooks/hard-eng.py check --base origin/main --plan-stage Complete` → exit 0, 30/30 PASS.
E2E: Passed — built CLI: a `State` reading `widget.user` → `pass`, 0 cycles; a screen reading `route.user` from an `InviteRoute` field → `fail`, 1 cycle; the issue 126 fixture → `pass`, 0 cycles.

Delivery target: Merge
Delivery: Pending — after the issue 126 PR merges: PR checks green, squash merge, release run publishes 0.0.57.
