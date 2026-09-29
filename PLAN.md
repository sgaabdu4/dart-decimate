# Count run commands in shell scripts as dependency usage

Status: Complete

## Outcome + scope

A dependency invoked as `dart run <pkg>[:<exe>]`, `dart pub run <pkg>` or `flutter pub run <pkg>` from a `.sh`, `.bash` or `.zsh` script anywhere in its package counts as used, so a Flutter app's `scripts/upload_sentry_symbols.sh` running `dart run sentry_dart_plugin` no longer reports that dev dependency as unused. Release as 0.0.63. Non-goals: loose name matching in scripts outside `tool/`, other script languages, and configured Dart-discovery `ignore_patterns`.

## Repository context

Owners: `src/dependency_scripts.rs` owns tooling usage and previously read shell scripts only under `tool/`. `src/dependencies/analyze.rs` (unused-dependency findings) and `src/trace.rs` (`trace-dependency`) are its callers. `src/dependencies/discovery.rs` walks packages with `project_walk(root, root, &IgnoreMatcher::default())`, which applies `.gitignore`, skipped tool directories and nested checkouts; script discovery reuses that walk. Tests extend `tests/cli_dependencies.rs` and the existing unit module. Version owners: Cargo.toml, Cargo.lock, package.json, README.md, docs/ci.md.

## Decisions + authorization

Blockers: None
Handoff: Ready for ship
Authority: The user authorized fixing this dependency-detection defect, one PR, a squash merge without admin override, and the repository's normal main release flow.

## Acceptance + steps

- [x] A `scripts/` shell script running `dart run sentry_dart_plugin` makes the dev dependency used in `check` and `trace-dependency` (`used_in_scripts: true`).
- [x] A script that only comments, echoes or runs `dart pub get` with the name keeps the dependency unused.
- [x] Detection accepts the three command forms, `:exe`, leading options, command paths and line continuations, and ignores commented commands.
- [x] Scripts belong to their nearest package; each package walks its tree once rather than once per dependency. Existing `tool/`, workflow, Makefile and config detection is unchanged.
- [x] Version 0.0.63 is synchronized across the established owners and README documents the new usage source.

## Baseline + execution

Result: Passed
Evidence: Base main c0b8c94 passed Release run 36517168678 and Security run 36517168716. The reproduction fixture (dev dependency `sentry_dart_plugin`, `scripts/upload_sentry_symbols.sh` running `dart run sentry_dart_plugin`) returned `used_in_scripts: false, is_used: false` from `trace-dependency` before the fix.
Execution: One builder changes the existing owner and its two callers, then runs the native gate.

## Risks + recovery

A loose match would hide truly unused dependencies, so only the three run forms count and comments are dropped. Walking large trees per dependency would slow analysis, so each package is walked once. Recovery is a normal revert and patch release.

## ux_reference

N/A — command-line analysis change with no user interface.

## Verification

Result: Passed
Evidence: After the fix the reproduction fixture returns `used_in_scripts: true, is_used: true`. `cargo test --locked --test cli_dependencies` passes 17 tests; with the `src/` change stashed, the new positive test fails and the negative test passes. The unit test covering the three command forms passes. `check-version-sync` and `check-pr-version-bump` pass for 0.0.62 to 0.0.63; tag v0.0.63 and npm 0.0.63 did not exist. The full native `check --base origin/main` result is recorded in the PR.
E2E: Passed — the built debug binary ran `trace-dependency` on the reproduction project, and the CLI entrypoint ran `check` and `trace-dependency` on real fixture projects in the integration tests.
Delivery target: Merge
Delivery: Pending — PR checks, squash merge, Release workflow publication of v0.0.63 to GitHub and npm.
