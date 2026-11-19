# Pass the Hard Eng release candidate

Status: Complete

## Outcome + scope

The repository passes the Hard Eng release candidate. The `fallow-audit` gate that wrapped Fallow in `sh -c` is gone. The `dead-code-duplicates` gate now runs dead code, duplicates and health on the whole project, which covers what the audit checked. Five Rust lint suppressions are fixed at their cause. The Fallow settings drop the switched-off rules and the entry globs. Release as 0.0.80. Non-goals: other rules, other files, new features.

## Sources + coverage

Sources: N/A — the coordinator's request is the only source.
Deliverables: None

## Repository context

Owners:
- `hard-eng.gates.json` owns the gates; `.fallowrc.json` owns the Fallow settings.
- `src/graph.rs` and `src/graph/parts.rs` own the dependency edge code; `src/trace.rs` owns the dependency trace report.
- `tests/cli_flutter_style_usage.rs` owns the theme extension test.
- Version owners: `Cargo.toml`, `Cargo.lock`, `package.json`, `README.md` and `docs/ci.md`.

Same cause: the candidate refuses a Rust lint suppression or a switched-off Fallow rule wherever it stands; a search of `src` and `tests` for `#[allow(`, `#[expect(` and `#[cfg_attr(` finds no other place.
Single source: N/A — no rule, value or type is added; one builder struct now holds the edge state that three functions took as arguments.

## Decisions + authorization

Blockers: None
Handoff: None
Authority: Autonomous. The coordinator asked for this repository to pass the release candidate, delivered as a PR that stops before merge for an independent review. The version gates need a version above main because the Rust source changes.

## Acceptance + steps

- [x] The dependency graph keeps its edges, unresolved imports and part checks → N/A — the installed plan check reads no Rust test names; the Rust tests `builds_edges_for_part_directives`, `reports_missing_part_of_for_resolved_part` and `records_unresolved_known_package_import_target` in `src/graph/tests.rs` pass unchanged in the gate.
- [x] The dependency trace JSON keeps every field name → N/A — the installed plan check reads no Rust test names; the Rust test `tooling_config_usage_counts_as_dependency_usage` in `tests/cli_dependencies.rs` reads `used_in_scripts` and `used_in_codegen` from the JSON and passes unchanged in the gate.
- [x] The theme extension test checks the same findings → N/A — the installed plan check reads no Rust test names; the Rust test `theme_extension_aliases_and_proven_factories_mark_owned_fields_used` in `tests/cli_flutter_style_usage.rs` passes with its sources held in constants.
- [x] The Fallow scan checks dead code, duplicates and health on the whole project → N/A — the `dead-code-duplicates` gate of `python3 .hooks/hard-eng.py check` is the proof.
- [x] Version 0.0.80 is the same in every owner → N/A — a version is not a behavior; the `version-sync`, `version-bump` and `release-version` gates are the proof.

## Baseline + execution

Result: Passed
Evidence: Starting code origin/main f182818. The candidate refuses in three steps: the `fallow-audit` gate runs `sh -c exec fallow audit`; then five Rust suppressions in `src/graph.rs`, `src/graph/parts.rs`, `src/trace.rs` and `tests/cli_flutter_style_usage.rs`; then the Fallow rules `boundary-violation` and `policy-violation` set off and two entry globs.
Execution: One agent; one commit per cause.

## Risks + recovery

The trace report moves its found, declared and used flags into two flattened structs. The JSON field names stay the same and only their order changes. A Rust user of the library reads `report.status.found` where it read `report.found`; the crate has `publish = false` and the README documents only the command line, so no published Rust interface changes. Merging to main starts the Release workflow, which publishes 0.0.80. Recovery: a forward fix in a new patch version.

## ux_reference

N/A — no user interface changes.

## Verification

Result: Passed
Evidence: `cargo test --all-targets` passes 1108 tests and `cargo clippy --all-targets -- -D warnings` is clean. `python3 .hooks/hard-eng.py check --base origin/main --plan-stage Complete` exits 0 with every gate PASS. The same check in a sealed copy updated to Hard Eng develop a7909cdb, with the main tip as base, exits 0 with every gate PASS.
E2E: N/A — no user journey changes.
Real service: N/A — no changed code calls an outside service.
Retro: Failed check: the shell-wrapped Fallow gate, five Rust suppressions and the switched-off Fallow rules → each fixed at its cause and the candidate check stays the prevention.
Delivery target: Merge
Delivery: Pending — PR, then merge after review, then confirm the Release workflow.
