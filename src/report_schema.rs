use serde_json::{Value, json};

use crate::output::SCHEMA_VERSION;

mod inventory;

/// Return the JSON schema for `dart-decimate.report.v1` CLI reports.
#[must_use]
pub fn report_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "schema_version": SCHEMA_VERSION,
        "title": "Dart Decimate report",
        "type": "object",
        "additionalProperties": false,
        "required": [
            "schema_version",
            "kind",
            "tool",
            "command",
            "verdict",
            "summary",
            "findings",
            "clone_groups",
            "complexity",
            "file_scores",
            "hotspots",
            "refactoring_targets",
            "threshold_overrides",
            "feature_flags",
            "security_candidates",
            "attack_surface",
            "next_steps"
        ],
        "properties": {
            "schema_version": { "const": SCHEMA_VERSION },
            "kind": { "type": "string", "enum": kind_values() },
            "tool": { "const": "dart-decimate" },
            "command": { "type": "string", "enum": command_values() },
            "verdict": { "type": "string", "enum": ["pass", "fail"] },
            "summary": { "$ref": "#/$defs/summary" },
            "findings": {
                "type": "array",
                "items": { "$ref": "#/$defs/finding" }
            },
            "clone_groups": array_ref_schema("clone_group"),
            "complexity": array_ref_schema("complexity_finding"),
            "file_scores": array_ref_schema("file_health_score"),
            "hotspots": array_ref_schema("health_hotspot"),
            "refactoring_targets": array_ref_schema("refactoring_target"),
            "threshold_overrides": threshold_overrides_schema(),
            "feature_flags": array_ref_schema("feature_flag"),
            "security_candidates": array_ref_schema("security_candidate"),
            "attack_surface": array_ref_schema("attack_surface"),
            "runtime_coverage": { "$ref": "#/$defs/runtime_coverage" },
            "next_steps": {
                "type": "array",
                "items": { "$ref": "#/$defs/next_step" }
            }
        },
        "$defs": {
            "summary": summary_schema(),
            "audit_attribution": audit_attribution_schema(),
            "audit_attribution_counts": audit_attribution_counts_schema(),
            "finding": finding_schema(),
            "finding_edge": finding_edge_schema(),
            "finding_action": finding_action_schema(),
            "clone_group": inventory::clone_group_schema(),
            "clone_instance": inventory::clone_instance_schema(),
            "complexity_finding": inventory::complexity_finding_schema(),
            "complexity_contribution": inventory::complexity_contribution_schema(),
            "effective_thresholds": inventory::effective_thresholds_schema(),
            "file_health_score": inventory::file_health_score_schema(),
            "health_hotspot": inventory::health_hotspot_schema(),
            "refactoring_target": inventory::refactoring_target_schema(),
            "feature_flag": inventory::feature_flag_schema(),
            "feature_flag_occurrence": inventory::feature_flag_occurrence_schema(),
            "security_candidate": inventory::security_candidate_schema(),
            "security_occurrence": inventory::security_occurrence_schema(),
            "attack_surface": inventory::attack_surface_schema(),
            "runtime_coverage": runtime_coverage_schema(),
            "threshold_override": threshold_override_schema(),
            "next_step": next_step_schema()
        }
    })
}

fn command_values() -> Value {
    json!([
        "check",
        "audit",
        "dead-code",
        "cycles",
        "dupes",
        "health",
        "flags",
        "security"
    ])
}

fn kind_values() -> Value {
    json!([
        "combined",
        "audit",
        "dead-code",
        "cycles",
        "dupes",
        "health",
        "flags",
        "security"
    ])
}

