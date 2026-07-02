use serde_json::{Map, Value, json};

pub(super) fn tools() -> Value {
    let mut values = Vec::new();
    values.extend(overview_tools());
    values.extend(trace_tools());
    values.extend(analysis_tools());
    values.extend(change_tools());
    Value::Array(values)
}

fn overview_tools() -> Vec<Value> {
    vec![
        code_execute_tool(),
        analyze_tool(),
        check_changed_tool(),
        project_info_tool(),
        list_boundaries_tool(),
        inspect_target_tool(),
    ]
}

fn code_execute_tool() -> Value {
    tool(
        "code_execute",
        "Compose bounded read-only Dart Decimate MCP calls with a JSON program.",
        schema(
            &[
                ("code", "object"),
                ("program", "object"),
                ("max_steps", "integer"),
                ("max_tool_calls", "integer"),
                ("max_result_bytes", "integer"),
            ],
            &[],
        ),
    )
}

fn analyze_tool() -> Value {
    tool(
        "analyze",
        "Run Dart Decimate check and return the JSON report.",
        schema(
            &[
                ("root", "string"),
                ("config", "string"),
                ("issue_types", "array"),
                ("entry", "array"),
                ("dart_platform", "string"),
                ("file", "array"),
                ("workspace", "array"),
                ("changed_workspaces", "string"),
                ("changed_since", "string"),
                ("compare", "string"),
                ("baseline", "string"),
                ("regression_baseline", "string"),
                ("fail_on_regression", "boolean"),
                ("tolerance", "string"),
                ("include_entry_exports", "boolean"),
                ("private_type_leaks", "boolean"),
                ("boundary", "array"),
                ("boundary_coverage", "boolean"),
                ("boundary_call", "array"),
                ("policy_pack", "array"),
                ("policy_violations", "boolean"),
                ("mode", "string"),
                ("min_tokens", "integer"),
                ("min_lines", "integer"),
                ("min_occurrences", "integer"),
                ("top", "integer"),
                ("threshold", "number"),
                ("cross_language", "boolean"),
                ("skip_local", "boolean"),
                ("ignore_imports", "boolean"),
                ("no_ignore_imports", "boolean"),
                ("max_cyclomatic", "integer"),
                ("max_cognitive", "integer"),
                ("max_crap", "integer"),
                ("coverage", "string"),
                ("runtime_coverage", "string"),
                ("min_invocations_hot", "integer"),
                ("min_observation_volume", "integer"),
                ("low_traffic_threshold", "number"),
                ("coverage_gaps", "boolean"),
                ("file_scores", "boolean"),
                ("hotspots", "boolean"),
                ("targets", "boolean"),
                ("ownership", "boolean"),
                ("complexity_breakdown", "boolean"),
                ("min_score", "integer"),
                ("production", "boolean"),
            ],
            &[],
        ),
    )
}

fn check_changed_tool() -> Value {
    tool(
        "check_changed",
        "Run Dart Decimate check scoped to files changed since a Git ref.",
        schema(
            &[
                ("root", "string"),
                ("config", "string"),
                ("since", "string"),
                ("changed_since", "string"),
                ("compare", "string"),
                ("baseline", "string"),
                ("regression_baseline", "string"),
                ("fail_on_regression", "boolean"),
                ("tolerance", "string"),
                ("production", "boolean"),
            ],
            &[],
        ),
    )
}

fn project_info_tool() -> Value {
    tool(
        "project_info",
        "List Dart Decimate project metadata.",
        schema(
            &[
                ("root", "string"),
                ("config", "string"),
                ("files", "boolean"),
                ("entry_points", "boolean"),
                ("plugins", "boolean"),
                ("boundaries", "boolean"),
                ("workspaces", "boolean"),
                ("entry", "array"),
                ("dart_platform", "string"),
                ("file", "array"),
                ("workspace", "array"),
                ("changed_workspaces", "string"),
                ("production", "boolean"),
            ],
            &[],
        ),
    )
}

fn list_boundaries_tool() -> Value {
    tool(
        "list_boundaries",
        "List configured architecture boundaries.",
        schema(
            &[
                ("root", "string"),
                ("config", "string"),
                ("entry", "array"),
                ("dart_platform", "string"),
                ("file", "array"),
                ("workspace", "array"),
                ("changed_workspaces", "string"),
                ("production", "boolean"),
            ],
            &[],
        ),
    )
}

