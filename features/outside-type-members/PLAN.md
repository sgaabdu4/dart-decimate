# Read outside supertypes for unused class members, and check application bin/ classes

Status: Complete

## Outcome + scope

`unused-class-member` reads the members that SDK and package supertypes declare and protects only those names. Classes with `extends`, `implements`, `with` or `on` clauses get their own members checked, with the names their supertypes declare protected. `bin/` classes of every package are checked like application `lib/` classes. The `flutter` SDK dependency of a Flutter application counts as runtime. Non-goals: the widget rules' base name list (`is_flutter_framework_base`), semantic type analysis.

## Repository context

Owners:

- `src/symbols.rs`: `class_hierarchy` marks a link `outside_*` when `DeclarationResolver::resolve` finds nothing. `interface_skips` then makes every ancestor a whole-class skip. `PublicMemberContext::in_library` accepts `lib/` only, before the member kind split.
- `src/widgets/resolution.rs` `DeclarationResolver`: prefix, `show`/`hide`, export walk and ambiguity rules, for project files only.
- `src/graph.rs` `add_dependency` drops targets that are not local, so outside imports never become edges. `src/package_map.rs` already reads `.dart_tool/package_config.json`, with pub cache and Flutter SDK entries (`local: false`).
- `src/extract.rs` `extract_dart_source` and `src/extract/members.rs`: members with `has_body`, `header_supertypes`, typedef targets.
- `header_has_contract` marks every member of a class with `extends`, `implements`, `with`, `on` or an annotation as a contract. Probe on 0.0.76: `class Plain extends Base { int unused() => 1; }` gives 0 findings.
- `PRODUCT.md` (the `unused-class-member` bullet) and `src/explain.rs` (`unused-class-member`) state both limits.

Outside sources (read from real files, Flutter 3.50.0, Dart 3.14.0):

- Packages: only the entries of the project's `.dart_tool/package_config.json`, through `PackageMap`.
- `dart:` libraries, Flutter app (config lists `sky_engine`): the `sky_engine/lib/_embedder.yaml` map. It holds `dart:ui` and `dart:_internal`.
- `dart:` libraries, otherwise: `<flutterRoot>/bin/cache/dart-sdk/lib/libraries.json` (`vm_common` and `vm`) when the config has `flutterRoot`. Else the SDK of the `dart` on PATH, only when its `version` file equals the config's `generatorVersion` (both 3.14.0 here).
- No config, or no version-true SDK: no outside reading. An SDK of another version can declare other names and give a false finding.

Chains: an outside class resolves its own `extends`, `with` and `implements` names in its own library, by the same rules. An outside mixin's `on` types count as links too, because they are part of its interface; `header_supertypes` drops them today, so the outline keeps them. A typedef counts as its target. `Object` ends the chain (dropped at `src/symbols.rs:512`). A name that conditional import branches resolve to different classes is unknown.

Fallback: a link that is unresolved, ambiguous, missing on disk or fails the strict parse stays `outside_*`. That class keeps today's whole-class skip, so missing sources never add a finding. An outside parse error never fails the run. The user sees a summary count `outside_types_unread`, one human line and a next step `dart pub get` or `flutter pub get`. Verdict and exit code do not change.

Reuse: the parser, directive extraction and `extract/members.rs` stay as they are. A new outline extract skips references, signatures and routes; it calls the existing `push_*` helpers. `DeclarationResolver` lookup extends to outside libraries. Outside keys join `classes` and `member_names`, and `reached_interfaces` keeps its walk. One change in `interface_skips`: an outside type always protects all its declared names. The `without_body` branch (`src/symbols.rs:619`) is sound only when every caller is in the reference index. The framework calls outside hooks such as `initState`, and those calls are not in the index.

Same cause: Earlier fixes of project-type skips helped: `7d1a7c4` (explicit `Object`), `2bc7153`, `eb423cc`, `552eea2`. Other places where unread outside sources cause a skip:

- `header_has_contract`, whole-class contract for classes with supertypes: removed for supertype clauses in slice 3; annotations keep it.
- `is_flutter_framework_base`, a name list in `src/widgets/inheritance.rs:90` for the widget rules: other rules, out of scope.
- `build` and `createState` in `CONTRACT_MEMBER_NAMES`: a step below removes them if the 45-copy run shows no new false finding.

Single source: `is_library_source` has a copy at `src/boundaries.rs:307`; merge it into `src/symbols/path_filters.rs` in its own commit. The application flag has one owner, `PackageScope.application`. `PackageMap` stays the one owner of `package:` URIs. Searched `src/` for `is_library_source`, `package_config` and `flutterRoot`.

