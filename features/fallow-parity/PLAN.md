# Close the Fallow parity gaps in the default gate

Status: Complete

## Outcome + scope

Six kinds of finding that a TypeScript code health scanner fails and `check . --threshold 0 --strict` passes today now fail it: an unused public class member, an export name declared in two library files, an unused dependency override, a misconfigured dependency override, a package cycle with no file cycle, and a CRAP score equal to the limit. Release as 0.0.76. The new `package-cycle` rule gets the same `--package-cycles` filter flag as the other rules. Non-goals: other rules, publishing.

## Repository context

Owners: `src/symbols` and `src/output/symbol_findings.rs` (class members, duplicate exports), `src/dependencies` (overrides), `src/health/coverage.rs` (CRAP), `src/graph.rs` and `src/output` (cycles), `tests/cli_*.rs` (CLI tests), `src/manifest.rs`, `src/explain.rs`, `docs/issue-types.md`, README.md (rule lists). Version owners: Cargo.toml, Cargo.lock, package.json, README.md, docs/ci.md. The repo has no CHANGELOG file; the issue type docs and explain entries are the rule record.
Same cause: N/A — six separate rule gaps measured on tiny samples; no earlier fix of the same cause.
Single source: each rule id lives once in `FindingKind` plus the alias, suppression, manifest and docs tables that already list the other rules; the new rule extends those tables.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: Autonomous. The owner asked for these six changes, one commit each, one draft PR, no publish and no release tag.

## Acceptance + steps

- [x] An unused public method, getter or field of a plain class fails `check --strict`; annotated members, classes with supertypes, generated-part libraries, publicly exported classes and `toString`/`toJson` do not → `tests/js/fallow-parity.test.mjs::an unused public method of a plain class fails the strict check` and `::a class member that another file uses keeps the strict check green`; Rust tests in `tests/cli_unused_public_members.rs` cover annotations, generated parts and exports.
- [x] The same public name in two library files fails `check --strict`; platform alternatives, private names and files outside `lib/` do not → `tests/js/fallow-parity.test.mjs::one public name declared in two library files fails the strict check` and `::platform alternatives that share a name keep the strict check green`; `tests/cli_duplicate_declarations.rs` also covers private names and files outside `lib/`.
- [x] An override that the resolved dependency graph never reaches fails `check --strict`, although the lock file lists it; an override a dependency needs, a missing graph and a graph of another package do not → `tests/js/fallow-parity.test.mjs::an override that the dependency graph never reaches fails the strict check` and `::an override stays when the dependency graph file is missing`; `tests/cli_dependencies.rs` also covers an override a dependency needs and a graph of another package.
- [x] A blank string override fails `check --strict` as misconfigured; an override of a package that nothing depends on is reported once, as unused → `tests/js/fallow-parity.test.mjs::a blank string override fails the strict check as misconfigured`.
- [x] Two workspace packages that depend on each other fail `check --strict` with `package-cycle` and no file cycle; one-way dependencies do not → `tests/js/fallow-parity.test.mjs::two workspace packages that depend on each other fail the strict check` and `::workspace packages with one-way dependencies keep the strict check green`.
- [x] A CRAP score equal to `max_crap` is reported, in the global limit and in an override → `tests/js/fallow-parity.test.mjs::a CRAP score equal to the limit is reported`.
- [x] The existing fixtures keep their results, except two intended changes → `cargo test --all-targets` passes: `src/symbols/tests.rs::reports_unused_enum_constants_and_private_class_members` now lists `publicUnused`, and `tests/cli_unused_widget_param.rs::check_counts_only_matching_widget_object_pattern_fields_as_usage` now expects the one `duplicate-export` error.
- [x] Full gate passes → N/A — a gate run is no product behavior; `python3 .hooks/hard-eng.py check --base origin/main --plan-stage Complete` is the proof.

## Baseline + execution

Result: Passed
Evidence: Base main b8a1609 holds 0.0.75 and its Release workflow passed on 2026-11-13. A measurement on tiny samples showed `check . --threshold 0 --strict` exits 0 for each of the six kinds. On the base code, the new tests for the unused public member, the duplicate name and the package cycle fail (exit 0 where 1 is expected), and the CRAP and override tests failed before their fixes.
Execution: One builder: failing test first, then the fix, one commit per kind; then version 0.0.76, docs, the comment repair and the biome update, each as its own commit.

## Risks + recovery

New findings can fail projects that pass today. Each rule starts from the narrowest safe reading and keeps 0 findings on the existing fixtures. Recovery: revert the commit of one rule; each rule is its own commit.

## ux_reference

N/A — this changes static CLI findings, not a rendered interface.

## Verification

Result: Passed
Evidence: `cargo test --all-targets` passes after the two intended expectation changes. Each of the four item commits compiles alone (`cargo check --all-targets`). The full Hard Eng check passed every gate but freshness, which found biome 2.6.7 and was repaired in its own commit. The shortened comment blocks keep clippy pedantic green.
Retro: old multi-line comment blocks in touched files failed the check → HE-COMMENT-LINES caught it, repaired in its own commit; a newer biome passed the age limit mid-task → the freshness gate caught it, updated in its own commit; a hunk-splitting script slip misplaced lines in one commit → None — caught by diffing the tree against a backup and by `cargo check` on each commit, and no repository rule can prevent a local script slip.
E2E: Passed — the built binary ran `check . --threshold 0 --strict --format json` on 35 parity samples. The six former misses now exit 1: unused-class-member, duplicate-export, unused-dependency-override, misconfigured-dependency-override, package-cycle and high-crap-score. The clean samples exit 0, and the `crap` sample without a `max_crap` setting still exits 0 by design.

Delivery target: Merge
Delivery: Pending — draft PR, checks green, ready for review; the owner merges and the Release workflow publishes.