fn inspect_target_tool() -> Value {
    tool(
        "inspect_target",
        "Inspect one Dart file or symbol.",
        schema(
            &[
                ("root", "string"),
                ("config", "string"),
                ("target", "target"),
                ("file", "string"),
                ("symbol", "string"),
                ("production", "boolean"),
            ],
            &[],
        ),
    )
}

fn trace_tools() -> Vec<Value> {
    vec![
        tool(
            "trace_file",
            "Trace a Dart file.",
            schema(
                &[("root", "string"), ("config", "string"), ("file", "string")],
                &["file"],
            ),
        ),
        tool(
            "trace_export",
            "Trace a top-level Dart symbol.",
            schema(
                &[
                    ("root", "string"),
                    ("config", "string"),
                    ("file", "string"),
                    ("symbol", "string"),
                    ("export_name", "string"),
                ],
                &["file"],
            ),
        ),
        tool(
            "trace_dependency",
            "Trace pub dependency usage.",
            schema(
                &[
                    ("root", "string"),
                    ("config", "string"),
                    ("dependency", "string"),
                    ("package_name", "string"),
                ],
                &[],
            ),
        ),
        tool(
            "trace_clone",
            "Trace a duplicate-code clone group.",
            schema(
                &[
                    ("root", "string"),
                    ("config", "string"),
                    ("fingerprint", "string"),
                    ("file", "string"),
                    ("line", "integer"),
                    ("mode", "string"),
                    ("min_tokens", "integer"),
                    ("min_lines", "integer"),
                    ("min_occurrences", "integer"),
                    ("top", "integer"),
                    ("threshold", "number"),
                    ("cross_language", "boolean"),
                    ("skip_local", "boolean"),
                    ("ignore_imports", "boolean"),
                    ("no_ignore_imports", "boolean"),
                ],
                &[],
            ),
        ),
    ]
}

fn analysis_tools() -> Vec<Value> {
    vec![
        dupes_tool(),
        health_tool(),
        runtime_tool(
            "check_runtime_coverage",
            "Merge local V8 or Istanbul runtime coverage into Dart Decimate runtime intelligence.",
            "coverage",
        ),
        runtime_tool(
            "get_hot_paths",
            "Return runtime coverage context for hot-path review.",
            "coverage",
        ),
        runtime_tool(
            "get_blast_radius",
            "Return runtime coverage context for blast-radius review.",
            "coverage",
        ),
        runtime_tool(
            "get_importance",
            "Return runtime coverage context for production-importance review.",
            "coverage",
        ),
        runtime_tool(
            "get_cleanup_candidates",
            "Return runtime coverage context for low-traffic and unavailable-code cleanup review.",
            "coverage",
        ),
        security_tool(),
        feature_flags_tool(),
        impact_tool(),
        impact_all_tool(),
    ]
}

fn dupes_tool() -> Value {
    tool(
        "find_dupes",
        "Find duplicated Dart code blocks.",
        schema(
            &[
                ("root", "string"),
                ("config", "string"),
                ("mode", "string"),
                ("entry", "array"),
                ("dart_platform", "string"),
                ("file", "array"),
                ("workspace", "array"),
                ("changed_workspaces", "string"),
                ("changed_since", "string"),
                ("compare", "string"),
                ("baseline", "string"),
                ("regression_baseline", "string"),
                ("fail_on_regression", "boolean"),
                ("tolerance", "string"),
                ("production", "boolean"),
                ("min_tokens", "integer"),
                ("min_lines", "integer"),
                ("min_occurrences", "integer"),
                ("top", "integer"),
                ("threshold", "number"),
                ("cross_language", "boolean"),
                ("skip_local", "boolean"),
                ("ignore_imports", "boolean"),
                ("no_ignore_imports", "boolean"),
            ],
            &[],
        ),
    )
}

