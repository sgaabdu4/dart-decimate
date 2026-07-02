use serde_json::{Value, json};

use crate::coverage::COVERAGE_ANALYSIS_SCHEMA_VERSION;
use crate::decision_surface::DECISION_SURFACE_SCHEMA_VERSION;
use crate::explain::EXPLAIN_SCHEMA_VERSION;
use crate::fix::FIX_SCHEMA_VERSION;
use crate::impact::IMPACT_SCHEMA_VERSION;
use crate::inspect::INSPECT_SCHEMA_VERSION;
use crate::mcp::code_execute::CODE_EXECUTE_SCHEMA_VERSION;
use crate::output::{SCHEMA_VERSION, TRACE_SCHEMA_VERSION};
use crate::project_list::PROJECT_LIST_SCHEMA_VERSION;

pub(super) fn mcp_tools() -> Value {
    json!({
        "server": "dart-decimate-mcp",
        "note": "Agent tool contracts backed by existing Dart Decimate CLI commands. fix_apply is mutating and requires yes: true.",
        "tools": mcp_tool_list()
    })
}

fn mcp_tool_list() -> Vec<Value> {
    let mut tools = Vec::new();
    tools.extend(mcp_overview_tools());
    tools.extend(mcp_trace_tools());
    tools.extend(mcp_analysis_tools());
    tools.extend(mcp_change_tools());
    tools
}

fn mcp_overview_tools() -> Vec<Value> {
    vec![
        mcp_tool(
            "code_execute",
            "dart-decimate-mcp code_execute",
            CODE_EXECUTE_SCHEMA_VERSION,
            &["code", "max_steps", "max_tool_calls", "max_result_bytes"],
        ),
        analyze_mcp_tool(),
        mcp_tool(
            "check_changed",
            "dart-decimate check --format json --changed-since",
            SCHEMA_VERSION,
            &[
                "root",
                "config",
                "since",
                "changed_since",
                "compare",
                "baseline",
                "regression_baseline",
                "fail_on_regression",
                "tolerance",
                "production",
            ],
        ),
        mcp_tool(
            "project_info",
            "dart-decimate list --format json",
            PROJECT_LIST_SCHEMA_VERSION,
            &[
                "root",
                "config",
                "files",
                "entry_points",
                "plugins",
                "boundaries",
                "workspaces",
                "entry",
                "dart_platform",
                "file",
                "workspace",
                "changed_workspaces",
                "production",
            ],
        ),
        mcp_tool(
            "list_boundaries",
            "dart-decimate list --format json --boundaries",
            PROJECT_LIST_SCHEMA_VERSION,
            &[
                "root",
                "config",
                "entry",
                "dart_platform",
                "file",
                "workspace",
                "changed_workspaces",
                "production",
            ],
        ),
        mcp_tool(
            "inspect_target",
            "dart-decimate inspect --format json",
            INSPECT_SCHEMA_VERSION,
            &["root", "config", "target", "file", "symbol", "production"],
        ),
    ]
}

fn mcp_trace_tools() -> Vec<Value> {
    vec![
        mcp_tool(
            "trace_file",
            "dart-decimate trace-file --format json",
            TRACE_SCHEMA_VERSION,
            &["root", "config", "file"],
        ),
        mcp_tool(
            "trace_export",
            "dart-decimate trace-symbol --format json",
            TRACE_SCHEMA_VERSION,
            &["root", "config", "file", "symbol", "export_name"],
        ),
        mcp_tool(
            "trace_dependency",
            "dart-decimate trace-dependency --format json",
            TRACE_SCHEMA_VERSION,
            &["root", "config", "dependency", "package_name"],
        ),
        mcp_tool(
            "trace_clone",
            "dart-decimate trace-clone --format json",
            TRACE_SCHEMA_VERSION,
            &[
                "root",
                "config",
                "fingerprint",
                "file",
                "line",
                "mode",
                "min_tokens",
                "min_lines",
                "min_occurrences",
                "top",
                "threshold",
                "cross_language",
                "skip_local",
                "ignore_imports",
                "no_ignore_imports",
            ],
        ),
    ]
}

