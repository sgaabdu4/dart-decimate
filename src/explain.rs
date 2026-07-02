use serde::{Deserialize, Serialize};
pub const EXPLAIN_SCHEMA_VERSION: &str = "dart-decimate.explain.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplainReport {
    pub schema_version: String,
    pub kind: String,
    pub id: String,
    pub name: String,
    pub issue_type: String,
    pub aliases: Vec<String>,
    pub summary: String,
    pub rationale: String,
    pub example: String,
    pub how_to_fix: String,
    pub suppressions: Vec<String>,
    pub related_commands: Vec<String>,
    pub docs: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ExplainError {
    #[error("unknown issue type {issue_type:?}")]
    UnknownIssueType { issue_type: String },
}

/// # Errors
/// Returns [`ExplainError`] when `issue_type` is not supported.
pub fn explain_issue(issue_type: &str) -> Result<ExplainReport, ExplainError> {
    let normalized = normalize_issue_type(issue_type);
    let Some(issue) = ISSUES
        .iter()
        .copied()
        .find(|issue| issue.aliases.iter().any(|alias| *alias == normalized))
    else {
        return Err(ExplainError::UnknownIssueType {
            issue_type: issue_type.to_owned(),
        });
    };

    Ok(ExplainReport {
        schema_version: EXPLAIN_SCHEMA_VERSION.to_owned(),
        kind: "explain".to_owned(),
        id: issue.rule_id.to_owned(),
        name: issue.title.to_owned(),
        issue_type: issue.issue_type.to_owned(),
        aliases: issue.aliases.iter().map(ToString::to_string).collect(),
        summary: issue.summary.to_owned(),
        rationale: issue.rationale.to_owned(),
        example: issue.example.to_owned(),
        how_to_fix: issue.fix_guidance.to_owned(),
        suppressions: issue.suppressions.iter().map(ToString::to_string).collect(),
        related_commands: issue
            .related_commands
            .iter()
            .map(ToString::to_string)
            .collect(),
        docs: issue.docs_url.to_owned(),
    })
}

#[must_use]
pub fn render_explain_report(report: &ExplainReport) -> String {
    format!(
        "{}\n{}\n\n{}\n\nWhy it matters\n{}\n\nExample\n{}\n\nHow to fix\n{}\n\nDocs: {}\n",
        report.name,
        report.id,
        report.summary,
        report.rationale,
        report.example,
        report.how_to_fix,
        report.docs
    )
}

fn normalize_issue_type(issue_type: &str) -> String {
    let trimmed = issue_type.trim().to_ascii_lowercase();
    let stripped = trimmed
        .strip_prefix("dart-decimate/")
        .or_else(|| trimmed.strip_prefix("fallow/"))
        .unwrap_or(&trimmed);
    stripped
        .chars()
        .map(|ch| match ch {
            '_' | ' ' => '-',
            _ => ch,
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct IssueExplanation {
    issue_type: &'static str,
    rule_id: &'static str,
    aliases: &'static [&'static str],
    title: &'static str,
    summary: &'static str,
    rationale: &'static str,
    example: &'static str,
    fix_guidance: &'static str,
    suppressions: &'static [&'static str],
    related_commands: &'static [&'static str],
    docs_url: &'static str,
}

macro_rules! issue {
    (
        $issue_type:expr,
        $rule_id:expr,
        $aliases:expr,
        $title:expr,
        $summary:expr,
        $rationale:expr,
        $example:expr,
        $fix_guidance:expr,
        $suppressions:expr,
        $related_commands:expr $(,)?
    ) => {
        IssueExplanation {
            issue_type: $issue_type,
            rule_id: $rule_id,
            aliases: $aliases,
            title: $title,
            summary: $summary,
            rationale: $rationale,
            example: $example,
            fix_guidance: $fix_guidance,
            suppressions: $suppressions,
            related_commands: $related_commands,
            docs_url: "https://github.com/sgaabdu4/dart-decimate#readme",
        }
    };
}

