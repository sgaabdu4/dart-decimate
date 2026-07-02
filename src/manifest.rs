use serde_json::{Value, json};

use crate::ci_template::{CI_RECONCILE_REVIEW_SCHEMA_VERSION, CI_TEMPLATE_SCHEMA_VERSION};
use crate::config::CONFIG_SCHEMA_VERSION;
use crate::coverage::COVERAGE_ANALYSIS_SCHEMA_VERSION;
use crate::decision_surface::DECISION_SURFACE_SCHEMA_VERSION;
use crate::explain::EXPLAIN_SCHEMA_VERSION;
use crate::fix::FIX_SCHEMA_VERSION;
use crate::hooks::HOOKS_SCHEMA_VERSION;
use crate::impact::IMPACT_SCHEMA_VERSION;
use crate::init::INIT_SCHEMA_VERSION;
use crate::inspect::INSPECT_SCHEMA_VERSION;
use crate::output::{SCHEMA_VERSION, TRACE_SCHEMA_VERSION};
use crate::policy::RULE_PACK_SCHEMA_VERSION;
use crate::project_list::PROJECT_LIST_SCHEMA_VERSION;
use crate::unsupported::UNSUPPORTED_SCHEMA_VERSION;

mod mcp;
use mcp::mcp_tools;

/// Stable schema version for the agent capability manifest.
pub const MANIFEST_SCHEMA_VERSION: &str = "dart-decimate.schema.v1";

/// Return Dart Decimate's machine-readable CLI and issue manifest.
#[must_use]
pub fn dart_decimate_schema() -> Value {
    json!({
        "schema_version": MANIFEST_SCHEMA_VERSION,
        "kind": "schema",
        "tool": "dart-decimate",
        "name": "Dart Decimate",
        "description": "Rust-native Dart and Flutter module-graph intelligence.",
        "manifest_version": MANIFEST_SCHEMA_VERSION,
        "version": env!("CARGO_PKG_VERSION"),
        "default_command": "check",
        "default_behavior": "Bare dart-decimate invocations default to dart-decimate check against the provided root.",
        "global_flags": ["--root", "--format", "--config", "--quiet"],
        "output_formats": ["human", "html", "json", "sarif"],
        "plugins": plugins(),
        "environment_variables": environment_variables(),
        "mcp_tools": mcp_tools(),
        "exit_codes": [
            { "code": 0, "meaning": "success or no error-severity findings" },
            { "code": 1, "meaning": "error-severity findings or skipped apply fixes" },
            { "code": 2, "meaning": "runtime or configuration error" },
            { "code": 3, "meaning": "config discovery miss for config --path" }
        ],
        "severity_levels": ["error", "warning"],
        "suppression_comments": {
            "next_line": "// dart-decimate-ignore-next-line <issue-type>",
            "file": "// dart-decimate-ignore-file <issue-type>"
        },
        "schemas": {
            "report": SCHEMA_VERSION,
            "trace": TRACE_SCHEMA_VERSION,
            "inspect": INSPECT_SCHEMA_VERSION,
            "list": PROJECT_LIST_SCHEMA_VERSION,
            "fix": FIX_SCHEMA_VERSION,
            "explain": EXPLAIN_SCHEMA_VERSION,
            "impact": IMPACT_SCHEMA_VERSION,
            "init": INIT_SCHEMA_VERSION,
            "hooks": HOOKS_SCHEMA_VERSION,
            "ci_reconcile_review": CI_RECONCILE_REVIEW_SCHEMA_VERSION,
            "ci_template": CI_TEMPLATE_SCHEMA_VERSION,
            "config": CONFIG_SCHEMA_VERSION,
            "coverage": COVERAGE_ANALYSIS_SCHEMA_VERSION,
            "decision_surface": DECISION_SURFACE_SCHEMA_VERSION,
            "unsupported": UNSUPPORTED_SCHEMA_VERSION,
            "rule_pack": RULE_PACK_SCHEMA_VERSION
        },
        "commands": commands(),
        "issue_types": issue_types(),
        "task_matrix": task_matrix()
    })
}

fn plugins() -> Value {
    json!([
        {
            "name": "dart",
            "kind": "language",
            "description": "Parses Dart imports, exports, parts, augmentations, declarations, and references."
        },
        {
            "name": "flutter",
            "kind": "framework",
            "description": "Adds Flutter widget, lifecycle, route, and security candidate checks."
        },
        {
            "name": "pub-workspace",
            "kind": "workspace",
            "description": "Resolves pubspec packages, path dependencies, package_config.json, and Dart workspaces."
        }
    ])
}

