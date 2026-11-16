# Read outside supertypes for unused class members, and check application bin/ classes

Status: Draft

## Outcome + scope

`unused-class-member` reads the members that SDK and package supertypes declare and protects only those names, and checks application `bin/` classes like `lib/` classes. Non-goals: the own members of classes with supertype clauses (Blocker 1), the widget rules, semantic type analysis.

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

Chains: an outside class resolves its own `extends`, `with` and `implements` names in its own library, by the same rules. A typedef counts as its target. `Object` ends the chain (dropped at `src/symbols.rs:512`). A name that conditional import branches resolve to different classes is unknown.

Fallback: a link that is unresolved, ambiguous, missing on disk or fails the strict parse stays `outside_*`. That class keeps today's whole-class skip, so missing sources never add a finding. An outside parse error never fails the run. The user sees a summary count `outside_types_unread`, one human line and a next step `dart pub get` or `flutter pub get`. Verdict and exit code do not change.

Reuse: the parser, directive extraction and `extract/members.rs` stay as they are. A new outline extract skips references, signatures and routes; it calls the existing `push_*` helpers. `DeclarationResolver` lookup extends to outside libraries. Outside keys join `classes` and `member_names`, so `reached_interfaces` and `interface_skips` keep their logic.

Same cause: Earlier fixes of project-type skips helped: `7d1a7c4` (explicit `Object`), `2bc7153`, `eb423cc`, `552eea2`. Other places where unread outside sources cause a skip:

- `header_has_contract`, whole-class contract for classes with supertypes: Blocker 1.
- `is_flutter_framework_base`, a name list in `src/widgets/inheritance.rs:90` for the widget rules: Blocker 1.
- `build` and `createState` in `CONTRACT_MEMBER_NAMES`: a step below removes them if the 45-copy run shows no new false finding.

Single source: `is_library_source` has a copy at `src/boundaries.rs:307`; merge it into `src/symbols/path_filters.rs` in its own commit. The application flag has one owner, `PackageScope.application`. `PackageMap` stays the one owner of `package:` URIs. Searched `src/` for `is_library_source`, `package_config` and `flutterRoot`.

## Decisions + authorization

Blockers: Two owner choices, each with a recommendation.

1. Scope (owner). The narrow change still leaves `class X extends StatelessWidget { void unused() {} }` unchecked, and every class with a supertype clause or annotation. Options:
   - (a) Recommended: the two stated limits only, now. It matches the request and needs one 45-copy judgment.
   - (b) Also check the own members of classes with supertypes, protecting names their supertypes declare, and replace the widget base name list. This is the biggest gain but adds findings in almost every widget class. Recommended as the next plan, reusing this outside reader.
   - (c) `bin/` of library packages too. Recommended yes: no package can import a `bin/` file, so nothing exports its classes. The request names application `bin/` only.
2. New Flutter app at 0 findings (owner). 0.0.76 on a new `flutter create` app (Flutter 3.50.0) gives 1 finding: `test-only-dependency` for `flutter`. The template's `lib/main.dart` imports only `package:material_ui`. With `flutter` moved to `dev_dependencies`, `pub get`, `analyze`, `test` and `build bundle` all pass. Options:
   - (a) Recommended: treat it as false. The `flutter` SDK dependency of a Flutter application counts as runtime, because every new app fails `--strict` today. Fix it first, in its own commit, with a failing test.
   - (b) Treat it as real. The acceptance row becomes "0 `unused-class-member` findings".

Handoff: Approval
Authority: Autonomous after approval. The owner approved this work on 2026-11-16 at 05:32 UTC. This step is plan only: no production code, push or PR. Build waits for answers to Blockers 1 and 2.

## Acceptance + steps