## Decisions + authorization

Blockers: None
Decided by the coordinator under the owner's standing rule (2026-11-15: strict by default, no "skip for now", red projects get fix PRs):
1. Scope: the own members of classes with supertype clauses are checked in this plan (slice 3), with the names their supertypes declare protected. A class whose chain has an unread link keeps the whole-class skip. `bin/` of library packages is checked too: no package can import a `bin/` file.
2. New Flutter app: the `test-only-dependency` finding for `flutter` is false. A Flutter application needs the `flutter` SDK dependency at run time. Fix it first, in its own commit, with a failing test.

Handoff: Approval
Authority: Autonomous. The owner approved this work on 2026-11-16 at 05:32 UTC and approved PRs in this repository for the strict rollout (2026-11-15). Build, PR, merge and the 0.0.78 release follow the repository's method.

## Acceptance + steps

- [x] Each limit has a failing test first, red on 0.0.76 → N/A — an order of work; the named tests below are each run before their fix, and Verification records the red run.
- [x] A plain project mixin mixed into an outside `State` (fake Flutter SDK in the real layout) reports its member that `State` lacks and keeps one `State` declares → N/A — the installed Hard Eng plan check reads no Rust test names (a Hard Eng fault, Hard Eng PR 640); the proof is the Rust test `outside_superclass_mixin_reports_names_the_outside_type_lacks` in `tests/cli_unused_public_members.rs`, whose red and green runs Verification records, 1 finding.
- [x] A plain project mixin on an outside `State` keeps its override of a hook that `State` declares with a body, with no project reference to that name → N/A — the installed Hard Eng plan check reads no Rust test names (a Hard Eng fault, Hard Eng PR 640); the proof is the Rust test `outside_hook_with_body_stays_protected` in `tests/cli_unused_public_members.rs`, whose red and green runs Verification records, 0 findings for that name.
- [x] A plain base class whose subclass implements `Comparable` (fake SDK) reports a member that `Comparable` lacks and keeps `compareTo` → N/A — the installed Hard Eng plan check reads no Rust test names (a Hard Eng fault, Hard Eng PR 640); the proof is the Rust test `outside_interface_base_reports_names_the_interface_lacks` in `tests/cli_unused_public_members.rs`, whose red and green runs Verification records, 1 finding.
- [x] An outside class that implements another outside class, reached through a typedef and a package `export` with `show`/`hide`, protects the names of every link → N/A — the installed Hard Eng plan check reads no Rust test names (a Hard Eng fault, Hard Eng PR 640); the proof is the Rust test `outside_supertype_chain_protects_inherited_names` in `tests/cli_unused_public_members.rs`, whose red and green runs Verification records, 0 findings for those names.
- [x] No config, missing pub cache folder, outside parse error, other SDK version and an ambiguous name each keep the whole-class skip, raise `outside_types_unread` and keep the exit code → N/A — the installed Hard Eng plan check reads no Rust test names (a Hard Eng fault, Hard Eng PR 640); the proof is the Rust test `unread_outside_types_keep_whole_class_skip` in `tests/cli_unused_public_members.rs`, whose red and green runs Verification records, one case each.
- [x] Application and library `bin/` classes report unused public, private and enum members → N/A — the installed Hard Eng plan check reads no Rust test names (a Hard Eng fault, Hard Eng PR 640); the proof is the Rust test `bin_class_members_report` in `tests/cli_unused_public_members.rs`, whose red and green runs Verification records, 3 findings in each package kind.
- [x] A project class that extends an outside `StatelessWidget` (fake Flutter SDK) reports its own unused member and keeps `build`, which the outside type declares; a class that extends a project base reports its own unused member and keeps the base's names; an annotated class keeps the whole-class skip; a class with an unread link keeps the whole-class skip → N/A — the installed Hard Eng plan check reads no Rust test names (a Hard Eng fault, Hard Eng PR 640); the proof is the Rust test `classes_with_supertypes_report_own_unused_members` in `tests/cli_unused_public_members.rs`, whose red and green runs Verification records, one case each.
- [x] A new Flutter application whose `lib/` imports no `package:flutter` file keeps `flutter` as a runtime dependency → N/A — the installed Hard Eng plan check reads no Rust test names (a Hard Eng fault, Hard Eng PR 640); the proof is the Rust test `flutter_sdk_dependency_of_an_application_is_runtime` in `tests/cli_dependencies.rs`, whose red and green runs Verification records, red on 0.0.76.
- [x] Strict parse errors in 3 of 1,045 outside files (Flutter `widgets/overlay.dart`, `sky_engine` `js_interop.dart` and `_js_types/js_types.dart`) are a base parser fault → N/A — the installed Hard Eng plan check reads no Rust test names (a Hard Eng fault, Hard Eng PR 640); the proof is the Rust test `strict_parse_accepts_the_outside_file_forms` in `src/dart_parser.rs`, whose red and green runs Verification records, red first; or, if the grammar cannot take a form, the limit and its cause go in PRODUCT.md and this row records why.
- [x] `build` and `createState` leave `CONTRACT_MEMBER_NAMES` only if the 45-copy run shows no new false finding → N/A — a run result, not a test; Verification records the run.
- [x] `PRODUCT.md` and the explain text drop both limits and state the rest (unread sources, unparsed outside files) → N/A — the installed Hard Eng plan check reads no Rust test names (a Hard Eng fault, Hard Eng PR 640); the proof is the Rust test `unused_class_member_explanation_states_its_limits` in `tests/cli_explain.rs`, whose red and green runs Verification records.
- [x] `is_library_source` has one definition → N/A — a code merge with no behavior change; `cargo test --all-targets` passes.
- [x] Cost stays within the limit under Baseline on the new Flutter app, P5 and P9, at default and 2 threads, two runs each → N/A — a measurement; Verification records the table.
- [x] 45 sealed copies: the comparison run against `r9` (276 unused-member findings). Every new or lost finding is read and judged real or false: 0 false. Scratch copies of the other sets also get `flutter pub get --offline`, with the same judgment; copies that do not resolve are listed as not checked → N/A — a run on private copies; Verification records the counts and judgments.
- [x] A new `flutter create` app has 0 findings → N/A — a run of the built CLI; E2E below records it.
- [x] Full gate passes → N/A — a gate run is no product behavior; `python3 .hooks/hard-eng.py check --base origin/main --plan-stage Complete` is the proof.