fn health_tool() -> Value {
    tool(
        "check_health",
        "Run Dart Decimate health checks.",
        schema(
            &[
                ("root", "string"),
                ("config", "string"),
                ("max_cyclomatic", "integer"),
                ("max_cognitive", "integer"),
                ("max_crap", "integer"),
                ("coverage", "string"),
                ("entry", "array"),
                ("dart_platform", "string"),
                ("file", "array"),
                ("workspace", "array"),
                ("changed_workspaces", "string"),
                ("changed_since", "string"),
                ("compare", "string"),
                ("baseline", "string"),
                ("regression_baseline", "string"),
                ("fail_on_regression", "boolean"),
                ("tolerance", "string"),
                ("runtime_coverage", "string"),
                ("min_invocations_hot", "integer"),
                ("min_observation_volume", "integer"),
                ("low_traffic_threshold", "number"),
                ("coverage_gaps", "boolean"),
                ("file_scores", "boolean"),
                ("hotspots", "boolean"),
                ("targets", "boolean"),
                ("ownership", "boolean"),
                ("complexity_breakdown", "boolean"),
                ("min_score", "integer"),
                ("top", "integer"),
                ("production", "boolean"),
            ],
            &[],
        ),
    )
}

fn runtime_tool(name: &str, description: &str, required: &str) -> Value {
    tool(
        name,
        description,
        schema(
            &[
                ("root", "string"),
                ("config", "string"),
                ("coverage", "string"),
                ("min_invocations_hot", "integer"),
                ("min_observation_volume", "integer"),
                ("low_traffic_threshold", "number"),
                ("top", "integer"),
                ("repo", "string"),
            ],
            &[required],
        ),
    )
}

fn security_tool() -> Value {
    tool(
        "security_candidates",
        "Surface security review candidates.",
        schema(
            &[
                ("root", "string"),
                ("config", "string"),
                ("top", "integer"),
                ("entry", "array"),
                ("dart_platform", "string"),
                ("file", "array"),
                ("workspace", "array"),
                ("changed_workspaces", "string"),
                ("changed_since", "string"),
                ("compare", "string"),
                ("baseline", "string"),
                ("regression_baseline", "string"),
                ("fail_on_regression", "boolean"),
                ("tolerance", "string"),
                ("surface", "boolean"),
                ("production", "boolean"),
                ("gate", "string"),
                ("paths", "array"),
                ("diff_file", "string"),
                ("ci", "boolean"),
                ("fail_on_issues", "boolean"),
                ("summary", "boolean"),
            ],
            &[],
        ),
    )
}

fn impact_tool() -> Value {
    tool(
        "impact",
        "Read the local Dart Decimate value report without running analysis.",
        schema(&[("root", "string")], &[]),
    )
}

fn impact_all_tool() -> Value {
    tool(
        "impact_all",
        "Roll up every tracked Dart Decimate project on this machine.",
        schema(&[("sort", "string"), ("limit", "integer")], &[]),
    )
}

fn feature_flags_tool() -> Value {
    tool(
        "feature_flags",
        "Inventory Dart and Flutter feature flags.",
        schema(
            &[
                ("root", "string"),
                ("config", "string"),
                ("entry", "array"),
                ("dart_platform", "string"),
                ("file", "array"),
                ("workspace", "array"),
                ("changed_workspaces", "string"),
                ("top", "integer"),
                ("changed_since", "string"),
                ("compare", "string"),
                ("baseline", "string"),
                ("regression_baseline", "string"),
                ("fail_on_regression", "boolean"),
                ("tolerance", "string"),
                ("production", "boolean"),
            ],
            &[],
        ),
    )
}

fn change_tools() -> Vec<Value> {
    vec![
        fix_preview_tool(),
        fix_apply_tool(),
        audit_tool(),
        decision_surface_tool(),
        explain_tool("dart_decimate_explain"),
        explain_tool("fallow_explain"),
    ]
}

fn fix_preview_tool() -> Value {
    tool(
        "fix_preview",
        "Preview safe auto-fixes without modifying files.",
        schema(
            &[
                ("root", "string"),
                ("config", "string"),
                ("entry", "array"),
                ("dart_platform", "string"),
                ("file", "array"),
                ("workspace", "array"),
                ("changed_workspaces", "string"),
                ("changed_since", "string"),
                ("compare", "string"),
                ("production", "boolean"),
                ("action", "array"),
                ("no_create_config", "boolean"),
            ],
            &[],
        ),
    )
}