fn environment_variables() -> Value {
    json!([
        {
            "name": "DART_DECIMATE_BASE",
            "scope": "generated-git-hook",
            "description": "Overrides the base ref used by Dart Decimate-managed Git pre-commit hooks."
        }
    ])
}

fn commands() -> Value {
    let mut commands = Vec::new();
    append_commands(&mut commands, analysis_commands());
    append_commands(&mut commands, coverage_commands());
    append_commands(&mut commands, evidence_commands());
    append_commands(&mut commands, support_commands());
    Value::Array(commands)
}

fn append_commands(commands: &mut Vec<Value>, values: Value) {
    if let Value::Array(values) = values {
        commands.extend(values);
    }
}

fn analysis_commands() -> Value {
    json!([
        {
            "name": "human",
            "kind": "shortcut",
            "description": "Shortcut for check with readable terminal output.",
            "schema": SCHEMA_VERSION,
            "flags": ["ROOT"]
        },
        {
            "name": "json",
            "kind": "shortcut",
            "description": "Shortcut for check with JSON output.",
            "schema": SCHEMA_VERSION,
            "flags": ["ROOT"]
        },
        {
            "name": "html",
            "kind": "shortcut",
            "description": "Shortcut for check with a browser HTML report.",
            "schema": SCHEMA_VERSION,
            "flags": ["ROOT", "--stdout"]
        },
        {
            "name": "check",
            "kind": "combined",
            "description": "Run all enabled graph, symbol, dependency, duplicate, health, flag, and security checks.",
            "schema": SCHEMA_VERSION,
            "flags": ["--root", "--format", "--open", "--config", "--entry", "--dart-platform", "--production", "--no-production", "--file", "--workspace", "--changed-workspaces", "--changed-since", "--compare", "--regression-baseline", "--save-regression-baseline", "--fail-on-regression", "--tolerance", "--baseline", "--save-baseline", "--boundary", "--boundary-coverage", "--boundary-call", "--policy-pack", "--policy-violations", "--max-cyclomatic", "--max-cognitive", "--complexity-breakdown", "--coverage", "--coverage-gaps", "--max-crap", "--runtime-coverage", "--min-invocations-hot", "--min-observation-volume", "--low-traffic-threshold", "--file-scores", "--hotspots", "--targets", "--ownership", "--min-score", "--mode", "--min-tokens", "--min-lines", "--min-occurrences", "--top", "--threshold", "--cross-language", "--skip-local", "--ignore-imports", "--no-ignore-imports", "--include-entry-exports", "--private-type-leaks", "--unused-files", "--unused-exports", "--unused-types", "--unused-deps", "--unlisted-deps", "--private-src-imports", "--duplicate-exports", "--circular-deps", "--re-export-cycles", "--boundary-violations", "--unused-enum-members", "--unused-class-members", "--unresolved-imports", "--stale-suppressions", "--unused-dependency-overrides", "--misconfigured-dependency-overrides"]
        },
        {
            "name": "audit",
            "kind": "audit",
            "description": "Run changed-code graph checks scoped from a Git base ref.",
            "schema": SCHEMA_VERSION,
            "flags": ["--root", "--brief", "--base", "--gate", "--dead-code-baseline", "--health-baseline", "--dupes-baseline", "--max-decisions", "--format", "--open", "--config", "--entry", "--dart-platform", "--production", "--no-production", "--file", "--workspace", "--changed-workspaces", "--changed-since", "--compare", "--boundary", "--boundary-coverage", "--boundary-call", "--policy-pack", "--policy-violations", "--max-cyclomatic", "--max-cognitive", "--complexity-breakdown", "--coverage", "--coverage-gaps", "--max-crap", "--runtime-coverage", "--min-invocations-hot", "--min-observation-volume", "--low-traffic-threshold", "--file-scores", "--hotspots", "--targets", "--ownership", "--min-score", "--mode", "--min-tokens", "--min-lines", "--min-occurrences", "--top", "--threshold", "--cross-language", "--skip-local", "--ignore-imports", "--no-ignore-imports", "--include-entry-exports", "--private-type-leaks"]
        },
        {
            "name": "review",
            "kind": "decision-surface",
            "description": "Review changed-code structural decisions without failing CI.",
            "schema": DECISION_SURFACE_SCHEMA_VERSION,
            "flags": ["--root", "--base", "--format", "--config", "--max-decisions"]
        },
        {
            "name": "decision-surface",
            "kind": "decision-surface",
            "description": "Surface changed-code structural decisions for reviewer judgment.",
            "schema": DECISION_SURFACE_SCHEMA_VERSION,
            "flags": ["--root", "--base", "--format", "--config", "--max-decisions"]
        },
        {
            "name": "dead-code",
            "kind": "dead-code",
            "description": "Find unreachable Dart files and conservative symbol-level dead code.",
            "schema": SCHEMA_VERSION,
            "flags": ["--root", "--format", "--open", "--config", "--entry", "--dart-platform", "--production", "--no-production", "--file", "--workspace", "--changed-workspaces", "--changed-since", "--compare", "--regression-baseline", "--save-regression-baseline", "--fail-on-regression", "--tolerance", "--baseline", "--save-baseline", "--include-entry-exports", "--private-type-leaks", "--unused-files", "--unused-exports", "--unused-types", "--unused-deps", "--unlisted-deps", "--private-src-imports", "--duplicate-exports", "--unused-enum-members", "--unused-class-members", "--unresolved-imports", "--stale-suppressions", "--unused-dependency-overrides", "--misconfigured-dependency-overrides"]
        },
        {
            "name": "cycles",
            "kind": "cycles",
            "description": "Find import/export/part/augment dependency cycles.",
            "schema": SCHEMA_VERSION,
            "flags": ["--root", "--format", "--open", "--config", "--entry", "--dart-platform", "--production", "--no-production", "--file", "--workspace", "--changed-workspaces", "--changed-since", "--compare", "--regression-baseline", "--save-regression-baseline", "--fail-on-regression", "--tolerance", "--baseline", "--save-baseline"]
        },
        {
            "name": "dupes",
            "kind": "dupes",
            "description": "Find duplicated Dart code blocks.",
            "schema": SCHEMA_VERSION,
            "flags": ["--root", "--format", "--open", "--config", "--entry", "--dart-platform", "--production", "--no-production", "--file", "--workspace", "--changed-workspaces", "--changed-since", "--compare", "--regression-baseline", "--save-regression-baseline", "--fail-on-regression", "--tolerance", "--baseline", "--save-baseline", "--mode", "--min-tokens", "--min-lines", "--min-occurrences", "--top", "--threshold", "--cross-language", "--skip-local", "--ignore-imports", "--no-ignore-imports"]
        },
        {
            "name": "health",
            "kind": "health",
            "description": "Find complex functions, coverage gaps, hotspots, and refactoring targets.",
            "schema": SCHEMA_VERSION,
            "flags": ["--root", "--format", "--open", "--config", "--entry", "--dart-platform", "--production", "--no-production", "--file", "--workspace", "--changed-workspaces", "--changed-since", "--compare", "--regression-baseline", "--save-regression-baseline", "--fail-on-regression", "--tolerance", "--baseline", "--save-baseline", "--max-cyclomatic", "--max-cognitive", "--complexity-breakdown", "--coverage", "--coverage-gaps", "--max-crap", "--runtime-coverage", "--min-invocations-hot", "--min-observation-volume", "--low-traffic-threshold", "--file-scores", "--hotspots", "--targets", "--ownership", "--min-score", "--top"]
        },
        {
            "name": "flags",
            "kind": "flags",
            "description": "Inventory Dart and Flutter feature flag patterns.",
            "schema": SCHEMA_VERSION,
            "flags": ["--root", "--format", "--open", "--config", "--entry", "--dart-platform", "--production", "--no-production", "--file", "--workspace", "--changed-workspaces", "--changed-since", "--compare", "--regression-baseline", "--save-regression-baseline", "--fail-on-regression", "--tolerance", "--baseline", "--save-baseline", "--top"]
        },
        {
            "name": "security",
            "kind": "security",
            "description": "Surface local deterministic security review candidates.",
            "schema": SCHEMA_VERSION,
            "flags": ["--root", "--format", "--open", "--config", "--entry", "--dart-platform", "--production", "--no-production", "--file", "--workspace", "--changed-workspaces", "--regression-baseline", "--save-regression-baseline", "--fail-on-regression", "--tolerance", "--baseline", "--save-baseline", "--top", "--surface", "--sarif-file", "--ci", "--fail-on-issues", "--summary", "--gate", "--diff-file", "--diff-stdin", "--changed-since", "--compare"]
        },
        {
            "name": "impact",
            "kind": "impact",
            "description": "Read the local Dart Decimate value report without running analysis.",
            "schema": IMPACT_SCHEMA_VERSION,
            "flags": ["--root", "--format", "--quiet", "--all", "--sort", "--limit"]
        }
    ])
}

