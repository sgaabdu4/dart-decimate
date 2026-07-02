use serde_json::{Value, json};

pub(super) fn clone_group_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["fingerprint", "instances", "line_count", "token_count"],
        "properties": {
            "fingerprint": string_schema(),
            "instances": array_ref_schema("clone_instance"),
            "line_count": nonnegative_integer_schema(),
            "token_count": nonnegative_integer_schema()
        }
    })
}

pub(super) fn clone_instance_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["path", "start_line", "end_line", "column"],
        "properties": {
            "path": string_schema(),
            "start_line": positive_integer_schema(),
            "end_line": positive_integer_schema(),
            "column": nonnegative_integer_schema()
        }
    })
}

pub(super) fn complexity_finding_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "rule_id",
            "path",
            "symbol",
            "kind",
            "line",
            "column",
            "cyclomatic_complexity",
            "cognitive_complexity",
            "line_coverage_percent",
            "covered_lines",
            "executable_lines",
            "crap_score",
            "coverage_status",
            "contributions"
        ],
        "properties": {
            "rule_id": string_schema(),
            "path": string_schema(),
            "symbol": string_schema(),
            "kind": string_schema(),
            "line": positive_integer_schema(),
            "column": nonnegative_integer_schema(),
            "cyclomatic_complexity": nonnegative_integer_schema(),
            "cognitive_complexity": nonnegative_integer_schema(),
            "line_coverage_percent": nullable_nonnegative_integer_schema(),
            "covered_lines": nullable_nonnegative_integer_schema(),
            "executable_lines": nullable_nonnegative_integer_schema(),
            "crap_score": nullable_nonnegative_integer_schema(),
            "coverage_status": nullable_string_schema(),
            "effective_thresholds": { "$ref": "#/$defs/effective_thresholds" },
            "threshold_source": nullable_string_schema(),
            "threshold_reason": nullable_string_schema(),
            "contributions": array_ref_schema("complexity_contribution")
        }
    })
}

pub(super) fn complexity_contribution_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["line", "column", "kind", "cyclomatic", "cognitive", "nesting"],
        "properties": {
            "line": positive_integer_schema(),
            "column": nonnegative_integer_schema(),
            "kind": string_schema(),
            "cyclomatic": nonnegative_integer_schema(),
            "cognitive": nonnegative_integer_schema(),
            "nesting": nonnegative_integer_schema()
        }
    })
}

pub(super) fn effective_thresholds_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "max_cyclomatic": nonnegative_integer_schema(),
            "max_cognitive": nonnegative_integer_schema(),
            "max_crap": nonnegative_integer_schema()
        }
    })
}

pub(super) fn file_health_score_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "path",
            "score",
            "functions",
            "complex_functions",
            "max_cyclomatic_complexity",
            "max_cognitive_complexity",
            "max_crap_score",
            "coverage_status",
            "covered_lines",
            "executable_lines",
            "line_coverage_percent",
            "reasons",
            "owners",
            "owner_source",
            "owner_section"
        ],
        "properties": {
            "path": string_schema(),
            "score": nonnegative_integer_schema(),
            "functions": nonnegative_integer_schema(),
            "complex_functions": nonnegative_integer_schema(),
            "max_cyclomatic_complexity": nonnegative_integer_schema(),
            "max_cognitive_complexity": nonnegative_integer_schema(),
            "max_crap_score": nonnegative_integer_schema(),
            "coverage_status": coverage_status_schema(),
            "covered_lines": nullable_nonnegative_integer_schema(),
            "executable_lines": nullable_nonnegative_integer_schema(),
            "line_coverage_percent": nullable_nonnegative_integer_schema(),
            "reasons": string_array_schema(),
            "owners": string_array_schema(),
            "owner_source": nullable_string_schema(),
            "owner_section": nullable_string_schema()
        }
    })
}

pub(super) fn health_hotspot_schema() -> Value {
    health_location_score_schema(&[
        "path",
        "line",
        "column",
        "score",
        "reasons",
        "owners",
        "owner_source",
        "owner_section",
    ])
}

pub(super) fn refactoring_target_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "path",
            "line",
            "column",
            "score",
            "priority",
            "reasons",
            "owners",
            "owner_source",
            "owner_section"
        ],
        "properties": {
            "path": string_schema(),
            "line": positive_integer_schema(),
            "column": nonnegative_integer_schema(),
            "score": nonnegative_integer_schema(),
            "priority": nonnegative_integer_schema(),
            "reasons": string_array_schema(),
            "owners": string_array_schema(),
            "owner_source": nullable_string_schema(),
            "owner_section": nullable_string_schema()
        }
    })
}

pub(super) fn feature_flag_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["name", "source", "provider", "confidence", "occurrences"],
        "properties": {
            "name": string_schema(),
            "source": { "type": "string", "enum": ["compile-time-environment", "process-environment", "sdk-call"] },
            "provider": string_schema(),
            "confidence": confidence_schema(),
            "occurrences": array_ref_schema("feature_flag_occurrence")
        }
    })
}