fn analyze_mcp_tool() -> Value {
    mcp_tool(
        "analyze",
        "dart-decimate check --format json",
        SCHEMA_VERSION,
        &[
            "root",
            "config",
            "issue_types",
            "entry",
            "dart_platform",
            "file",
            "workspace",
            "changed_workspaces",
            "changed_since",
            "compare",
            "baseline",
            "regression_baseline",
            "fail_on_regression",
            "tolerance",
            "include_entry_exports",
            "private_type_leaks",
            "boundary",
            "boundary_coverage",
            "boundary_call",
            "policy_pack",
            "policy_violations",
            "mode",
            "min_tokens",
            "min_lines",
            "min_occurrences",
            "top",
            "threshold",
            "cross_language",
            "skip_local",
            "ignore_imports",
            "no_ignore_imports",
            "max_cyclomatic",
            "max_cognitive",
            "max_crap",
            "coverage",
            "runtime_coverage",
            "min_invocations_hot",
            "min_observation_volume",
            "low_traffic_threshold",
            "coverage_gaps",
            "file_scores",
            "hotspots",
            "targets",
            "ownership",
            "complexity_breakdown",
            "min_score",
            "production",
        ],
    )
}

fn mcp_analysis_tools() -> Vec<Value> {
    let mut tools = vec![dupes_mcp_tool(), health_mcp_tool()];
    tools.extend(runtime_mcp_tools());
    tools.extend([
        security_mcp_tool(),
        feature_flags_mcp_tool(),
        mcp_tool(
            "impact",
            "dart-decimate impact --format json --quiet",
            IMPACT_SCHEMA_VERSION,
            &["root"],
        ),
        mcp_tool(
            "impact_all",
            "dart-decimate impact --format json --quiet --all",
            IMPACT_SCHEMA_VERSION,
            &["sort", "limit"],
        ),
    ]);
    tools
}

fn dupes_mcp_tool() -> Value {
    mcp_tool(
        "find_dupes",
        "dart-decimate dupes --format json",
        SCHEMA_VERSION,
        &[
            "root",
            "config",
            "entry",
            "dart_platform",
            "file",
            "workspace",
            "changed_workspaces",
            "changed_since",
            "compare",
            "baseline",
            "regression_baseline",
            "fail_on_regression",
            "tolerance",
            "production",
            "mode",
            "min_tokens",
            "min_lines",
            "min_occurrences",
            "top",
            "threshold",
            "cross_language",
            "skip_local",
            "ignore_imports",
            "no_ignore_imports",
        ],
    )
}

fn health_mcp_tool() -> Value {
    mcp_tool(
        "check_health",
        "dart-decimate health --format json",
        SCHEMA_VERSION,
        &[
            "root",
            "config",
            "entry",
            "dart_platform",
            "file",
            "workspace",
            "changed_workspaces",
            "changed_since",
            "compare",
            "baseline",
            "regression_baseline",
            "fail_on_regression",
            "tolerance",
            "production",
            "max_cyclomatic",
            "max_cognitive",
            "max_crap",
            "coverage",
            "runtime_coverage",
            "min_invocations_hot",
            "min_observation_volume",
            "low_traffic_threshold",
            "coverage_gaps",
            "file_scores",
            "hotspots",
            "targets",
            "ownership",
            "complexity_breakdown",
            "min_score",
            "top",
        ],
    )
}

fn runtime_mcp_tools() -> Vec<Value> {
    vec![
        runtime_mcp_tool(
            "check_runtime_coverage",
            &[
                "root",
                "config",
                "coverage",
                "min_invocations_hot",
                "min_observation_volume",
                "low_traffic_threshold",
                "top",
                "repo",
            ],
        ),
        runtime_mcp_tool(
            "get_hot_paths",
            &["root", "config", "coverage", "min_invocations_hot", "top"],
        ),
        runtime_mcp_tool(
            "get_blast_radius",
            &[
                "root",
                "config",
                "coverage",
                "min_invocations_hot",
                "min_observation_volume",
                "low_traffic_threshold",
                "top",
            ],
        ),
        runtime_mcp_tool(
            "get_importance",
            &[
                "root",
                "config",
                "coverage",
                "min_invocations_hot",
                "min_observation_volume",
                "low_traffic_threshold",
                "top",
            ],
        ),
        runtime_mcp_tool(
            "get_cleanup_candidates",
            &[
                "root",
                "config",
                "coverage",
                "min_invocations_hot",
                "min_observation_volume",
                "low_traffic_threshold",
                "top",
            ],
        ),
    ]
}

fn security_mcp_tool() -> Value {
    mcp_tool(
        "security_candidates",
        "dart-decimate security --format json",
        SCHEMA_VERSION,
        &[
            "root",
            "config",
            "entry",
            "dart_platform",
            "file",
            "workspace",
            "changed_workspaces",
            "changed_since",
            "compare",
            "baseline",
            "regression_baseline",
            "fail_on_regression",
            "tolerance",
            "top",
            "surface",
            "production",
            "gate",
            "paths",
            "diff_file",
            "ci",
            "fail_on_issues",
            "summary",
        ],
    )
}