fn coverage_commands() -> Value {
    json!([
        {
            "name": "coverage setup",
            "kind": "coverage-setup",
            "description": "Plan or write local Dart/Flutter runtime coverage defaults.",
            "schema": COVERAGE_ANALYSIS_SCHEMA_VERSION,
            "flags": ["--root", "--format", "--config", "--yes", "--non-interactive"]
        },
        {
            "name": "coverage analyze",
            "kind": "runtime-coverage",
            "description": "Analyze local V8 or Istanbul runtime coverage.",
            "schema": COVERAGE_ANALYSIS_SCHEMA_VERSION,
            "flags": ["--root", "--format", "--config", "--runtime-coverage", "--cloud", "--repo", "--min-invocations-hot", "--min-observation-volume", "--low-traffic-threshold", "--top"]
        },
        {
            "name": "coverage upload-inventory",
            "kind": "coverage-upload-inventory",
            "description": "Build a local Dart source inventory upload dry-run packet.",
            "schema": COVERAGE_ANALYSIS_SCHEMA_VERSION,
            "flags": ["--root", "--format", "--config", "--repo", "--dry-run"]
        },
        {
            "name": "coverage upload-source-maps",
            "kind": "coverage-upload-source-maps",
            "description": "Build a source-map upload dry-run packet.",
            "schema": COVERAGE_ANALYSIS_SCHEMA_VERSION,
            "flags": ["--root", "--format", "--config", "--dir", "--git-sha", "--repo", "--strip-path", "--dry-run"]
        }
    ])
}

