# Count named constructors at the end of arrow callbacks

Status: Complete

## Outcome + scope

A widget built with a named constructor at the end of an arrow callback, `(c) => B.named()` or `(c) => cond ? A() : B.named()` (nested conditionals included), counts as constructed for `unrendered-widget`. Release as 0.0.64. Non-goals: other expression shapes and other rules.

## Repository context

Owners: tree-sitter-dart parses `(c) => B.named()` as `((c) => B).named()`, a `call_expression` whose `function` is a `member_expression` with a `function_expression` object, so the tail `B` loses its constructor call. `function_expression_body_constructor_name` in `src/widgets/unrendered.rs` already recovers the split for unnamed constructors and conditional else branches; it now also recovers the named form. `tests/cli_unrendered_widget.rs` owns the CLI tests. Version owners: Cargo.toml, Cargo.lock, package.json, README.md, docs/ci.md.

## Decisions + authorization

Blockers: None
Handoff: Ready for ship
Authority: The user authorized this fix in one new PR, a squash merge through ship, and the normal Release workflow publishing 0.0.64.

## Acceptance + steps

- [x] `(context) => BodyCard.named()` and `(context) => ready ? const SizedBox() : ElseCard.named(title: 'a')` count as constructed; an unused widget with only a named constructor is still reported.
- [x] Nested conditional else branches ending in a named constructor count as constructed.
- [x] Existing invoked-closure member and bare-type cases keep their results.
- [x] Version 0.0.64 is synchronized across the established owners.

## Baseline + execution

Result: Passed
Evidence: Base main 3430c9b passed its Release workflow, which published 0.0.63. A synthetic widget fixture reported `ElseCard` as unrendered for `(context) => ElseCard.named()` and for the conditional form before the fix.
Execution: One builder changes the existing owner and adds one CLI test, then runs the native gate.

## Risks + recovery

The recovery only applies when the member object is an unparenthesized arrow closure and the tail is a bare capitalized identifier, so ordinary member calls and invoked closures are unaffected. Recovery is a normal revert and patch release.

## ux_reference

N/A — command-line analysis change with no user interface.

## Verification

Result: Passed
Evidence: `cargo test --locked --test cli_unrendered_widget` passes 10 tests; with the `unrendered.rs` change stashed, the new test fails. The built debug binary reports only the unused card for the plain, conditional and nested conditional named-constructor forms. `check-version-sync` and `check-pr-version-bump` pass for 0.0.63 to 0.0.64; tag v0.0.64 and npm 0.0.64 did not exist. The full native `check --base origin/main` result is recorded in the PR.
E2E: Passed — the built debug binary ran `check` on the synthetic reproduction project, and the CLI entrypoint ran `check` on the new fixture project.
Delivery target: Merge
Delivery: Pending — PR checks, squash merge, Release workflow publication of v0.0.64 to GitHub and npm.
