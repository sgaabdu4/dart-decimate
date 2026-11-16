# Keep a test for the build contract name in an entry file

Status: Ready

## Outcome + scope

A `build` method of a class in an application's `lib/main.dart` stays unreported, and a test proves it again. Commit 416d6bb removed the only case that checked `build` in `CONTRACT_MEMBER_NAMES`. Since 2d1b8a5, classes in an application's `lib/main.dart` are checked, so a Flutter `MyApp.build` there depends on that name. Non-goals: other contract names, rule changes, a release.

## Repository context

Owners: `CONTRACT_MEMBER_NAMES` in `src/symbols.rs`; the test `strict_check_checks_the_members_of_classes_declared_in_entry_files` in `tests/cli_unused_public_members.rs`.
Same cause: 416d6bb removed three cases that a review said proved nothing; one of them was the only check of the `build` name. The other two (`lib/main_dev.dart` with a used member, and `lib/pkg.dart` that the export check protects) proved nothing and stay removed.
Single source: the contract names live once in `CONTRACT_MEMBER_NAMES`.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Autonomous. The owner owns dart-decimate and approved PRs there for this effort (2026-11-15), and asked for every review finding to be fixed.

## Acceptance + steps

- [x] An application's `lib/main.dart` with `class MyApp { void build() {} }` reports no unused member, and the same file with `Plain.spare` reports it → `tests/cli_unused_public_members.rs::strict_check_checks_the_members_of_classes_declared_in_entry_files`.
- [x] Full gate passes → N/A — a gate run is no product behavior; `python3 .hooks/hard-eng.py check` is the proof.

## Baseline + execution

Result: Passed
Evidence: Base main 86bfd2d holds 0.0.76; its Release workflow passed on 2026-11-16. With `build` taken off `CONTRACT_MEMBER_NAMES`, the full test run on 86bfd2d still passes, so no test guards the name. With the case added back, that change fails the test (`left: ["MyApp.build"]`, `right: []`), and with the name in place all 27 tests in the file pass.
Execution: One commit with the test case; the coordinator runs the full check.

## Risks + recovery

None beyond the test file. Recovery: revert the commit.

## ux_reference

N/A — this changes a test, not a rendered interface.

## Verification

Result: Pending
Evidence: Pending
Retro: Pending
E2E: N/A — the change adds one test case; the CLI behavior does not change.

Delivery target: Merge
Delivery: Pending
