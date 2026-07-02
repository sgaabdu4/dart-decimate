# Dart Decimate Product Contract

Dart Decimate is a Rust-native codebase intelligence tool for Dart and Flutter.
It analyzes repositories as module graphs, not as a linter or type checker.

## Principles

- Treat Dart code as graph data: files, declarations, imports, exports, parts,
  and library augmentations.
- Do not evaluate variables, types, or inner function logic.
- Keep outputs deterministic and agent-readable.
- Prefer fast structural parsing over semantic analysis.

## AI Drift Goal

Dart Decimate exists for the same product problem Fallow targets: AI can create code
quickly, but teams still need deterministic evidence to review it, clean it up,
and keep architecture from drifting. For Dart and Flutter, that means:

- finding dead files, unused symbols, duplicate implementation patterns, and
  stale suppressions before they become permanent;
- showing dependency, re-export, boundary, and route-graph risks with paths and
  line numbers;
- ranking hotspots and PR risk so reviewers spend attention where change is most
  likely to matter;
- producing structured JSON, traces, schemas, and MCP tools that agents can use
  before proposing safe cleanup.

Dart Decimate must stay objective and graph/intelligence-focused. It must not enforce
team-specific Flutter style preferences such as generated Riverpod ownership.

## Phase Scope

Phase 1 extracts syntax facts from one `.dart` file:

- `import` directives
- `export` directives
- import/export `show` and `hide` combinators
- import prefixes and deferred import markers
- `part` directives
- `library augment` directives
- all branch URIs from conditional imports and exports, including
  `dart.library.*` branch guards
- top-level class, mixin, extension, and extension type declarations
- top-level enum declarations
- top-level typedef declarations
- top-level variable declarations
- top-level function-like declarations
- class-like member declarations for fields, constructors, getters, setters,
  methods, operators, and enum constants
- syntactic identifier and type-identifier references, excluding directive
  metadata and obvious declaration-name positions

Phase 2 builds a directed module graph:

- nodes are Dart files
- edges are resolved `import`, `export`, `part`, and `library augment`
  dependencies
- dependency edges preserve import prefixes, deferred markers, and `show`/`hide`
  combinators for later symbol visibility propagation
- `part` and `library augment` directives are modeled as file dependencies
- conditional import/export branches are modeled as possible file dependencies
  by default, and `--dart-platform vm|web` can select the target-specific
  compile graph
- relative URIs resolve from the importing file's directory
- `package:` URIs prefer Pub's `.dart_tool/package_config.json` when present,
  including `rootUri` and `packageUri`
- `package:` URIs resolve to local pub workspace/path packages when present
- scan roots expand through local path dependencies and pub workspace members

Phase 3 runs graph intelligence algorithms:

- dead code reachability from caller-supplied entry points
- first conservative unused-export analysis for reachable public top-level
  declarations with no reachable syntactic references
- conservative unused-member analysis for non-public-api enum constants and
  private class-like fields, getters, setters, and methods
- public library `export` chains count as public API reachability and respect
  `show`/`hide` combinators
- circular dependency detection via strongly connected components
- export-only cycle detection for barrel/re-export loops
- architecture boundary rules over graph edges
- structural Dart health analysis for cyclomatic and cognitive function
  complexity
- feature flag inventory for Dart compile-time environment reads, native
  process-environment gates, Firebase Remote Config calls, and LaunchDarkly-style
  variation calls
- unverified security candidate inventory for hardcoded secrets, Firebase client
  API keys, cleartext transport, TLS bypasses, risky WebView surfaces, process
  execution, dynamic SQL, and plain secret storage

Phase 4 exposes the CLI and agent output contract:

- bare `dart-decimate` as a default `dart-decimate check` over the current directory
- `dart-decimate check`
- `dart-decimate audit`
- `dart-decimate review`
- `dart-decimate decision-surface`
- `dart-decimate dead-code`
- `dart-decimate cycles`
- `dart-decimate dupes`
- `dart-decimate health`
- `dart-decimate flags`
- `dart-decimate security`
- `dart-decimate trace`
- `dart-decimate trace-file`
- `dart-decimate trace-symbol`
- `dart-decimate trace-dependency`
- `dart-decimate trace-clone`
- `dart-decimate inspect`
- `dart-decimate list`
- `dart-decimate workspaces`
- `dart-decimate explain`
- `dart-decimate fix`
- `dart-decimate init`
- `dart-decimate hooks`
- `dart-decimate impact`
- `dart-decimate ci-template`
- `dart-decimate schema`
- `dart-decimate config`
- `dart-decimate config-schema`
- `dart-decimate report-schema`
- `dart-decimate human`, `dart-decimate json`, and `dart-decimate html` are
  shortcuts for `dart-decimate check`; `html` opens a browser report by default
  unless `--stdout` is passed
- analysis commands with `--format json` emit `dart-decimate.report.v1`
- report commands with `--format html` emit a browser-ready static HTML report
- finding HTML reports group visible findings by issue type in numbered,
  collapsible sections with search and type filters; search never opens groups
  automatically, and a type filter opens only the selected group
- report commands with `--open` write an HTML report to a private temporary file
  and open its `file://` URL in the default browser
- human reports strip terminal control characters from user-derived paths and
  messages; HTML reports escape user-derived content
- trace commands with `--format json` emit `dart-decimate.trace.v1`
- `dart-decimate inspect --format json` emits `dart-decimate.inspect.v1`
- `dart-decimate schema --format json` emits `dart-decimate.schema.v1` with commands,
  issue types, schema versions, and an agent task matrix
- `dart-decimate decision-surface --format json --base REF` emits
  `dart-decimate.decision-surface.v1` as an advisory, always-zero review envelope
- `dart-decimate review` and `dart-decimate audit --brief` emit the same advisory
  decision-surface envelope and always exit 0
- `dart-decimate impact --format json --quiet` emits `dart-decimate.impact.v1` as a
  read-only local value report, enabling `.dart-decimate/impact.jsonl` history when
  present
- `dart-decimate init` emits `dart-decimate.init.v1`, writes `.dart-decimaterc`, optionally
  writes `AGENTS.md` with `--agents`, and refuses to overwrite existing files
  unless `--force` is passed
- `dart-decimate hooks status|install|uninstall` emits `dart-decimate.hooks.v1`, manages
  only Dart Decimate-marked Git pre-commit hooks by default, and refuses unmanaged
  hook overwrite/removal unless `--force` is passed
- `dart-decimate ci-template github|gitlab` emits `dart-decimate.ci-template.v1` JSON or
  YAML CI templates; `gitlab --vendor` writes scoped local template files only
  with explicit overwrite via `--force`
- `dart-decimate report-schema --format json` emits the JSON schema for
  analysis reports under `dart-decimate.report.v1`
- process-level command, config, and runtime failures with `--format json` emit
  `{"error":true,"message":"...","exit_code":2}` on stdout instead of plain
  stderr so agents can parse failure states
- `--config PATH` loads `.dart-decimaterc`, `.dart-decimaterc.json`,
  `.dart-decimaterc.jsonc`, `dart-decimate.toml`, or `.dart-decimate.toml` defaults
- `--save-baseline PATH` writes current visible findings to
  `dart-decimate.baseline.v1`
- `--baseline PATH` suppresses findings already captured in a saved baseline
- `--save-regression-baseline PATH` writes current visible finding counts to
  `dart-decimate.regression-baseline.v1`
- `--regression-baseline PATH`, `--fail-on-regression`, and `--tolerance`
  provide count-based regression gates
- `dart-decimate audit` accepts per-analysis baseline inputs with
  `--dead-code-baseline`, `--health-baseline`, and `--dupes-baseline`
- `dart-decimate audit` summary includes `risk_score`, `risk_level`, and
  introduced/pre-existing attribution; `--gate new-only` fails only on
  introduced error findings while keeping related pre-existing findings visible
- JSON findings include paths, line/column locations, `safe_to_delete`, and `actions`
- JSON reports include read-only `next_steps` for trace-before-delete and
  grouped security-surface review workflows
- `dart-decimate audit --base REF` runs full graph analysis and reports only findings
  anchored to files changed since the Git ref or their related files
- findings respect `// dart-decimate-ignore-next-line ...` inline suppressions
- simple one-line unused top-level declarations can emit safe
  `remove-declaration` fix actions; multi-line declarations remain trace-only