fn summary_schema() -> Value {
    let mut properties = serde_json::Map::new();
    for key in [
        "files",
        "edges",
        "unresolved_dependencies",
        "part_of_violations",
        "unused_dependencies",
        "unused_dev_dependencies",
        "test_only_dependencies",
        "dependency_overrides",
        "unused_dependency_overrides",
        "misconfigured_dependency_overrides",
        "unlisted_dependencies",
        "private_src_imports",
        "dead_files",
        "unused_exports",
        "unused_types",
        "private_type_leaks",
        "unused_enum_members",
        "unused_class_members",
        "duplicate_exports",
        "route_collisions",
        "mixed_go_router_styles",
        "private_widget_classes",
        "widget_top_level_functions",
        "unused_widget_params",
        "unrendered_widgets",
        "missing_context_mounted_after_await",
        "code_duplications",
        "duplication_analyzed_lines",
        "duplicated_lines",
        "duplication_percentage_basis_points",
        "quality_score",
        "health_files",
        "functions",
        "complex_functions",
        "max_cyclomatic_complexity",
        "max_cognitive_complexity",
        "coverage_files",
        "coverage_gaps",
        "crap_functions",
        "max_crap_score",
        "file_scores",
        "hotspots",
        "refactoring_targets",
        "feature_flags",
        "feature_flag_occurrences",
        "security_candidates",
        "security_candidate_occurrences",
        "attack_surface",
        "missing_entry_points",
        "cycles",
        "re_export_cycles",
        "boundary_violations",
        "boundary_coverage",
        "boundary_call_violations",
        "policy_violations",
        "missing_suppression_reasons",
        "findings",
    ] {
        properties.insert(key.to_owned(), json!({ "type": "integer", "minimum": 0 }));
    }
    properties.insert(
        "risk_score".to_owned(),
        json!({ "type": "integer", "minimum": 0, "maximum": 100 }),
    );
    properties.insert(
        "duplication_threshold_basis_points".to_owned(),
        json!({ "type": "integer", "minimum": 0, "maximum": 10000 }),
    );
    properties.insert(
        "duplication_threshold_exceeded".to_owned(),
        json!({ "type": "boolean" }),
    );
    properties.insert(
        "risk_level".to_owned(),
        json!({ "type": "string", "enum": ["pass", "warn", "fail"] }),
    );
    properties.insert(
        "attribution".to_owned(),
        json!({ "$ref": "#/$defs/audit_attribution" }),
    );

    json!({
        "type": "object",
        "additionalProperties": false,
        "required": properties
            .keys()
            .filter(|key| {
                !matches!(
                    key.as_str(),
                    "risk_score" | "risk_level" | "attribution" | "duplication_threshold_basis_points"
                )
            })
            .cloned()
            .collect::<Vec<_>>(),
        "properties": properties
    })
}

fn audit_attribution_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["introduced", "pre_existing"],
        "properties": {
            "introduced": { "$ref": "#/$defs/audit_attribution_counts" },
            "pre_existing": { "$ref": "#/$defs/audit_attribution_counts" }
        }
    })
}

fn audit_attribution_counts_schema() -> Value {
    let mut properties = serde_json::Map::new();
    for key in [
        "findings",
        "error_findings",
        "warning_findings",
        "safe_to_delete",
        "files",
    ] {
        properties.insert(key.to_owned(), json!({ "type": "integer", "minimum": 0 }));
    }

    json!({
        "type": "object",
        "additionalProperties": false,
        "required": properties.keys().cloned().collect::<Vec<_>>(),
        "properties": properties
    })
}

fn finding_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "rule_id",
            "fingerprint",
            "kind",
            "severity",
            "message",
            "path",
            "line",
            "column",
            "safe_to_delete",
            "files",
            "edge",
            "actions"
        ],
        "properties": {
            "rule_id": { "type": "string" },
            "fingerprint": { "type": ["string", "null"] },
            "kind": { "type": "string", "enum": finding_kind_values() },
            "severity": { "type": "string", "enum": ["error", "warning"] },
            "message": { "type": "string" },
            "path": { "type": "string" },
            "line": { "type": "integer", "minimum": 1 },
            "column": { "type": "integer", "minimum": 0 },
            "safe_to_delete": { "type": "boolean" },
            "files": {
                "type": "array",
                "items": { "type": "string" }
            },
            "edge": {
                "oneOf": [
                    { "type": "null" },
                    { "$ref": "#/$defs/finding_edge" }
                ]
            },
            "actions": {
                "type": "array",
                "items": { "$ref": "#/$defs/finding_action" }
            }
        }
    })
}