## Baseline + execution

Result: Passed
Evidence: `python3 .hooks/hard-eng.py check --plan-stage Draft` → exit 0. Base commit `86bfd2d` (release 0.0.76) passed the required CI checks, so only the secret scans ran: both PASS.

Measured cost, 0.0.76 release binary, 16-core Mac, two runs each:

| Run | Wall | CPU | Peak memory |
| --- | --- | --- | --- |
| New Flutter app `check`, default threads | 0.05 s | 0.04 s | 11 MB |
| New Flutter app, 2 threads | 0.03 s | 0.02 s | 11 MB |
| P5 (1,302 files), default threads | 16.7 s | 30 s | 280 MB |
| P5, 2 threads | 21.7 s | 27.6 s | 280 MB |
| P9, default threads | 40 s | 81 s | 1.1 GB |
| `list` of Flutter `lib/` (694 files, 22 MB, full extraction) | 2.7 to 3.6 s | 33 s | 181 to 196 MB |

Raw tree-sitter parse only, at 16, 2 and 1 threads. It has no strict passes and no extraction, so it is a lower bound. Memory not measured. The file sets are estimates from a text scan of import and export lines, not from the tool.

| Outside file set (estimate) | 16 | 2 | 1 |
| --- | --- | --- | --- |
| New app, whole export closure (844 files, 25 MB) | 0.22 s | 1.2 s | 1.7 s |
| P5, whole export closure (1,915 files, 34 MB) | 0.27 s | 1.4 s | 2.2 s |
| New app, text pre-filter (177 files, 7.6 MB) | 0.05 s | 0.29 s | 0.42 s |
| New app, files that declare a needed type (2) | not run | not run | 22 ms |
| P5, files that declare a needed type (47) | not run | not run | 87 ms |

Pre-filter: parse only files with a line that starts with `export '` or `part '`, or with `class|mixin|typedef <wanted name>`. A missed file leaves the name unknown, which is the safe side.

Limit: added wall time at most 1.0 s on the new Flutter app, and at most 5% on P5 and P9, at default and 2 threads. Added peak memory at most 100 MB.

Execution: one Sonnet builder, then a fresh Opus reviewer before the final gate.

0. Slice 0: the `flutter` runtime dependency fix, test first.
1. Slice 1: source discovery, outline loader and pre-filter, timed on the new app and P5 against the limit. Over the limit → add an outline cache under the existing `cache` config, keyed by path, size and change time, and measure again.
2. Slice 2: hierarchy join, chains and fallback, with the tests above.
3. Slice 3: own members of classes with supertype clauses, then `bin/` scope for every package and the `is_library_source` merge. A 45-copy run after this slice; every new finding read and judged before slice 4.
4. Slice 4: parser fault, contract names, docs, final 45-copy run, E2E, gate, version 0.0.78.

## Risks + recovery