- [ ] Both limits have a failing test first, red on 0.0.76 → the two rows below, run before the fix.
- [ ] A plain project mixin mixed into an outside `State` (fake Flutter SDK in the real layout) reports its member that `State` lacks and keeps one `State` declares → `tests/cli_unused_public_members.rs::outside_superclass_mixin_reports_names_the_outside_type_lacks`, 1 finding.
- [ ] A plain base class whose subclass implements `Comparable` (fake SDK) reports a member that `Comparable` lacks and keeps `compareTo` → `tests/cli_unused_public_members.rs::outside_interface_base_reports_names_the_interface_lacks`, 1 finding.
- [ ] An outside class that implements another outside class, reached through a typedef and a package `export` with `show`/`hide`, protects the names of every link → `tests/cli_unused_public_members.rs::outside_supertype_chain_protects_inherited_names`, 0 findings for those names.
- [ ] No config, missing pub cache folder, outside parse error, other SDK version and an ambiguous name each keep the whole-class skip, raise `outside_types_unread` and keep the exit code → `tests/cli_unused_public_members.rs::unread_outside_types_keep_whole_class_skip`, one case each.
- [ ] Application `bin/` classes report unused public, private and enum members; library `bin/` follows Blocker 1(c) → `tests/cli_unused_public_members.rs::application_bin_class_members_report`, 3 findings.
- [ ] Strict parse errors in 3 of 1,045 outside files (Flutter `widgets/overlay.dart`, `sky_engine` `js_interop.dart` and `_js_types/js_types.dart`) are a base parser fault → own commit with a failing `src/dart_parser.rs` test, or the limit is stated in the docs with the cause.
- [ ] `build` and `createState` leave `CONTRACT_MEMBER_NAMES` only if the 45-copy run shows no new false finding → run evidence recorded.
- [ ] `PRODUCT.md` and the explain text drop both limits and state the rest (unread sources, unparsed outside files) → `cargo test --test cli_explain` passes.
- [ ] `is_library_source` has one definition → `cargo test --all-targets` passes.
- [ ] Cost stays within the limit under Baseline on the new Flutter app, P5 and P9, at default and 2 threads, two runs each → recorded table.
- [ ] 45 sealed copies: the comparison run against `r9` (276 unused-member findings). Every new or lost finding is read and judged real or false: 0 false. Scratch copies of the other sets also get `flutter pub get --offline`, with the same judgment; copies that do not resolve are listed as not checked.
- [ ] A new `flutter create` app has 0 findings (per Blocker 2) → E2E below.
- [ ] Full gate → `python3 .hooks/hard-eng.py check --base origin/main --plan-stage Complete` passes.

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

Raw tree-sitter parse only, at 16, 2 and 1 threads. It has no strict passes and no extraction, so it is a lower bound. Memory not measured.

| Outside file set | 16 | 2 | 1 |
| --- | --- | --- | --- |
| New app, whole export closure (844 files, 25 MB) | 0.22 s | 1.2 s | 1.7 s |
| P5, whole export closure (1,915 files, 34 MB) | 0.27 s | 1.4 s | 2.2 s |
| New app, text pre-filter (177 files, 7.6 MB) | 0.05 s | 0.29 s | 0.42 s |
| Files that declare a needed type: new app 2, P5 47 | 22 ms (1) | | 87 ms (1) |

Pre-filter: parse only files with a line that starts with `export '` or `part '`, or with `class|mixin|typedef <wanted name>`. A missed file leaves the name unknown, which is the safe side.

Limit: added wall time at most 1.0 s on the new Flutter app, and at most 5% on P5 and P9, at default and 2 threads. Added peak memory at most 100 MB.

Execution: one Sonnet builder, then a fresh Opus reviewer before the final gate.

1. Slice 1: source discovery, outline loader and pre-filter, timed on the new app and P5 against the limit. Over the limit → add an outline cache under the existing `cache` config, keyed by path, size and change time, and measure again.
2. Slice 2: hierarchy join, chains and fallback, with the tests above.
3. Slice 3: `bin/` scope and the `is_library_source` merge.
4. Slice 4: parser fault, contract names, docs, 45-copy run, E2E, gate, version 0.0.77.

## Risks + recovery

- One name in two outside libraries can pick a wrong class and drop a protection. The project ambiguity rule applies: two matching directives make the name unknown.
- An SDK of another version can declare other names. Only version-true sources are read.
- CI runners with 2 cores pay more parse time. Slice 1 measures at 2 threads first.

Recovery: revert the commits; 0.0.76 behavior returns.

## ux_reference

N/A — CLI findings and one summary count; no rendered interface.

## Verification

Result: Pending
Evidence: Pending
E2E: Required — built CLI on (1) a new `flutter create` app → 0 findings; (2) a fixture with `lib/` and `bin/` classes → the listed findings; (3) the same app with `.dart_tool` removed → same verdict, `outside_types_unread` above 0.
Retro: Pending

Delivery target: Merge
Delivery: Pending — PR checks green, merge by the repository's method, release run publishes 0.0.77.