## Fallow Parity Target

Dart Decimate must become Fallow-equivalent for Dart and Flutter, adapted to Dart
semantics from the official Dart docs:

- Dart libraries are the privacy boundary; identifiers beginning with `_` are
  private to the library.
- `import`, `export`, `part`, and `library augment` directives define the
  module graph.
- `show`, `hide`, prefixes, deferred imports, conditional imports/exports,
  target platform selection, and `part` files affect symbol visibility and
  reachability.
- `pubspec.yaml` owns package dependencies, `dev_dependencies`, and
  `dependency_overrides`.
- Pub workspaces can share dependency resolution across multiple packages.

Parity areas:

- Dead code: unused files, exports, types, enum members, class members, stale
  suppressions, private type leak checks, traces, and safe fix actions.
- Dependency hygiene: unused dependencies, unlisted dependencies, dev/test-only
  placement, dependency overrides, and workspace placement.
- Graph issues: unresolved imports/exports/parts/augmentations, invalid Dart
  `part of` relationships, circular dependencies, re-export cycles, duplicate
  exports, and architecture boundary violations.
- Flutter framework checks: typed and raw GoRouter route path and name
  collisions, private widget classes, top-level widget helper boundaries,
  unused widget constructor parameters, and unrendered widget classes.
- Duplication: strict, mild, weak, and semantic clone detection with traceable
  fingerprints, top-N filtering, and clone tracing.
- Health: cyclomatic/cognitive/CRAP complexity, file scores, hotspots,
  ownership, refactoring targets, coverage gaps, and explanations.
- Audit: changed-code gates, baselines, regression tolerance, CI review
  envelopes, and next-step suggestions.
- Feature flags: env gates, SDK/config flag calls, stale-flag evidence,
  rollout owner review, and top-N flag inventory.
- Fix: dry-run and confirmed apply flows for safe unused exports/dependencies.
- Config: suppressions, workspaces, public packages, cache settings, and
  broader rule-pack controls.
- Onboarding: local init flows for config, CI, and agent instructions.
- Security candidates: deterministic local review candidates for Dart/Flutter
  sinks, Firebase client keys, and hardcoded secrets, never verified
  vulnerability claims.
- Runtime coverage: local Dart/Flutter coverage ingestion for hot paths,
  cleanup confidence, coverage gaps, and read-only MCP runtime slices;
  cloud/runtime agent capture remains future work.
- Integrations: schema output, CI templates, an MCP stdio server with guarded
  `fix_preview` / `fix_apply`, editor-ready JSON contracts, impact reporting,
  and read-only MCP impact reports.

Current implemented parity:

- file-level dead code reachability
- circular dependency detection
- simple directory boundary rules, built-in `layered`, `hexagonal`,
  `feature-sliced`, and `bulletproof` boundary presets, opt-in boundary
  coverage checks for unzoned Dart library files with `allowUnmatched`
  exceptions, forbidden direct boundary-call checks, and
  `dart-decimate list --boundaries` boundary inventory
- declarative policy rule packs for banned Dart import/export URI patterns and
  direct call patterns, plus `dart-decimate rule-pack-schema`
- stale suppression detection and opt-in missing suppression reason checks
- unresolved local dependency findings
- `dart-decimate/part-of-violation` findings for resolved `part` files whose
  `part of` directive is missing, orphaned, or points at a different library
- Dart `part` files, `library augment` directives with base-to-augment
  reachability, and platform-aware conditional import/export branches in the
  module graph
- Pub `.dart_tool/package_config.json` resolution for local package graph edges
- import/export visibility metadata for `show`, `hide`, prefixes, and deferred imports
- import/export visibility metadata preserved on graph dependency edges
- top-level symbol extraction for classes, mixins, extensions, extension types,
  enums, typedefs, variables, and function-like declarations
- class-like member extraction for fields, constructors, getters, setters,
  methods, operators, and enum constants
- conservative `dart-decimate/unused-enum-member` and
  `dart-decimate/unused-class-member` findings for reachable Dart libraries,
  including same-library `part` reference handling, suppressions, rule config,
  baselines, and JSON schema coverage