fn feature_flags_mcp_tool() -> Value {
    mcp_tool(
        "feature_flags",
        "dart-decimate flags --format json",
        SCHEMA_VERSION,
        &[
            "root",
            "config",
            "entry",
            "dart_platform",
            "file",
            "workspace",
            "changed_workspaces",
            "changed_since",
            "compare",
            "baseline",
            "regression_baseline",
            "fail_on_regression",
            "tolerance",
            "production",
            "top",
        ],
    )
}

fn mcp_change_tools() -> Vec<Value> {
    vec![
        fix_preview_mcp_tool(),
        fix_apply_mcp_tool(),
        audit_mcp_tool(),
        mcp_tool(
            "decision_surface",
            "dart-decimate decision-surface --format json",
            DECISION_SURFACE_SCHEMA_VERSION,
            &["root", "config", "base", "max_decisions"],
        ),
        mcp_tool(
            "dart_decimate_explain",
            "dart-decimate explain --format json",
            EXPLAIN_SCHEMA_VERSION,
            &["issue_type", "rule_id"],
        ),
        mcp_tool(
            "fallow_explain",
            "dart-decimate explain --format json",
            EXPLAIN_SCHEMA_VERSION,
            &["issue_type", "rule_id"],
        ),
    ]
}

fn fix_preview_mcp_tool() -> Value {
    mcp_tool(
        "fix_preview",
        "dart-decimate fix --format json --dry-run",
        FIX_SCHEMA_VERSION,
        &[
            "root",
            "config",
            "entry",
            "dart_platform",
            "file",
            "workspace",
            "changed_workspaces",
            "changed_since",
            "compare",
            "production",
            "action",
            "no_create_config",
        ],
    )
}

fn fix_apply_mcp_tool() -> Value {
    mcp_write_tool(
        "fix_apply",
        "dart-decimate fix --format json --yes",
        FIX_SCHEMA_VERSION,
        &[
            "root",
            "config",
            "entry",
            "dart_platform",
            "file",
            "workspace",
            "changed_workspaces",
            "changed_since",
            "compare",
            "production",
            "action",
            "no_create_config",
            "yes",
        ],
    )
}

fn audit_mcp_tool() -> Value {
    mcp_tool(
        "audit",
        "dart-decimate audit --format json",
        SCHEMA_VERSION,
        &[
            "root",
            "config",
            "base",
            "gate",
            "entry",
            "dart_platform",
            "file",
            "workspace",
            "changed_workspaces",
            "changed_since",
            "compare",
            "include_entry_exports",
            "private_type_leaks",
            "boundary",
            "boundary_coverage",
            "boundary_call",
            "policy_pack",
            "policy_violations",
            "mode",
            "min_tokens",
            "min_lines",
            "min_occurrences",
            "top",
            "threshold",
            "cross_language",
            "skip_local",
            "ignore_imports",
            "no_ignore_imports",
            "max_cyclomatic",
            "max_cognitive",
            "max_crap",
            "coverage",
            "runtime_coverage",
            "min_invocations_hot",
            "min_observation_volume",
            "low_traffic_threshold",
            "coverage_gaps",
            "file_scores",
            "hotspots",
            "targets",
            "ownership",
            "complexity_breakdown",
            "min_score",
            "dead_code_baseline",
            "health_baseline",
            "dupes_baseline",
            "production",
            "brief",
            "max_decisions",
        ],
    )
}

fn runtime_mcp_tool(name: &str, key_params: &[&str]) -> Value {
    mcp_tool(
        name,
        "dart-decimate coverage analyze --format json",
        COVERAGE_ANALYSIS_SCHEMA_VERSION,
        key_params,
    )
}

fn mcp_tool(name: &str, command: &str, schema: &str, key_params: &[&str]) -> Value {
    mcp_tool_with_access(name, command, schema, key_params, true)
}

fn mcp_write_tool(name: &str, command: &str, schema: &str, key_params: &[&str]) -> Value {
    mcp_tool_with_access(name, command, schema, key_params, false)
}

fn mcp_tool_with_access(
    name: &str,
    command: &str,
    schema: &str,
    key_params: &[&str],
    read_only: bool,
) -> Value {
    json!({
        "name": name,
        "read_only": read_only,
        "command": command,
        "schema": schema,
        "key_params": key_params
    })
}