fn evidence_commands() -> Value {
    json!([
        {
            "name": "inspect",
            "kind": "inspect",
            "description": "Compose one evidence bundle for a Dart file or top-level symbol.",
            "schema": INSPECT_SCHEMA_VERSION,
            "flags": ["--root", "--format", "--config", "--entry", "--dart-platform", "--production", "--no-production", "--file", "--symbol"]
        },
        {
            "name": "trace-file",
            "kind": "trace-file",
            "description": "Trace one Dart file's graph evidence.",
            "schema": TRACE_SCHEMA_VERSION,
            "flags": ["--root", "--format", "--config", "--entry", "--dart-platform", "--production", "--no-production", "--file"]
        },
        {
            "name": "trace",
            "kind": "trace-symbol",
            "description": "Trace one top-level symbol's declaration, references, and re-export chains.",
            "schema": TRACE_SCHEMA_VERSION,
            "flags": ["--root", "--format", "--config", "--entry", "--dart-platform", "--production", "--no-production"]
        },
        {
            "name": "trace-symbol",
            "kind": "trace-symbol",
            "description": "Trace one top-level symbol's declaration, references, and re-export chains.",
            "schema": TRACE_SCHEMA_VERSION,
            "flags": ["--root", "--format", "--config", "--entry", "--dart-platform", "--production", "--no-production", "--file", "--symbol"]
        },
        {
            "name": "trace-dependency",
            "kind": "trace-dependency",
            "description": "Trace one pub dependency declaration and Dart import/export usage.",
            "schema": TRACE_SCHEMA_VERSION,
            "flags": ["--root", "--format", "--config", "--entry", "--dart-platform", "--production", "--no-production", "--dependency"]
        },
        {
            "name": "trace-clone",
            "kind": "trace-clone",
            "description": "Trace one duplicate-code group by fingerprint or file line.",
            "schema": TRACE_SCHEMA_VERSION,
            "flags": ["--root", "--format", "--config", "--entry", "--dart-platform", "--production", "--no-production", "--mode", "--min-tokens", "--min-lines", "--min-occurrences", "--top", "--threshold", "--cross-language", "--skip-local", "--ignore-imports", "--no-ignore-imports", "--fingerprint"]
        }
    ])
}

fn support_commands() -> Value {
    let mut commands = Vec::new();
    commands.extend(project_support_commands());
    commands.extend(local_support_commands());
    commands.extend(integration_support_commands());
    Value::Array(commands)
}

