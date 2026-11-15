# Close the Fallow parity gaps in the default gate

Status: Draft

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

- [ ] An unused public method, getter or field of a class no reachable file uses fails `check --strict`; overrides, `build`, `createState`, interface members and generated `toJson`/`fromJson` do not → CLI test.
- [ ] The same export name in two library files of one package fails `check --strict` → CLI test.
- [ ] An override of a package the pubspec dependency graph never needs fails `check --strict`, although the lock file lists it → CLI test.
- [ ] A blank, malformed or unresolvable override entry fails `check --strict` → CLI test.
- [ ] Two workspace packages that depend on each other fail `check --strict` with a package cycle, with no file cycle → CLI test.
- [ ] A CRAP score equal to `max_crap` is reported → unit test at exactly the limit.
- [ ] A clean sample and the existing fixtures keep 0 findings → `cargo test --all-targets`.
- [ ] Full gate passes → `python3 .hooks/hard-eng.py check --base origin/main --plan-stage Complete`.

## Baseline + execution

Result: Pending
Evidence: Base main b8a1609 holds 0.0.75. A measurement on tiny samples showed `check . --threshold 0 --strict` exits 0 for each of the six kinds.
Execution: One builder: failing test first, then the fix, one commit per kind; then version 0.0.76 and docs.

## Risks + recovery

New findings can fail projects that pass today. Each rule starts from the narrowest safe reading and keeps 0 findings on the existing fixtures. Recovery: revert the commit of one rule; each rule is its own commit.

## ux_reference

N/A — this changes static CLI findings, not a rendered interface.

## Verification

Result: Pending
Evidence: Pending.
E2E: Pending — the built binary runs on each measurement sample.

Delivery target: Merge
Delivery: Pending — draft PR, checks green, ready for review; the owner merges and the Release workflow publishes.