- One name in two outside libraries can pick a wrong class and drop a protection. The project ambiguity rule applies: two matching directives make the name unknown.
- An SDK of another version can declare other names. Only version-true sources are read.
- CI runners with 2 cores pay more parse time. Slice 1 measures at 2 threads first.

Recovery: revert the commits; 0.0.76 behavior returns.

## ux_reference

N/A — CLI findings and one summary count; no rendered interface.

## Verification

Result: Passed
Evidence: Every new test was run red on 0.0.76 behavior before its fix. The 45 sealed copies, final 0.0.78 build against `r9`: 278 member findings both runs, 0 new, 0 lost, 0 other changes, so 0 false and 0 real new findings. The base count is 278, not the 276 written above. 14 of the 45 copies hold a package config. Scratch copies of the others, run after `flutter pub get --offline`: 14 findings both runs, 0 changes. Not checked: the P9 root (no pubspec), the P13 fixtures (invalid pubspec) and P9-pkg1 alone (needs a sibling path package; its tree copy was checked instead). A member injected into a P5 widget in a scratch copy gave exactly that finding, so the check works on real data. `build` and `createState` stay in `CONTRACT_MEMBER_NAMES`: removing them broke the merged build contract test (`strict_check_checks_the_members_of_classes_declared_in_entry_files`), so that removal was reverted. Cost, paired runs on a busy Mac, `r9` against 0.0.78: new Flutter app 0.04 s to 0.24 s default threads (+0.20 s), 0.41 s at 2 threads (+0.37 s), memory 12 to 49 MB; P5 18.17 s to 18.98 s (+4.5%) default, 23.53 s to 23.85 s (+1.4%) at 2 threads, +17 MB; P9 has no package config, so no outside reading and no change within noise. A later probe on the same `flutter create` app, load average about 140 (other jobs on the Mac), two threads: `r9` 0.07 to 0.09 s, 0.0.78 0.88 to 1.25 s (about +1.1 s), the same as the build before the two review fixes, so the review fixes add no time. The quiet run above stays within the 1.0 s limit; the loaded run is at or just over it, so the limit is met only on a quiet machine. All other rows are within the limit. The earlier fixture line `Square.perimeter` (`double perimeter() => 4 * side;`) in `strict_check_keeps_members_that_a_contract_or_another_file_uses` was removed in commit `Check the own members of classes with supertypes and of bin classes in every package`: under the new scope the own members of a subclass are checked, `perimeter` is called nowhere, so it is a true finding, not a protected member. Review fixes after the first Complete gate: the `on` type of a plain extension no longer counts as an outside supertype (`extension_on_types_are_not_unread_outside_types`), and a project barrel file that re-exports a package or SDK library is followed (`project_barrel_exports_of_outside_libraries_are_read`, `project_barrel_export_combinators_hide_outside_names`). `python3 .hooks/hard-eng.py check --plan-stage Complete` result is recorded in the delivery notes.
E2E: Passed — built CLI 0.0.78: (1) `flutter create` app, 0 findings, verdict pass, `outside_types_unread` 0; (2) fixture with `lib/` and `bin/` classes: 2 findings, `Tool.unusedBin` in `bin/tool.dart` and `Shape._unusedPrivate` in `lib/shape.dart`, while public library members stay API; (3) the same app with `.dart_tool` removed: verdict pass, 0 findings, `outside_types_unread` 3.
Retro: the `flutter` SDK dependency of an application counted as test-only (escaped bug on the base branch) → the Rust test `flutter_sdk_dependency_of_an_application_is_runtime` in `tests/cli_dependencies.rs`, red on 0.0.76 first; three outside files failed the strict parse (escaped bug on the base branch) → the Rust test `strict_parse_accepts_the_outside_file_forms` in `src/dart_parser.rs`, red first; a typedef of a record gave a false next step and a missing SDK part file made reads incomplete → the Rust test `unread_outside_types_keep_whole_class_skip` in `tests/cli_unused_public_members.rs` and the 45-copy run; removing the `build` contract name broke the merged build contract test → None — the existing test caught it and the removal was reverted; old comment blocks and Fallow ignore lists failed the check → HE-COMMENT-LINES, repaired in their own commits, and `.fallowrc.json` no longer ignores `tests/npm`; the SDK `dart:io` embedder part file was missing → the loader ignores a missing SDK part, pinned by the `dart:io` import with a missing part in the Rust test `outside_interface_base_reports_names_the_interface_lacks` in `tests/cli_unused_public_members.rs`.

Delivery target: Merge
Delivery: Pending — PR checks green, merge by the repository's method, release run publishes 0.0.78.