fn project_support_commands() -> Vec<Value> {
    vec![
        json!({
            "name": "list",
            "kind": "list",
            "description": "List Dart Decimate project structure and active Dart/Flutter adapters.",
            "schema": PROJECT_LIST_SCHEMA_VERSION,
            "flags": ["--root", "--format", "--config", "--entry", "--dart-platform", "--production", "--no-production", "--files", "--entry-points", "--workspaces", "--plugins", "--boundaries", "--file", "--workspace", "--changed-workspaces"]
        }),
        json!({
            "name": "workspaces",
            "kind": "list",
            "description": "List discovered local pub packages.",
            "schema": PROJECT_LIST_SCHEMA_VERSION,
            "flags": ["--root", "--format", "--config", "--entry", "--dart-platform", "--production", "--no-production", "--file", "--workspace", "--changed-workspaces"]
        }),
        json!({
            "name": "explain",
            "kind": "explain",
            "description": "Explain one Dart Decimate issue type without running analysis.",
            "schema": EXPLAIN_SCHEMA_VERSION,
            "flags": ["--format"]
        }),
        json!({
            "name": "fix",
            "kind": "fix",
            "description": "Plan or apply safe auto-fixes.",
            "schema": FIX_SCHEMA_VERSION,
            "flags": ["--root", "--format", "--config", "--entry", "--dart-platform", "--production", "--no-production", "--file", "--workspace", "--changed-workspaces", "--changed-since", "--compare", "--action", "--dry-run", "--apply", "--yes", "--confirm"]
        }),
    ]
}

fn local_support_commands() -> Vec<Value> {
    vec![
        json!({
            "name": "migrate",
            "kind": "unsupported",
            "description": "Report Dart support status for Fallow migration helpers.",
            "schema": UNSUPPORTED_SCHEMA_VERSION,
            "flags": ["--format", "--dry-run"]
        }),
        json!({
            "name": "telemetry",
            "kind": "unsupported",
            "description": "Report Dart Decimate telemetry support status.",
            "schema": UNSUPPORTED_SCHEMA_VERSION,
            "flags": ["status", "enable", "disable", "--format"]
        }),
        json!({
            "name": "license",
            "kind": "unsupported",
            "description": "Report Dart Decimate license support status.",
            "schema": UNSUPPORTED_SCHEMA_VERSION,
            "flags": ["status", "activate", "--format"]
        }),
        json!({
            "name": "init",
            "kind": "init",
            "description": "Create Dart Decimate config and optional AGENTS.md guidance.",
            "schema": INIT_SCHEMA_VERSION,
            "flags": ["--root", "--format", "--agents", "--force"]
        }),
        json!({
            "name": "hooks",
            "kind": "hooks",
            "description": "Inspect, install, or remove Dart Decimate-managed Git and agent hooks.",
            "schema": HOOKS_SCHEMA_VERSION,
            "flags": ["status", "install", "uninstall", "--root", "--format", "--target", "--branch", "--force"]
        }),
        json!({
            "name": "setup-hooks",
            "kind": "hooks",
            "description": "Install or remove Dart Decimate-managed repo-local agent hooks.",
            "schema": HOOKS_SCHEMA_VERSION,
            "flags": ["--root", "--format", "--agent", "--branch", "--dry-run", "--force", "--uninstall"]
        }),
        json!({
            "name": "watch",
            "kind": "watch",
            "description": "Watch Dart project files and rerun Dart Decimate check.",
            "schema": SCHEMA_VERSION,
            "flags": ["--root", "--format", "--no-clear", "--once", "--interval-ms"]
        }),
        json!({
            "name": "ci",
            "kind": "ci",
            "description": "Run CI integration utilities.",
            "schema": CI_RECONCILE_REVIEW_SCHEMA_VERSION,
            "flags": ["reconcile-review", "--provider", "--repo", "--project-id", "--pr", "--mr", "--api-url", "--envelope", "--dry-run", "--format"]
        }),
    ]
}

fn integration_support_commands() -> Vec<Value> {
    vec![
        json!({
            "name": "ci-template",
            "kind": "ci-template",
            "description": "Print or vendor GitHub Actions and GitLab CI templates.",
            "schema": CI_TEMPLATE_SCHEMA_VERSION,
            "flags": ["--format", "--vendor", "--root", "--force"]
        }),
        json!({
            "name": "config",
            "kind": "config",
            "description": "Print the resolved Dart Decimate configuration.",
            "schema": CONFIG_SCHEMA_VERSION,
            "flags": ["--root", "--format", "--config", "--path"]
        }),
        json!({
            "name": "schema",
            "kind": "schema",
            "description": "Print this machine-readable CLI and issue manifest.",
            "schema": MANIFEST_SCHEMA_VERSION,
            "flags": ["--format"]
        }),
        json!({
            "name": "config-schema",
            "kind": "config-schema",
            "description": "Print the configuration JSON schema.",
            "schema": CONFIG_SCHEMA_VERSION,
            "flags": ["--format"]
        }),
        json!({
            "name": "report-schema",
            "kind": "report-schema",
            "description": "Print the analysis report JSON schema.",
            "schema": SCHEMA_VERSION,
            "flags": ["--format"]
        }),
        json!({
            "name": "rule-pack-schema",
            "kind": "rule-pack-schema",
            "description": "Print the policy rule-pack JSON schema.",
            "schema": RULE_PACK_SCHEMA_VERSION,
            "flags": ["--format"]
        }),
    ]
}