fn finding_kind_values() -> Value {
    json!([
        "dead-file",
        "unused-export",
        "unused-type",
        "private-type-leak",
        "unused-enum-member",
        "unused-class-member",
        "duplicate-export",
        "route-collision",
        "mixed-go-router-style",
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

fn finding_edge_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["from", "to", "specifier", "kind"],
        "properties": {
            "from": { "type": "string" },
            "to": { "type": "string" },
            "specifier": { "type": "string" },
            "kind": { "type": "string", "enum": ["import", "export", "part", "augment"] }
        }
    })
}

fn finding_action_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["action", "type", "description", "auto_fixable"],
        "properties": {
            "action": { "type": "string" },
            "type": { "type": "string" },
            "description": { "type": "string" },
            "auto_fixable": { "type": "boolean" },
            "command": { "type": "string" },
            "argv": {
                "type": "array",
                "items": { "type": "string" }
            },
            "target_path": { "type": "string" },
            "target_symbol": { "type": "string" },
            "target_dependency": { "type": "string" },
            "target_end_line": { "type": "integer", "minimum": 1 },
            "suppression_comment": { "type": "string" },
            "config_key": { "type": "string" },
            "value_schema": { "type": "string" }
        }
    })
}

fn next_step_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["id", "command", "reason"],
        "properties": {
            "id": { "type": "string" },
            "command": { "type": "string" },
            "reason": { "type": "string" }
        }
    })
}

fn runtime_coverage_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "verdict",
            "signals",
            "summary",
            "findings",
            "hot_paths",
            "coverage_intelligence",
            "blast_radius",
            "importance",
            "actionable",
            "provenance",
            "watermark",
            "warnings"
        ],
        "properties": {
            "verdict": { "type": "string", "enum": ["pass", "warn"] },
            "signals": string_array_schema(),
            "summary": { "type": "object", "additionalProperties": true },
            "findings": inventory_array_schema(),
            "hot_paths": inventory_array_schema(),
            "coverage_intelligence": inventory_array_schema(),
            "blast_radius": inventory_array_schema(),
            "importance": inventory_array_schema(),
            "actionable": { "type": "object", "additionalProperties": true },
            "provenance": { "type": "object", "additionalProperties": true },
            "watermark": { "type": "object", "additionalProperties": true },
            "warnings": string_array_schema()
        }
    })
}

fn threshold_overrides_schema() -> Value {
    json!({
        "type": "array",
        "items": { "$ref": "#/$defs/threshold_override" }
    })
}

fn threshold_override_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "index",
            "files",
            "functions",
            "max_cyclomatic",
            "max_cognitive",
            "max_crap",
            "reason",
            "active",
            "stale",
            "no_match",
            "matched_functions"
        ],
        "properties": {
            "index": { "type": "integer", "minimum": 0 },
            "files": string_array_schema(),
            "functions": string_array_schema(),
            "max_cyclomatic": { "type": ["integer", "null"], "minimum": 1 },
            "max_cognitive": { "type": ["integer", "null"], "minimum": 1 },
            "max_crap": { "type": ["integer", "null"], "minimum": 1 },
            "reason": { "type": ["string", "null"] },
            "active": { "type": "boolean" },
            "stale": { "type": "boolean" },
            "no_match": { "type": "boolean" },
            "matched_functions": string_array_schema()
        }
    })
}

fn array_ref_schema(definition: &str) -> Value {
    json!({
        "type": "array",
        "items": { "$ref": format!("#/$defs/{definition}") }
    })
}

fn string_array_schema() -> Value {
    json!({
        "type": "array",
        "items": { "type": "string" }
    })
}

fn inventory_array_schema() -> Value {
    json!({
        "type": "array",
        "items": {
            "type": "object",
            "additionalProperties": true
        }
    })
}