pub(super) fn feature_flag_occurrence_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["path", "line", "column", "expression"],
        "properties": location_expression_properties()
    })
}

pub(super) fn security_candidate_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "rule_id",
            "finding_id",
            "fingerprint",
            "category",
            "cwe",
            "severity",
            "candidate",
            "sink",
            "confidence",
            "occurrences",
            "evidence",
            "trace"
        ],
        "properties": {
            "rule_id": string_schema(),
            "finding_id": string_schema(),
            "fingerprint": string_schema(),
            "category": security_category_schema(),
            "cwe": string_array_schema(),
            "severity": severity_schema(),
            "candidate": {
                "type": "object",
                "additionalProperties": false,
                "required": ["source", "sink", "boundary"],
                "properties": {
                    "source": string_schema(),
                    "sink": string_schema(),
                    "boundary": string_schema()
                }
            },
            "sink": string_schema(),
            "confidence": confidence_schema(),
            "occurrences": array_ref_schema("security_occurrence"),
            "reachability": security_reachability_schema(),
            "evidence": {
                "type": "object",
                "additionalProperties": false,
                "required": ["occurrences"],
                "properties": {
                    "occurrences": array_ref_schema("security_occurrence")
                }
            },
            "trace": {
                "type": "array",
                "items": security_trace_step_schema()
            }
        }
    })
}

pub(super) fn security_occurrence_schema() -> Value {
    let mut properties = location_expression_properties();
    if let Some(object) = properties.as_object_mut() {
        object.insert("evidence".to_owned(), string_schema());
        object.insert("reachability".to_owned(), security_reachability_schema());
    }
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["path", "line", "column", "expression", "evidence"],
        "properties": properties
    })
}

fn security_reachability_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "reachable_from_entrypoint",
            "taint_confidence",
            "entry_points",
            "reachable_occurrences"
        ],
        "properties": {
            "reachable_from_entrypoint": { "type": "boolean" },
            "taint_confidence": { "type": "string", "enum": ["module-level"] },
            "entry_points": string_array_schema(),
            "reachable_occurrences": nonnegative_integer_schema()
        }
    })
}

fn security_trace_step_schema() -> Value {
    let mut properties = location_expression_properties();
    if let Some(object) = properties.as_object_mut() {
        object.insert(
            "role".to_owned(),
            json!({ "type": "string", "enum": ["source", "sink", "boundary"] }),
        );
        object.insert("evidence".to_owned(), string_schema());
    }
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["role", "path", "line", "column", "expression", "evidence"],
        "properties": properties
    })
}

pub(super) fn attack_surface_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["category", "path", "line", "column", "surface", "verification_prompt"],
        "properties": {
            "category": security_category_schema(),
            "path": string_schema(),
            "line": positive_integer_schema(),
            "column": nonnegative_integer_schema(),
            "surface": string_schema(),
            "verification_prompt": string_schema()
        }
    })
}

fn health_location_score_schema(required: &[&str]) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": required.to_vec(),
        "properties": {
            "path": string_schema(),
            "line": positive_integer_schema(),
            "column": nonnegative_integer_schema(),
            "score": nonnegative_integer_schema(),
            "reasons": string_array_schema(),
            "owners": string_array_schema(),
            "owner_source": nullable_string_schema(),
            "owner_section": nullable_string_schema()
        }
    })
}

fn location_expression_properties() -> Value {
    json!({
        "path": string_schema(),
        "line": positive_integer_schema(),
        "column": nonnegative_integer_schema(),
        "expression": string_schema()
    })
}

fn security_category_schema() -> Value {
    json!({
        "type": "string",
        "enum": [
            "hardcoded-secret",
            "firebase-api-key",
            "insecure-transport",
            "tls-bypass",
            "web-view-risk",
            "process-execution",
            "raw-sql",
            "plain-secret-storage"
        ]
    })
}

fn coverage_status_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["not-requested", "missing", "no-executable-lines", "uncovered", "covered"]
    })
}

fn confidence_schema() -> Value {
    json!({ "type": "string", "enum": ["low", "medium", "high"] })
}

fn severity_schema() -> Value {
    json!({ "type": "string", "enum": ["error", "warning"] })
}

fn array_ref_schema(definition: &str) -> Value {
    json!({
        "type": "array",
        "items": { "$ref": format!("#/$defs/{definition}") }
    })
}

fn string_schema() -> Value {
    json!({ "type": "string" })
}

fn nullable_string_schema() -> Value {
    json!({ "type": ["string", "null"] })
}

fn positive_integer_schema() -> Value {
    json!({ "type": "integer", "minimum": 1 })
}

fn nonnegative_integer_schema() -> Value {
    json!({ "type": "integer", "minimum": 0 })
}

fn nullable_nonnegative_integer_schema() -> Value {
    json!({ "type": ["integer", "null"], "minimum": 0 })
}

fn string_array_schema() -> Value {
    json!({
        "type": "array",
        "items": { "type": "string" }
    })
}
