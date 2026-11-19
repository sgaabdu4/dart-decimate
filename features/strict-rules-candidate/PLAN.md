# Pass the Hard Eng release candidate

Status: Ready

## Outcome + scope

The repository passes the Hard Eng release candidate. The `fallow-audit` gate that wrapped Fallow in `sh -c` is gone because the `dead-code-duplicates` gate already runs the same scan on the whole project. Five Rust lint suppressions are fixed at their cause. The Fallow settings drop the switched-off rules and the entry globs. Release as 0.0.80. Non-goals: other rules, other files, new features.

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

- [x] The dependency graph keeps its edges, unresolved imports and part checks → `src/graph/tests.rs::builds_edges_for_part_directives`, `src/graph/tests.rs::reports_missing_part_of_for_resolved_part` and `src/graph/tests.rs::records_unresolved_known_package_import_target` pass unchanged.
- [x] The dependency trace JSON keeps every field name → `tests/cli_dependencies.rs::tooling_config_usage_counts_as_dependency_usage` reads `used_in_scripts` and `used_in_codegen` from the JSON and passes unchanged.
- [x] The theme extension test checks the same findings → `tests/cli_flutter_style_usage.rs::theme_extension_aliases_and_proven_factories_mark_owned_fields_used` passes with its sources held in constants.
- [x] The Fallow scan keeps its dead-code, duplicate and health checks → N/A — the `dead-code-duplicates` gate of `python3 .hooks/hard-eng.py check` is the proof.
- [x] Version 0.0.80 is the same in every owner → `scripts/check-version-sync.mjs`, run by the `version-sync` gate.

## Baseline + execution

Result: Passed
Evidence: Starting code origin/main f182818. The candidate refuses in three steps: the `fallow-audit` gate runs `sh -c exec fallow audit`; then five Rust suppressions in `src/graph.rs`, `src/graph/parts.rs`, `src/trace.rs` and `tests/cli_flutter_style_usage.rs`; then the Fallow rules `boundary-violation` and `policy-violation` set off and two entry globs.
Execution: One agent; one commit per cause.

## Risks + recovery

The trace report moves its found, declared and used flags into two flattened structs. The JSON field names stay the same and only their order changes. A Rust user of the library reads `report.status.found` where it read `report.found`. Merging to main starts the Release workflow, which publishes 0.0.80. Recovery: a forward fix in a new patch version.

## ux_reference

N/A — no user interface changes.

## Verification

Result: Pending
Evidence: Pending
E2E: N/A — no user journey changes.
Real service: N/A — no changed code calls an outside service.
Retro: Pending
Delivery target: Merge
Delivery: Pending — PR, then merge after review, then confirm the Release workflow.