fn fix_apply_tool() -> Value {
    tool_with_annotations(
        "fix_apply",
        "Apply safe auto-fixes. Requires yes: true.",
        schema(
            &[
                ("root", "string"),
                ("config", "string"),
                ("entry", "array"),
                ("dart_platform", "string"),
                ("file", "array"),
                ("workspace", "array"),
                ("changed_workspaces", "string"),
                ("changed_since", "string"),
                ("compare", "string"),
                ("production", "boolean"),
                ("action", "array"),
                ("no_create_config", "boolean"),
                ("yes", "boolean"),
            ],
            &["yes"],
        ),
        false,
        true,
    )
}

fn audit_tool() -> Value {
    tool(
        "audit",
        "Run changed-code audit.",
        schema(
            &[
                ("root", "string"),
                ("config", "string"),
                ("base", "string"),
                ("gate", "string"),
                ("entry", "array"),
                ("dart_platform", "string"),
                ("file", "array"),
                ("workspace", "array"),
                ("changed_workspaces", "string"),
                ("changed_since", "string"),
                ("compare", "string"),
                ("include_entry_exports", "boolean"),
                ("private_type_leaks", "boolean"),
                ("boundary", "array"),
                ("boundary_coverage", "boolean"),
                ("boundary_call", "array"),
                ("policy_pack", "array"),
                ("policy_violations", "boolean"),
                ("mode", "string"),
                ("min_tokens", "integer"),
                ("min_lines", "integer"),
                ("min_occurrences", "integer"),
                ("top", "integer"),
                ("threshold", "number"),
                ("cross_language", "boolean"),
                ("skip_local", "boolean"),
                ("ignore_imports", "boolean"),
                ("no_ignore_imports", "boolean"),
                ("max_cyclomatic", "integer"),
                ("max_cognitive", "integer"),
                ("max_crap", "integer"),
                ("coverage", "string"),
                ("runtime_coverage", "string"),
                ("min_invocations_hot", "integer"),
                ("min_observation_volume", "integer"),
                ("low_traffic_threshold", "number"),
                ("coverage_gaps", "boolean"),
                ("file_scores", "boolean"),
                ("hotspots", "boolean"),
                ("targets", "boolean"),
                ("ownership", "boolean"),
                ("complexity_breakdown", "boolean"),
                ("min_score", "integer"),
                ("dead_code_baseline", "string"),
                ("health_baseline", "string"),
                ("dupes_baseline", "string"),
                ("production", "boolean"),
                ("brief", "boolean"),
                ("max_decisions", "integer"),
            ],
            &["base"],
        ),
    )
}

fn decision_surface_tool() -> Value {
    tool(
        "decision_surface",
        "Surface changed-code architecture decisions.",
        schema(
            &[
                ("root", "string"),
                ("config", "string"),
                ("base", "string"),
                ("max_decisions", "integer"),
            ],
            &["base"],
        ),
    )
}

fn explain_tool(name: &str) -> Value {
    tool(
        name,
        "Explain a Dart Decimate issue type.",
        schema(&[("issue_type", "string"), ("rule_id", "string")], &[]),
    )
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    tool_with_annotations(name, description, input_schema, true, false)
}

fn tool_with_annotations(
    name: &str,
    description: &str,
    input_schema: Value,
    read_only: bool,
    destructive: bool,
) -> Value {
    let mut tool = Map::new();
    tool.insert("name".to_owned(), Value::String(name.to_owned()));
    tool.insert(
        "description".to_owned(),
        Value::String(description.to_owned()),
    );
    tool.insert("inputSchema".to_owned(), input_schema);
    tool.insert(
        "annotations".to_owned(),
        json!({
            "readOnlyHint": read_only,
            "destructiveHint": destructive,
            "openWorldHint": false
        }),
    );
    Value::Object(tool)
}

fn schema(properties: &[(&str, &str)], required: &[&str]) -> Value {
    let properties = properties
        .iter()
        .map(|(name, kind)| ((*name).to_owned(), property_schema(kind)))
        .collect::<Map<_, _>>();
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

fn property_schema(kind: &str) -> Value {
    match kind {
        "array" => json!({ "type": "array", "items": { "type": "string" } }),
        "object" => json!({ "type": "object" }),
        "target" => target_schema(),
        _ => json!({ "type": kind }),
    }
}

fn target_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "type": { "type": "string", "enum": ["file", "symbol"] },
            "file": { "type": "string" },
            "symbol": { "type": "string" },
            "export_name": { "type": "string" }
        },
        "required": ["type"],
        "additionalProperties": false
    })
}
