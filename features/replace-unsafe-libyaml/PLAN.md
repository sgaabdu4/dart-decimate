# Replace unsafe-libyaml and fold in pending dependency updates

Status: Complete

## Outcome + scope

`unsafe-libyaml` leaves `Cargo.lock`, so RUSTSEC-2024-0449 no longer applies. The YAML parser moves from `serde_yaml_ng` 0.10.0 to `noyalib` 0.0.58. The same PR carries the four open Dependabot updates, because each Dependabot PR fails the plan check on its own. Release as 0.0.68. Non-goals: other dependency moves, YAML behavior changes, and new features.

## Repository context

Owners: `Cargo.toml`, `Cargo.lock`; YAML readers in `src/dependencies.rs`, `src/dependencies/{overrides,flutter_usage,pubspec_entry}.rs`, `src/package_map.rs`, `src/dependency_scripts.rs`, `src/cli/entry_points.rs`, `src/generated.rs`; parse error types in `src/graph.rs`, `src/scan.rs`, `src/dependencies.rs`; `tests/ci_version_gate.rs`; `.github/workflows/{release,security}.yml`; `package.json`, `bun.lock`; version files Cargo.toml, Cargo.lock, package.json, README.md and docs/ci.md.
Evidence: `cargo tree -i unsafe-libyaml` showed `unsafe-libyaml 0.2.11 <- serde_yaml_ng 0.10.0 <- dart-decimate`. RUSTSEC-2024-0449 says the crate is unmaintained, has no fixed version, and suggests saphyr, serde-saphyr or noyalib. `serde_yaml_ng` 0.10.0 is its latest release.

## Decisions + authorization

Blockers: None
Handoff: Approval
Authority: The owner approved the full fix: replace the YAML crate, fold in the Dependabot updates, merge when CI passes, and let the Release workflow publish.
Decisions: `noyalib` keeps a YAML `Value` enum and an order-preserving `Mapping` with string keys, so call sites change little and dependency order is unchanged. `serde-saphyr` has no value type; going through `serde_json::Value` would sort keys, because this build does not enable `preserve_order`. `noyalib` errors are boxed in the three error enums, because clippy rejects the larger error size. One `package_map::parse_yaml` reader keeps the old parser's rules: duplicate keys fail, and the alias-to-anchor ratio limit is off. Dependabot #138 (pnpm era) is superseded by #143.

## Acceptance + steps

- [x] `cargo tree -i unsafe-libyaml` finds no such package, and `cargo audit` reports no warnings.
- [x] `check` JSON output from the new build matches the 0.0.67 build on six local Flutter projects, except the version string.
- [x] Override keys keep their source order; numeric keys now report their source text instead of a debug form.
- [x] A pubspec with a duplicate key still fails to parse, and a pubspec that reuses one anchor eleven times still parses → `package_map` tests.
- [x] Dependabot updates applied: serde_json 1.0.154, toml 1.1.8, tree-sitter 0.27.1; upload-artifact 7.0.2, download-artifact 8.0.2, scorecard-action 2.5.0, codeql-action upload-sarif 4.38.3; @biomejs/biome 2.5.15, @types/node 22.20.5.
- [x] Version 0.0.68 is synchronized across the established owners.

## Baseline + execution

Result: Passed
Evidence: Base main 2432cab. `osv-scanner scan source -r .` reported RUSTSEC-2024-0449 for `unsafe-libyaml` 0.2.11. All four Dependabot PRs failed "Run Hard Eng checks" with "repository changes need an applicable PLAN.md".
Execution: Single builder; Codex adversarial review before push.

## Risks + recovery

`noyalib` is young (0.0.x). It forbids unsafe code and parsed every real pubspec, lock and workflow file in the comparison the same way. By default it keeps the last duplicate key and rejects more than ten aliases per anchor; `parse_yaml` turns both off to match the old parser. Its index operator panics on a missing key, so the code uses `get`. Recovery is a normal revert and patch release.

## ux_reference

N/A — command-line dependency change with no user interface.

## Verification

Result: Passed
Evidence: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test --all-targets` pass. `check-version-sync` and `check-pr-version-bump` pass for 0.0.67 to 0.0.68. `bun install --frozen-lockfile` passes. The full `check --base origin/main` result is recorded in the PR.
E2E: Passed — debug builds of main and this branch ran `json` on six local Flutter projects; outputs differ only in the version string.

Delivery target: Merge
Delivery: Pending — PR checks, merge, Release workflow publication of v0.0.68 to GitHub and npm.