macro_rules! next_line_suppressions {
    ($rule:literal) => {
        &[concat!("// dart-decimate-ignore-next-line ", $rule)]
    };
}

macro_rules! file_suppressions {
    ($rule:literal) => {
        &[concat!("// dart-decimate-ignore-file ", $rule)]
    };
}

const ISSUES: &[IssueExplanation] = &[
    issue!(
        "unused-file",
        "dart-decimate/dead-file",
        &["unused-file", "unused-files", "dead-file", "dead-files"],
        "Unused file",
        "A Dart file is unreachable from the configured or inferred entry points.",
        "Dart Decimate follows import/export/part/augment edges and reports files outside that graph.",
        "lib/old_screen.dart is not imported by any reachable library.",
        "Trace the file before deletion; generated or externally loaded files should be configured as entry points or ignored.",
        next_line_suppressions!("unused-file"),
        &["dart-decimate trace-file --format json --file <path>"],
    ),
    issue!(
        "unused-export",
        "dart-decimate/unused-export",
        &["unused-export", "unused-exports"],
        "Unused export",
        "A public top-level Dart declaration in reachable code has no reachable syntactic references.",
        "Dart library privacy is based on leading underscores, so public declarations are candidates for API cleanup.",
        "class LegacyApi {} is public but never referenced or re-exported from live code.",
        "Run `dart-decimate trace-symbol` before editing; do not delete public API without a fix preview.",
        next_line_suppressions!("unused-export"),
        &["dart-decimate trace-symbol --format json --symbol <file>:<symbol>"],
    ),
    issue!(
        "unused-type",
        "dart-decimate/unused-type",
        &["unused-type", "unused-types", "type-alias", "type-aliases"],
        "Unused type",
        "A public Dart typedef in reachable code has no reachable syntactic references.",
        "Dart typedef declarations are public API when not private, but stale aliases hide real type ownership and migration cleanup.",
        "typedef LegacyId = String; is declared in live code but no reachable file uses LegacyId.",
        "Run `dart-decimate trace-symbol` before editing; aliases used by external APIs should be exported intentionally or suppressed.",
        next_line_suppressions!("unused-type"),
        &["dart-decimate trace-symbol --format json --symbol <file>:<type-alias>"],
    ),
    issue!(
        "private-type-leak",
        "dart-decimate/private-type-leak",
        &["private-type-leak", "private-type-leaks"],
        "Private type leak",
        "An exported Dart declaration signature references a same-library private type.",
        "Dart identifiers starting with `_` are private to their library, including `part` files, so exposing them through public API signatures makes downstream use impossible or misleading.",
        "class Api extends _Hidden {} is public while _Hidden is a private class in the same Dart library.",
        "Rename the private type, hide the declaration from the public surface, or change the signature to a public type. This rule is opt-in with `--private-type-leaks` or `rules.private-type-leak`.",
        next_line_suppressions!("private-type-leak"),
        &["dart-decimate check --format json --private-type-leaks"],
    ),
    issue!(
        "unused-enum-member",
        "dart-decimate/unused-enum-member",
        &["unused-enum-member", "unused-enum-members"],
        "Unused enum member",
        "A non-public-API enum constant is never referenced from reachable Dart code.",
        "Enum constants are declarations and can become stale after feature or state cleanup.",
        "enum Mode { live, legacy } where legacy is never read.",
        "Review serialization and external contracts before removing the member.",
        next_line_suppressions!("unused-enum-member"),
        &["dart-decimate trace-symbol --format json --symbol <file>:<enum>"],
    ),
    issue!(
        "unused-class-member",
        "dart-decimate/unused-class-member",
        &["unused-class-member", "unused-class-members"],
        "Unused class member",
        "A private class-like field, getter, setter, or method is not referenced in its Dart library.",
        "Dart private names are library-scoped, including `part` files, so unused private members are cleanup candidates.",
        "void _legacyHandler() {} is never called in the library.",
        "Check reflection, generated bindings, and framework callbacks before removal.",
        next_line_suppressions!("unused-class-member"),
        &["dart-decimate trace-symbol --format json --symbol <file>:<class>"],
    ),
    issue!(
        "duplicate-export",
        "dart-decimate/duplicate-export",
        &["duplicate-export", "duplicate-exports"],
        "Duplicate export",
        "A public barrel exports the same symbol name from multiple Dart files.",
        "Importers may receive an ambiguous API surface when barrels expose duplicate declarations.",
        "lib/app.dart exports src/a.dart and src/b.dart, both declaring Api.",
        "Rename, hide, or narrow one export path after reviewing the public API.",
        file_suppressions!("duplicate-export"),
        &["dart-decimate trace-symbol --format json --symbol <file>:<symbol>"],
    ),
    issue!(
        "route-collision",
        "dart-decimate/route-collision",
        &["route-collision"],
        "Route collision",
        "Two GoRouter route declarations resolve to the same path pattern or route name.",
        "GoRouter route trees require unique identities, so duplicate paths or names can make navigation ambiguous or fail generated route tables.",
        "GoRoute(path: '/users/:id') and @TypedGoRoute<MemberRoute>(path: '/users/:userId') describe the same path shape.",
        "Rename one route or change one path segment after checking deep-link compatibility.",
        &["// dart-decimate-ignore-next-line route-collision"],
        &["dart-decimate inspect --format json --file <path>"],
    ),
    issue!(
        "private-widget-class",
        "dart-decimate/private-widget-class",
        &["private-widget-class", "flutter-private-widget-class"],
        "Private widget class",
        "A private Dart class extends a Flutter widget base class.",
        "Extracted widgets should be public classes so widget boundaries stay reusable, testable, and discoverable; private State subclasses remain exempt.",
        "class _Header extends StatelessWidget {}",
        "Rename the widget class to a public name or add an explicit suppression when the private widget is intentional.",
        &["// dart-decimate-ignore-next-line private-widget-class"],
        &["dart-decimate check --format json"],
    ),
    issue!(
        "widget-top-level-function-boundary",
        "dart-decimate/widget-top-level-function-boundary",
        &[
            "widget-top-level-function-boundary",
            "top-level-widget-helper"
        ],
        "Widget top-level function boundary",
        "A top-level Dart function returns Flutter UI from a widget or screen file.",
        "Flutter UI helpers should live in widget classes or explicit owning boundaries so dependencies, tests, and reuse stay discoverable.",
        "Widget _buildHeader(BuildContext context) => const SizedBox();",
        "Extract the helper to a public widget class or move it behind another explicit owner.",
        &["// dart-decimate-ignore-next-line widget-top-level-function-boundary"],
        &["dart-decimate check --format json"],
    ),
    issue!(
        "unused-widget-param",
        "dart-decimate/unused-widget-param",
        &[
            "unused-widget-param",
            "unused-widget-params",
            "unused-component-prop"
        ],
        "Unused widget parameter",
        "A Flutter widget constructor parameter is never read by the widget or paired State class.",
        "Stale widget inputs make call sites harder to trust and mirror Fallow's unused component prop cleanup signal.",
        "const UserCard({required String subtitle}) : _subtitle = subtitle; where _subtitle is never read in UserCard or _UserCardState.",
        "Review callers before removing the parameter; Dart Decimate reports this as a warning by default.",
        &["// dart-decimate-ignore-next-line unused-widget-param"],
        &["dart-decimate check --format json"],
    ),
    issue!(
        "missing-context-mounted-after-await",
        "dart-decimate/missing-context-mounted-after-await",
        &["missing-context-mounted-after-await"],
        "Missing context.mounted guard",
        "A Flutter widget or State method awaits and then uses BuildContext without first checking context.mounted.",
        "Async UI callbacks can resume after the widget is disposed; guarding after await prevents stale-context navigation, dialogs, and state access.",
        "await save(); Navigator.of(context).pop(); without `if (!context.mounted) return;` after the await.",
        "Add `if (!context.mounted) return;` immediately after the await before using context.",
        &["// dart-decimate-ignore-next-line missing-context-mounted-after-await"],
        &["dart-decimate check --format json"],
    ),
    issue!(
        "missing-entry-point",
        "dart-decimate/missing-entry-point",
        &["missing-entry-point", "missing-entry-points"],
        "Missing entry point",
        "A configured entry point does not exist in the parsed module graph.",
        "Reachability is only meaningful when entry files can be resolved.",
        "--entry lib/mian.dart points at a typo instead of lib/main.dart.",
        "Fix the entry path or add the missing Dart file.",
        &[],
        &["dart-decimate list --entry-points --format json"],
    ),
    issue!(
        "circular-dependency",
        "dart-decimate/circular-dependency",
        &[
            "circular-dependency",
            "circular-dependencies",
            "circular-deps"
        ],
        "Circular dependency",
        "Two or more Dart files import or export each other in a dependency cycle.",
        "Cycles make ownership and initialization order harder to reason about.",
        "lib/a.dart imports lib/b.dart while lib/b.dart imports lib/a.dart.",
        "Move shared contracts behind a lower-level module or invert the dependency.",
        next_line_suppressions!("circular-dependency"),
        &["dart-decimate trace-file --format json --file <path>"],
    ),
    issue!(
        "re-export-cycle",
        "dart-decimate/re-export-cycle",
        &["re-export-cycle", "re-export-cycles"],
        "Re-export cycle",
        "Barrel files re-export each other without adding useful ownership direction.",
        "Re-export loops can silently confuse API exposure and duplicate-export analysis.",
        "lib/a.dart exports b.dart and lib/b.dart exports a.dart.",
        "Break the barrel loop and expose symbols from a single public owner.",
        file_suppressions!("re-export-cycle"),
        &["dart-decimate cycles --format json"],
    ),
    issue!(
        "boundary-violation",
        "dart-decimate/boundary-violation",
        &["boundary-violation", "boundary-violations"],
        "Boundary violation",
        "A resolved dependency edge crosses a configured forbidden architecture boundary.",
        "Dart Decimate treats imports and exports as ownership edges between Dart files.",
        "lib/domain/order.dart imports lib/ui/order_card.dart.",
        "Move the dependency behind an allowed boundary or invert the ownership.",
        next_line_suppressions!("boundary-violation"),
        &["dart-decimate trace-file --format json --file <path>"],
    ),
    issue!(
        "boundary-coverage",
        "dart-decimate/boundary-coverage",
        &["boundary-coverage", "boundary-coverages"],
        "Boundary coverage",
        "A Dart library file is outside every configured architecture boundary zone.",
        "Coverage checks catch architecture drift where new files land outside the intended layer or feature map.",
        "lib/data/cache.dart exists while only lib/domain and lib/ui are configured boundary zones.",
        "Move the file into a configured zone or add an intentional boundary rule. Use `dart-decimate list --boundaries` to inspect zones before changing code.",
        &[
            "// dart-decimate-ignore-next-line boundary-violation",
            "// dart-decimate-ignore-file boundary-violation"
        ],
        &[
            "dart-decimate list --boundaries --format json",
            "dart-decimate check --format json --boundary-coverage"
        ],
    ),
    issue!(
        "boundary-call-violation",
        "dart-decimate/boundary-violation",
        &[
            "boundary-call-violation",
            "boundary-call-violations",
            "boundary-violation",
            "boundary-violations"
        ],
        "Boundary call violation",
        "A Dart file inside a configured architecture zone calls a forbidden direct callee pattern.",
        "Boundary call checks catch ownership leaks that happen through platform, framework, or service calls instead of imports alone.",
        "lib/ui/page.dart calls SystemChrome.setPreferredOrientations while lib/ui forbids SystemChrome.*.",
        "Move the call into the owning boundary or expose a smaller allowed abstraction.",
        next_line_suppressions!("boundary-call-violation"),
        &["dart-decimate check --format json --boundary-call lib/ui:SystemChrome.*"],
    ),
    issue!(
        "policy-violation",
        "dart-decimate/policy-violation",
        &["policy-violation", "policy-violations"],
        "Policy violation",
        "A Dart import/export URI or direct call matches a declarative policy rule pack.",
        "Rule packs let teams enforce project-specific ownership rules without embedding project code in Dart Decimate.",
        "A policy pack bans dart:io imports or Process.* calls from app code.",
        "Change the import or call to comply with the pack, or suppress the exact scoped rule after owner review.",
        next_line_suppressions!("policy-violation"),
        &[
            "dart-decimate check --format json --policy-pack policy.jsonc",
            "dart-decimate config-schema --format json"
        ],
    ),
    issue!(
        "unresolved-dependency",
        "dart-decimate/unresolved-dependency",
        &[
            "unresolved-dependency",
            "unresolved-import",
            "unresolved-imports",
            "unresolved-augment"
        ],
        "Unresolved dependency",
        "A local import, export, part, or library augment directive did not resolve to a parsed Dart file.",
        "Broken graph edges hide real reachability and cleanup evidence.",
        "import 'src/missing.dart'; points at no file.",
        "Fix the URI, add the file, or adjust package resolution.",
        next_line_suppressions!("unresolved-import"),
        &["dart-decimate trace-file --format json --file <path>"],
    ),
    issue!(
        "part-of-violation",
        "dart-decimate/part-of-violation",
        &["part-of-violation", "part-of-violations", "invalid-part-of"],
        "Part of violation",
        "A Dart part file and its owning library disagree about their reciprocal part relationship.",
        "Invalid library membership corrupts reachability and private-library symbol analysis.",
        "part 'src/model.g.dart'; resolves, but the target says part of app.other; or has no part of directive.",
        "Update either the library's part directive or the part file's part of directive.",
        next_line_suppressions!("part-of-violation"),
        &["dart-decimate inspect --format json --file <part-file>"],
    ),
    issue!(
        "unused-dependency",
        "dart-decimate/unused-dependency",
        &["unused-dependency", "unused-dependencies", "unused-deps"],
        "Unused pub dependency",
        "A declared runtime pub dependency has no matching Dart import/export usage evidence.",
        "Pubspec dependencies drift as imports are removed or packages move between runtime and test code.",
        "dependencies: path is declared but no Dart file imports package:path/...",
        "Run `dart-decimate trace-dependency` before editing pubspec entries.",
        &[],
        &["dart-decimate trace-dependency --format json --dependency <package>"],
    ),
    issue!(
        "unused-dev-dependency",
        "dart-decimate/unused-dev-dependency",
        &[
            "unused-dev-dependency",
            "unused-dev-dependencies",
            "unused-dev-deps"
        ],
        "Unused dev dependency",
        "A declared Dart dev dependency has no matching Dart import/export usage evidence.",
        "Build, test, and generator dependencies can drift after tooling or test cleanup.",
        "dev_dependencies: mockito is declared but no reachable dev/test Dart file imports package:mockito/...",
        "Run `dart-decimate trace-dependency` and check non-Dart tool usage before editing pubspec.yaml.",
        &[],
        &["dart-decimate trace-dependency --format json --dependency <package>"],
    ),
    issue!(
        "test-only-dependency",
        "dart-decimate/test-only-dependency",
        &[
            "test-only-dependency",
            "test-only-dependencies",
            "test-only-deps"
        ],
        "Test-only dependency",
        "A runtime dependency is imported only from test or development paths.",
        "Dart packages should keep test-only imports in `dev_dependencies` when possible.",
        "dependencies: mocktail is declared but only imported under test/.",
        "Move the package to `dev_dependencies` after confirming no runtime imports exist.",
        &[],
        &["dart-decimate trace-dependency --format json --dependency <package>"],
    ),
    issue!(
        "unused-dependency-override",
        "dart-decimate/unused-dependency-override",
        &[
            "unused-dependency-override",
            "unused-dependency-overrides",
            "dependency-override",
            "dependency-overrides"
        ],
        "Unused dependency override",
        "A Dart dependency override is absent from the resolved pubspec.lock package graph.",
        "Overrides are temporary build policy and should not linger after the resolver stops using them.",
        "dependency_overrides: stale is declared but pubspec.lock does not contain stale.",
        "Compare the override with pubspec.lock and active Pub resolution before removing it.",
        &[],
        &["dart-decimate trace-dependency --format json --dependency <package>"],
    ),
    issue!(
        "misconfigured-dependency-override",
        "dart-decimate/misconfigured-dependency-override",
        &[
            "misconfigured-dependency-override",
            "misconfigured-dependency-overrides"
        ],
        "Misconfigured dependency override",
        "A Dart dependency override key or value cannot be honored by Pub.",
        "Overrides should use a valid package name and a non-empty hosted, path, git, sdk, or version value.",
        "dependency_overrides: Bad-Name or dependency_overrides: stale with no value.",
        "Fix the override key or value before relying on the resolved dependency graph.",
        &[],
        &[],
    ),
    issue!(
        "unlisted-dependency",
        "dart-decimate/unlisted-dependency",
        &[
            "unlisted-dependency",
            "unlisted-dependencies",
            "unlisted-deps"
        ],
        "Unlisted pub dependency",
        "A Dart file imports a package that is not declared in its owning pubspec section.",
        "Package imports should be backed by pubspec dependencies for reproducible builds.",
        "import 'package:http/http.dart'; appears without http in pubspec.yaml.",
        "Add the package to the correct pubspec section or remove the import.",
        &[],
        &["dart-decimate trace-dependency --format json --dependency <package>"],
    ),
    issue!(
        "private-src-import",
        "dart-decimate/private-src-import",
        &["private-src-import", "private-src-imports"],
        "Private src import",
        "A Dart file imports or exports another package's private `lib/src` implementation library.",
        "Dart package `lib/src` directories are implementation details; importing them couples code to APIs the dependency does not publish.",
        "import 'package:shared/src/internal.dart'; reaches into shared's private implementation tree.",
        "Import a public library from the package or ask the dependency owner to expose a supported API.",
        next_line_suppressions!("private-src-import"),
        &["dart-decimate check --format json --private-src-imports"],
    ),
    issue!(
        "code-duplication",
        "dart-decimate/code-duplication",
        &[
            "code-duplication",
            "code-duplications",
            "duplication",
            "dupes"
        ],
        "Code duplication",
        "Two or more Dart code blocks are structurally duplicated.",
        "Duplicated blocks increase maintenance cost and can drift during fixes.",
        "Two widgets contain the same validation branch block.",
        "Trace the clone group and extract shared behavior only when the abstraction has a real owner.",
        next_line_suppressions!("code-duplication"),
        &["dart-decimate trace-clone --format json --fingerprint dup:<id>"],
    ),
    issue!(
        "complexity",
        "dart-decimate/high-complexity",
        &[
            "complexity",
            "high-complexity",
            "high-cyclomatic-complexity",
            "high-cognitive-complexity"
        ],
        "High complexity",
        "A function exceeds configured cyclomatic, cognitive, or combined complexity thresholds.",
        "Branch-heavy Dart functions are harder to test, review, and safely change.",
        "A route builder nests loops and conditionals in one function.",
        "Run health with `--complexity-breakdown`, then split branches into named policy or helper owners.",
        next_line_suppressions!("complexity"),
        &["dart-decimate health --format json --complexity-breakdown"],
    ),
    issue!(
        "coverage-gap",
        "dart-decimate/coverage-gap",
        &[
            "coverage-gap",
            "coverage-gaps",
            "untested-file",
            "untested-files"
        ],
        "Coverage gap",
        "A Dart file has no observed covered executable LCOV lines.",
        "LCOV-backed gaps identify code that is reachable to static analysis but unexercised by tests.",
        "lib/src/parser.dart appears in LCOV with DA lines all at zero hits.",
        "Add or run tests that execute the file, then refresh LCOV.",
        file_suppressions!("coverage-gaps"),
        &["dart-decimate health --format json --coverage-gaps --coverage coverage/lcov.info"],
    ),
    issue!(
        "high-crap-score",
        "dart-decimate/high-crap-score",
        &["high-crap-score", "crap-score", "crap"],
        "High CRAP score",
        "A complex function also has weak line coverage.",
        "CRAP combines complexity and coverage to find functions that are risky to modify.",
        "route() has several branches and 0% LCOV line coverage.",
        "Reduce branching or add targeted tests covering the function.",
        next_line_suppressions!("complexity"),
        &["dart-decimate health --format json --coverage coverage/lcov.info --max-crap 30"],
    ),
    issue!(
        "health-hotspot",
        "dart-decimate/health-hotspot",
        &["health-hotspot", "health-hotspots", "hotspot", "hotspots"],
        "Health hotspot",
        "A Dart file has a low aggregate health score.",
        "File scores combine complexity, CRAP, and coverage status into one prioritization signal.",
        "lib/checkout_flow.dart scores below the configured minimum.",
        "Review score reasons before refactoring; prefer small, owner-preserving changes.",
        &[],
        &["dart-decimate health --format json --file-scores --hotspots"],
    ),
    issue!(
        "refactoring-target",
        "dart-decimate/refactoring-target",
        &[
            "refactoring-target",
            "refactoring-targets",
            "target",
            "targets"
        ],
        "Refactoring target",
        "A low-scoring file is ranked as a high-priority refactoring candidate.",
        "Targets combine health score with complex-function and coverage risk signals.",
        "lib/billing_state.dart has low score and multiple complex functions.",
        "Refactor around the highest-ranked reasons, then rerun health.",
        &[],
        &["dart-decimate health --format json --targets"],
    ),
    issue!(
        "feature-flag",
        "dart-decimate/feature-flag",
        &["feature-flag", "feature-flags", "flags"],
        "Feature flag",
        "A Dart or Flutter feature flag pattern was detected.",
        "Flag inventories help find stale rollout logic and risky config gates.",
        "bool.fromEnvironment('NEW_FLOW') gates production behavior.",
        "Review owner, rollout state, and dead-code traces before deleting flag branches.",
        next_line_suppressions!("feature-flag"),
        &["dart-decimate flags --format json"],
    ),
    issue!(
        "security-candidate",
        "dart-decimate/security-candidate",
        &[
            "security",
            "security-candidate",
            "security-candidates",
            "security-sink",
            "hardcoded-secret",
            "firebase-api-key",
            "insecure-transport",
            "tls-bypass",
            "webview-risk",
            "process-execution",
            "process-exec",
            "raw-sql",
            "plain-secret-storage",
        ],
        "Security candidate",
        "A deterministic local security review candidate was detected.",
        "Dart Decimate surfaces candidates for agent verification; it does not prove exploitability.",
        "HttpClient.badCertificateCallback or an http:// URL appears in reachable code.",
        "Verify source, sink, reachability, and product intent before changing code.",
        next_line_suppressions!("security-sink"),
        &["dart-decimate security --format json --surface"],
    ),
    issue!(
        "stale-suppression",
        "dart-decimate/stale-suppression",
        &[
            "stale-suppression",
            "stale-suppressions",
            "unused-suppression",
            "unused-suppressions"
        ],
        "Stale suppression",
        "A Dart Decimate or Fallow inline suppression no longer suppresses any finding.",
        "Stale suppressions hide historical context and can mask future findings accidentally.",
        "// dart-decimate-ignore-next-line unused-export remains above live code with no finding.",
        "Remove the unused suppression comment.",
        &[],
        &["dart-decimate check --format json"],
    ),
    issue!(
        "missing-suppression-reason",
        "dart-decimate/missing-suppression-reason",
        &["missing-suppression-reason", "missing-suppression-reasons"],
        "Missing suppression reason",
        "A Dart Decimate or Fallow inline suppression is missing required justification text.",
        "Reasoned suppressions make intentional exceptions reviewable and prevent silent cleanup drift.",
        "// dart-decimate-ignore-next-line unused-export omits why the export is intentionally kept.",
        "Add a short reason after `--`, `because`, or `reason:`.",
        &[],
        &["dart-decimate check --format json"],
    ),
];