fn issue_types() -> Value {
    json!([
        "dead-file",
        "unused-export",
        "unused-type",
        "private-type-leak",
        "unused-enum-member",
        "unused-class-member",
        "duplicate-export",
        "route-collision",
        "private-widget-class",
        "widget-top-level-function-boundary",
        "unused-widget-param",
        "unrendered-widget",
        "missing-context-mounted-after-await",
        "missing-entry-point",
        "circular-dependency",
        "re-export-cycle",
        "boundary-violation",
        "boundary-coverage",
        "boundary-call-violation",
        "policy-violation",
        "unresolved-dependency",
        "part-of-violation",
        "unused-dependency",
        "unused-dev-dependency",
        "test-only-dependency",
        "unused-dependency-override",
        "misconfigured-dependency-override",
        "unlisted-dependency",
        "private-src-import",
        "code-duplication",
        "high-cyclomatic-complexity",
        "high-cognitive-complexity",
        "high-complexity",
        "coverage-gap",
        "high-crap-score",
        "health-hotspot",
        "refactoring-target",
        "feature-flag",
        "security-candidate",
        "stale-suppression",
        "missing-suppression-reason"
    ])
}

fn task_matrix() -> Value {
    json!([
        {
            "intent": "find cleanup opportunities",
            "command": "dart-decimate check --format json",
            "reason": "Combined graph, symbol, dependency, duplication, health, flag, and security report."
        },
        {
            "intent": "review changed code",
            "command": "dart-decimate audit --format json --base <ref>",
            "reason": "Changed-file gate with related graph findings."
        },
        {
            "intent": "review structural decisions",
            "command": "dart-decimate decision-surface --format json --base <ref>",
            "reason": "Ranked changed-code questions for architecture, API, and dependency reviewers."
        },
        {
            "intent": "delete an unused file or export",
            "command": "dart-decimate inspect --format json --file <path>",
            "reason": "Evidence bundle before any deletion or suppression."
        },
        {
            "intent": "trace a top-level symbol",
            "command": "dart-decimate inspect --format json --symbol <file>:<symbol>",
            "reason": "Declaration, references, re-export chains, and file-scoped findings."
        },
        {
            "intent": "verify an unused dependency",
            "command": "dart-decimate trace-dependency --format json --dependency <package>",
            "reason": "Pubspec declarations and Dart import/export usage."
        },
        {
            "intent": "consolidate duplicated code",
            "command": "dart-decimate trace-clone --format json --fingerprint <fingerprint>",
            "reason": "Duplicate group instances and extraction suggestion."
        },
        {
            "intent": "scope a monorepo",
            "command": "dart-decimate check --format json --workspace <pattern>",
            "reason": "Restrict findings to matching local pub packages."
        },
        {
            "intent": "explain an issue",
            "command": "dart-decimate explain --format json <issue-type>",
            "reason": "Rule rationale, aliases, suppressions, and follow-up commands."
        },
        {
            "intent": "show local value report",
            "command": "dart-decimate impact --format json --quiet",
            "reason": "Read-only local impact report; disabled projects return a populated zero-count report."
        },
        {
            "intent": "initialize a Dart or Flutter project",
            "command": "dart-decimate init --agents",
            "reason": "Write agent-first Dart Decimate defaults and optional coding-agent guidance."
        },
        {
            "intent": "guard commits",
            "command": "dart-decimate hooks install --target git --branch origin/main",
            "reason": "Install a Dart Decimate-managed pre-commit hook that runs changed-code audit."
        },
        {
            "intent": "set up runtime coverage",
            "command": "dart-decimate coverage setup --non-interactive --format json",
            "reason": "Read-only setup plan; add --yes to create local coverage defaults."
        },
        {
            "intent": "set up CI",
            "command": "dart-decimate ci-template github --format yaml",
            "reason": "Read-only CI template output for changed-code audit gating."
        }
    ])
}