- syntactic reference extraction for identifiers and type identifiers
- a `SymbolIndex` owner plus conservative `dart-decimate/unused-export` and
  `dart-decimate/unused-type` findings for reachable non-entry, non-generated,
  public top-level declarations and typedefs
- Fallow-style `--include-entry-exports` for `check`, `audit`, and `dead-code`,
  plus `includeEntryExports` config, to opt into unused public declarations in
  entry libraries while keeping Dart `main` protected
- public export-chain protection for package barrel APIs, including `show`/`hide`
- opt-in `dart-decimate/private-type-leak` API hygiene for exported signatures that
  expose same-library private Dart types, including `part` libraries,
  `show`/`hide`, suppressions, rule config, JSON schema, and explain coverage
- `dart-decimate/duplicate-export` findings for public barrel APIs that expose the
  same top-level symbol from multiple files
- `dart-decimate/route-collision` findings for typed GoRouter routes and raw
  `GoRoute` route trees that resolve to the same path pattern or route name,
  with parameter-name normalization and nested route path joining
- `dart-decimate/unrendered-widget` findings for Flutter widget classes with no
  reachable production object construction, ignoring generated/test/dead files
  and explicit package export chains
- Flutter widget hygiene findings for private widget classes, top-level widget
  helper boundaries, and unused widget constructor parameters
- separate `dart-decimate/re-export-cycle` findings for barrel export loops
- read-only file, symbol, dependency, clone, and Fallow-compatible `trace`
  symbol-trace JSON envelopes for deletion review, using `kind` discriminators
  and `dart-decimate.trace.v1`
- `dart-decimate inspect` evidence bundles for one file or top-level symbol,
  combining trace output with a file-scoped `check` report
- `dart-decimate schema` machine-readable command and issue manifest for agents
- `dart-decimate decision-surface` advisory changed-code questions for coupling,
  public API contracts, and Pub dependency ownership
- Fallow-compatible `dart-decimate review` and `dart-decimate audit --brief` aliases for
  advisory decision-surface output that always exits 0
- `dart-decimate impact --format json --quiet` read-only local value report,
  including disabled zero-count output, `.dart-decimate/impact.jsonl` local-history
  aggregation, and `--all` rollup shape
- `dart-decimate ci-template` read-only GitHub Actions and GitLab CI template output,
  plus explicit GitLab vendoring with overwrite protection
- `dart-decimate init --agents` project onboarding with `.dart-decimaterc`, optional
  `AGENTS.md`, overwrite protection, and `dart-decimate.init.v1` JSON output
- `dart-decimate hooks install --target git` safe Git pre-commit hook management
  using Dart Decimate ownership markers and `dart-decimate.hooks.v1` JSON output
- `dart-decimate list` project metadata JSON for files, entry points, local pub
  packages, and active Dart/Flutter/workspace adapters; `dart-decimate workspaces`
  emits the same schema scoped to local pub packages
- `dart-decimate explain` read-only issue explanations with Fallow-compatible
  aliases such as `unused exports`, `fallow/unused-export`, `complexity`, and
  `code duplication`
- `dart-decimate fix` safe-fix planning with dry-run JSON by default and explicit
  `--apply --confirm` mutation for auto-fixable stale suppressions and dead
  Dart files, simple unused pub dependencies, and simple one-line unused
  top-level Dart declarations
- finding `actions` include Fallow-style `type`, `auto_fixable`, optional
  read-only `command` plus argv, target metadata, config hints, and suppression
  comments
- pub dependency hygiene for runtime unused packages and unlisted packages
- pub dependency placement hygiene for unused `dev_dependencies`, runtime
  dependencies imported only from dev/test files, and `dev_dependencies`
  imported from `lib/` or `bin/`
- typed dependency finding kinds and summary counts for
  `dart-decimate/unused-dev-dependency`, `dart-decimate/test-only-dependency`, and
  `dart-decimate/unused-dependency-override`,
  `dart-decimate/misconfigured-dependency-override`
- conservative non-Dart tooling usage for dependency traces and unused-dev
  checks from Dart/Flutter config files such as `build.yaml`,
  `analysis_options.yaml`, workflow YAML, and tool scripts
