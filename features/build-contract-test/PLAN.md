# Keep a test for the build contract name in an entry file

Status: Complete

## Outcome + scope

A `build` method of a class in an application's `lib/main.dart` stays unreported, and a test proves it again. Commit 86bfd2d removed the only case that checked `build` in `CONTRACT_MEMBER_NAMES`. Since 3fd484e, classes in an application's `lib/main.dart` are checked, so a Flutter `MyApp.build` there depends on that name. Release as 0.0.77, because the version gates need a version above main. Non-goals: other contract names, rule changes.

## Repository context

Owners: `CONTRACT_MEMBER_NAMES` in `src/symbols.rs`; the test `strict_check_checks_the_members_of_classes_declared_in_entry_files` in `tests/cli_unused_public_members.rs`.
Same cause: 86bfd2d removed three cases that a review said proved nothing; one of them was the only check of the `build` name. The other two (`lib/main_dev.dart` with a used member, and `lib/pkg.dart` that the export check protects) proved nothing and stay removed.
Single source: the contract names live once in `CONTRACT_MEMBER_NAMES`.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Autonomous. The owner owns dart-decimate and approved PRs there for this effort (2026-11-15), and asked for every review finding to be fixed.

## Acceptance + steps

- [x] An application's `lib/main.dart` with `class MyApp { void build() {} }` reports no unused member, and the same file with `Plain.spare` reports it → N/A — the installed Hard Eng plan check reads no Rust test names (a Hard Eng fault, being fixed on Hard Eng branch `fix/rust-test-names`, not yet merged or released); the proof is the Rust test `strict_check_checks_the_members_of_classes_declared_in_entry_files` in `tests/cli_unused_public_members.rs`, whose runs Verification records.
- [x] Full gate passes → N/A — a gate run is no product behavior; `python3 .hooks/hard-eng.py check` is the proof.

## Baseline + execution

Result: Passed
Evidence: Base main 86bfd2d holds 0.0.76; its Release workflow passed on 2026-11-16. With `build` taken off `CONTRACT_MEMBER_NAMES`, the full test run on 86bfd2d still passes, so no test guards the name. With the case added back, that change fails the test (`left: ["MyApp.build"]`, `right: []`), and with the name in place all 27 tests in the file pass.
Execution: One commit with the test case; the coordinator runs the full check.

## Risks + recovery

None beyond the test file. Recovery: 0.0.77 is published at merge, so the version gates refuse a plain revert; drop the case in a later release, 0.0.78 or above.

## ux_reference

N/A — this changes a test, not a rendered interface.

## Verification

Result: Passed
Evidence: `cargo test --test cli_unused_public_members`: 27 passed. With `build` renamed in `CONTRACT_MEMBER_NAMES`, the test fails with `left: ["MyApp.build"]`, `right: []`. `python3 .hooks/hard-eng.py check --plan-stage Ready`: all 30 gates PASS. A fresh review of 86bfd2d found the lost case.
Retro: the removed case was the only guard of a contract name → the restored case in the Rust test `strict_check_checks_the_members_of_classes_declared_in_entry_files`; CI refused this plan because the installed plan check reads no Rust test names → Hard Eng fault, being fixed on Hard Eng branch `fix/rust-test-names`; before removing a test case, its guard was not checked by mutation → None — the review caught it, and the same review step runs on every final head.
E2E: N/A — the change adds one test case; the CLI behavior does not change.

Delivery target: Merge
Delivery: Pending