- lockfile-backed `dependency_overrides` hygiene for overrides absent from the
  resolved `pubspec.lock` package graph
- misconfigured `dependency_overrides` hygiene for invalid package keys and
  empty override values
- Fallow-style `ignoreDependencyOverrides` config entries for intentional
  `dependency_overrides` by package and optional source file
- Fallow-style `ignoreDependencies` config entries for intentional pub
  dependency hygiene exceptions
- unused dependency reports include a read-only `trace-dependency` next step
- code duplication findings with stable `dup:<id>` fingerprints and read-only
  `trace-clone` next steps
- code health findings for high cyclomatic and cognitive complexity, including
  `--complexity-breakdown`, `--max-cyclomatic`, `--max-cognitive`, `--top`, and
  read-only `complexity-breakdown` next steps
- LCOV-backed health findings for `dart-decimate/coverage-gap` and
  `dart-decimate/high-crap-score`, including `--coverage`, `--coverage-gaps`,
  `--max-crap`, config defaults, rule controls, baselines, regression counts,
  and JSON schema coverage
- Fallow-style `health.thresholdOverrides` for local cyclomatic, cognitive, and
  CRAP ceilings, with `threshold_overrides`, `effective_thresholds`, and
  `threshold_source` JSON output for agent review
- local `--runtime-coverage` ingestion for Istanbul `coverage-final.json`,
  V8 JSON files, and V8 JSON directories, with Fallow-style thresholds
  `--min-invocations-hot`, `--min-observation-volume`, and
  `--low-traffic-threshold`, plus a `runtime_coverage` JSON block containing
  `verdict`, `signals`, `summary`, `findings`, `hot_paths`,
  `coverage_intelligence`, `blast_radius`, `importance`, `actionable`,
  `provenance`, `watermark`, and `warnings`
- runtime coverage intelligence includes stable IDs, hot-path review rows,
  low-traffic cleanup rows, coverage-unavailable rows, direct graph caller blast
  radius, and traffic-weighted importance scores for agent prioritization
- focused `dart-decimate coverage analyze --runtime-coverage <path> --format json`
  output using the `dart-decimate.coverage.v1` runtime coverage envelope
- file health scoring with `--file-scores`, low-score
  `dart-decimate/health-hotspot` findings via `--hotspots`, and prioritized
  `dart-decimate/refactoring-target` findings via `--targets`, including
  `--min-score`, config defaults, rule controls, baselines, regression counts,
  and JSON schema coverage
- CODEOWNERS-backed `dart-decimate health --ownership` metadata for file scores,
  hotspots, and refactoring targets, including GitLab-style section names and
  config-driven `health.ownership`
- `dart-decimate flags` inventory for `bool.fromEnvironment`,
  `String.fromEnvironment`, `int.fromEnvironment`, `Platform.environment`,
  Firebase Remote Config `get*` calls, and LaunchDarkly-style `*Variation` calls,
  with `--top`, grouped `feature_flags`, occurrence locations, and
  non-autofixable `dart-decimate/feature-flag` findings
- `dart-decimate security` inventory for hardcoded secret-shaped literals,
  `FirebaseOptions.apiKey` client keys, remote `http://` network sinks,
  certificate-validation bypasses, unrestricted or file-backed WebView surfaces,
  shell/dynamic process execution, dynamic raw SQL, JavaScript password autofill
  literals, and secret-like writes to plain local storage, with `--top`,
  `--surface`, `--format sarif`, `--sarif-file`, `--ci`, `--fail-on-issues`,
  `--summary`, `--gate new|newly-reachable`, `--changed-since REF`,
  `--compare REF`, `--diff-file PATCH`, `--diff-stdin`, grouped
  `security_candidates`, config-level `security.categories` filtering, redacted
  evidence, benign password-route/copy filtering, and non-autofixable
  `dart-decimate/security-*` findings; security gates exit `8` when new
  review-required candidates are present
- `dart-decimate check` and `dart-decimate audit` include feature flag and security
  candidate findings in the same report envelope, with focused commands still
  available for targeted inventories
- inline `dart-decimate-ignore-next-line` suppressions for agent findings,
  including `security-sink`, `hardcoded-secret`, and `firebase-api-key` security
  aliases
- `dart-decimate/stale-suppression` findings for unused Dart Decimate inline
  suppressions, with a safe remove action
- `dart-decimate audit --base REF` changed-file gating for the existing check stack,
  including untracked new files, while keeping full-graph context before output
  scoping
- audit report scoping expands changed files by one resolved
  import/export/part/augment graph hop so directly related findings stay visible
- Fallow-style `--file PATH` report scoping for exact Dart files, keeping
  full-graph analysis while filtering JSON findings, fix previews, list
  metadata, and scoped detail arrays
- Fallow-style `--changed-since REF` report scoping and `--compare REF` alias
  that derive changed Dart files from Git, include untracked files, hard-error
  on invalid refs with similar-ref suggestions, and keep full-graph analysis
  while filtering JSON findings and scoped detail arrays
- Fallow-style `--production` reachability mode for `check`, `audit`,
  `dead-code`, `trace-file`, `trace-symbol`, `list`, `workspaces`, and `fix`,
  using only production Dart entry heuristics while keeping full graph parsing;
  production dead-file findings are intentionally not auto-fixable;
  `.dart-decimaterc` supports `production = true` and `[cli].production`, with
  `--no-production` as an explicit CLI override
- Fallow-style `--workspace` report scoping for local pub package names,
  package-root globs, comma lists, and `!` excludes, keeping full-graph analysis
  while filtering JSON findings and scoped detail arrays; `list --workspace`
  and `workspaces --workspace` filter file and package metadata
- Fallow-style `--changed-workspaces REF` report scoping that derives changed
  local pub packages from Git, hard-errors on invalid refs, is mutually
  exclusive with `--workspace`, and keeps full-graph analysis while filtering
  reports to changed package files and pubspecs
- `.dart-decimaterc`, JSON/JSONC, and TOML config discovery with strict unknown-key
  rejection, root-relative explicit `--config`, `dart-decimate config`,
  `dart-decimate config-schema`, `dart-decimate report-schema`, config-driven `format`,
  `entry`, `boundary`, Fallow-style `[boundaries]` preset/rule/coverage
  objects, `ignore_patterns`, and analyzer defaults for health, dupes, flags,
  security, plus strict `cache.enabled` / `cache.path` metadata
- config `rules` support for Fallow-style `"error"`, `"warn"`, and `"off"`
  severities, including Fallow aliases like `unused-files`, `unused-exports`,
  `unresolved-imports`, `unused-deps`, `unlisted-deps`, `circular-deps`, and
  `complexity`, plus Flutter aliases like `unused-component` and
  `unused-widget-class`
- identity baselines for `check`, `dead-code`, `cycles`, `dupes`, `health`,
  `flags`, and `security`, plus audit baseline loading for dead-code, health,
  and duplicate-code findings
- count-based regression baselines for `check`, `dead-code`, `cycles`,
  `dupes`, `health`, `flags`, and `security`, with absolute and percentage
  tolerance parsing
- Dart entry heuristics for public `lib/*.dart`, direct `bin/`, `test/`,
  `integration_test/`, `test_driver/`, `tool/`, and `pigeon/` scripts
- CLI JSON output for `check`, `audit`, `dead-code`, `cycles`, `dupes`,
  `health`, `flags`, `security`, `list`, `workspaces`, `explain`, `fix`,
  `config`, `config-schema`, and `report-schema`
- concise human report details plus browser-ready HTML report output with
  issue-type grouping, search, and type filters for report commands via
  `--format html`, `--open`, and `dart-decimate human|json|html`
  shortcuts; human output strips terminal control characters and HTML output
  escapes user-derived content
- SARIF 2.1.0 output for report commands via `--format sarif`, suitable for
  code-scanning upload after the same suppression, rule, baseline, and `--top`
  filtering used by JSON reports; security `--sarif-file PATH` writes SARIF
  while keeping stdout on the selected report format; security gates with
  `--changed-since`, `--compare`, `--diff-file`, or `--diff-stdin` narrow both
  JSON and SARIF to added-line candidates for `--gate new` or reachable changed
  candidates for `--gate newly-reachable`; security `--ci` emits SARIF and
  fails on candidates, while `--summary` keeps counts without item arrays for
  human, HTML, and JSON output
