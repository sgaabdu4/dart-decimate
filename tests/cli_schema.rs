use dart_decimate::cli::run_from;
use serde_json::Value;

#[test]
fn schema_command_emits_agent_manifest() -> Result<(), Box<dyn std::error::Error>> {
    let json = schema_json()?;
    assert_manifest_identity(&json);
    assert_manifest_metadata(&json);
    assert!(
        json["commands"]
            .as_array()
            .is_some_and(|commands| commands.iter().any(|command| command["name"] == "inspect"))
    );
    assert!(json["commands"].as_array().is_some_and(|commands| {
        commands.iter().any(|command| {
            command["name"] == "decision-surface"
                && command["schema"] == "dart-decimate.decision-surface.v1"
        })
    }));
    assert_eq!(json["schemas"]["coverage"], "dart-decimate.coverage.v1");
    assert_eq!(json["schemas"]["init"], "dart-decimate.init.v1");
    assert_eq!(json["schemas"]["hooks"], "dart-decimate.hooks.v1");
    assert!(json["commands"].as_array().is_some_and(|commands| {
        commands.iter().any(|command| {
            command["name"] == "trace-symbol" && command["schema"] == "dart-decimate.trace.v1"
        })
    }));
    assert!(json["commands"].as_array().is_some_and(|commands| {
        commands
            .iter()
            .any(|command| command["name"] == "trace" && command["kind"] == "trace-symbol")
    }));
    assert!(
        json["issue_types"]
            .as_array()
            .is_some_and(|issues| issues.iter().any(|issue| issue == "unused-export"))
    );
    assert!(
        json["issue_types"]
            .as_array()
            .is_some_and(|issues| issues.iter().any(|issue| issue == "unused-type"))
    );
    assert!(
        json["issue_types"]
            .as_array()
            .is_some_and(|issues| issues.iter().any(|issue| issue == "private-type-leak"))
    );
    assert!(
        json["issue_types"]
            .as_array()
            .is_some_and(|issues| issues.iter().any(|issue| issue == "unused-widget-param"))
    );
    assert!(
        json["issue_types"]
            .as_array()
            .is_some_and(|issues| issues.iter().any(|issue| issue == "unrendered-widget"))
    );
    assert!(json["issue_types"].as_array().is_some_and(|issues| {
        issues
            .iter()
            .any(|issue| issue == "missing-context-mounted-after-await")
    }));
    assert!(
        json["issue_types"]
            .as_array()
            .is_some_and(|issues| { issues.iter().any(|issue| issue == "private-widget-class") })
    );
    assert!(json["issue_types"].as_array().is_some_and(|issues| {
        issues
            .iter()
            .any(|issue| issue == "widget-top-level-function-boundary")
    }));
    assert!(
        json["issue_types"]
            .as_array()
            .is_some_and(|issues| issues.iter().any(|issue| issue == "boundary-coverage"))
    );
    assert!(json["issue_types"].as_array().is_some_and(|issues| {
        issues
            .iter()
            .any(|issue| issue == "boundary-call-violation")
    }));
    assert!(
        json["issue_types"]
            .as_array()
            .is_some_and(|issues| issues.iter().any(|issue| issue == "policy-violation"))
    );
    assert!(json["issue_types"].as_array().is_some_and(|issues| {
        issues
            .iter()
            .any(|issue| issue == "missing-suppression-reason")
    }));
    assert!(json["task_matrix"].as_array().is_some_and(|tasks| {
        tasks.iter().any(|task| {
            task["intent"] == "trace a top-level symbol"
                && task["command"]
                    .as_str()
                    .is_some_and(|command| command.contains("dart-decimate inspect"))
        })
    }));

    Ok(())
}

fn assert_manifest_identity(json: &Value) {
    assert_eq!(json["schema_version"], "dart-decimate.schema.v1");
    assert_eq!(json["kind"], "schema");
    assert_eq!(json["name"], "Dart Decimate");
    assert_eq!(json["default_command"], "check");
    assert!(json["plugins"].as_array().is_some_and(|plugins| {
        plugins
            .iter()
            .any(|plugin| plugin["name"] == "flutter" && plugin["kind"] == "framework")
    }));
    assert!(json["environment_variables"].as_array().is_some_and(|env| {
        env.iter()
            .any(|variable| variable["name"] == "DART_DECIMATE_BASE")
    }));
    assert_eq!(json["mcp_tools"]["server"], "dart-decimate-mcp");
    assert!(json["mcp_tools"]["tools"].as_array().is_some_and(|tools| {
        tools.iter().any(|tool| {
            tool["name"] == "code_execute"
                && tool["read_only"] == true
                && tool["schema"] == "dart-decimate.mcp.code_execute.v1"
        })
    }));
    assert_mcp_tool_key(json, "code_execute", "code");
    assert!(json["mcp_tools"]["tools"].as_array().is_some_and(|tools| {
        tools.iter().any(|tool| {
            tool["name"] == "analyze"
                && tool["read_only"] == true
                && tool["command"] == "dart-decimate check --format json"
        })
    }));
    assert!(json["mcp_tools"]["tools"].as_array().is_some_and(|tools| {
        tools.iter().any(|tool| {
            tool["name"] == "dart_decimate_explain" && tool["schema"] == "dart-decimate.explain.v1"
        })
    }));
    assert!(json["mcp_tools"]["tools"].as_array().is_some_and(|tools| {
        tools.iter().any(|tool| {
            tool["name"] == "fallow_explain"
                && tool["command"] == "dart-decimate explain --format json"
                && tool["schema"] == "dart-decimate.explain.v1"
        })
    }));
    assert_mcp_tool_key(json, "analyze", "changed_workspaces");
    assert_mcp_tool_key(json, "analyze", "private_type_leaks");
    assert_mcp_tool_key(json, "check_changed", "since");
    assert_mcp_tool_key(json, "list_boundaries", "workspace");
    assert_mcp_tool_key(json, "inspect_target", "production");
    assert_mcp_tool_key(json, "trace_clone", "file");
    assert_mcp_tool_key(json, "trace_clone", "min_tokens");
    assert_mcp_tool_key(json, "trace_clone", "threshold");
    assert_mcp_tool_key(json, "find_dupes", "cross_language");
    assert_mcp_tool_key(json, "check_health", "min_score");
    assert_mcp_tool_key(json, "check_runtime_coverage", "coverage");
    assert_mcp_tool_key(json, "get_hot_paths", "coverage");
    assert_mcp_tool_key(json, "impact", "root");
    assert_mcp_tool_key(json, "impact_all", "limit");
    assert_mcp_tool_key(json, "security_candidates", "gate");
    assert_mcp_tool_key(json, "security_candidates", "paths");
    assert_mcp_tool_key(json, "fallow_explain", "rule_id");
    assert_mcp_tool_key(json, "fix_preview", "action");
    assert_mcp_tool_key(json, "fix_apply", "yes");
    assert_mcp_tool_key(json, "audit", "gate");
    assert!(json["mcp_tools"]["tools"].as_array().is_some_and(|tools| {
        tools
            .iter()
            .any(|tool| tool["name"] == "fix_apply" && tool["read_only"] == false)
    }));
    assert_mcp_tool_key(json, "audit", "dead_code_baseline");
}

fn assert_mcp_tool_key(json: &Value, tool_name: &str, key: &str) {
    assert!(json["mcp_tools"]["tools"].as_array().is_some_and(|tools| {
        tools.iter().any(|tool| {
            tool["name"] == tool_name
                && tool["key_params"]
                    .as_array()
                    .is_some_and(|params| params.iter().any(|param| param == key))
        })
    }));
}

fn assert_manifest_metadata(json: &Value) {
    assert_eq!(json["manifest_version"], "dart-decimate.schema.v1");
    assert_eq!(
        json["output_formats"],
        serde_json::json!(["human", "html", "json", "sarif"])
    );
    assert!(json["global_flags"].as_array().is_some_and(|flags| {
        ["--root", "--format", "--config", "--quiet"]
            .iter()
            .all(|expected| flags.iter().any(|flag| flag == expected))
    }));
    assert!(
        json["exit_codes"]
            .as_array()
            .is_some_and(|codes| { codes.iter().any(|code| code["code"] == 2) })
    );
    assert!(
        json["exit_codes"]
            .as_array()
            .is_some_and(|codes| { codes.iter().any(|code| code["code"] == 3) })
    );
    assert_eq!(
        json["severity_levels"],
        serde_json::json!(["error", "warning"])
    );
}

#[test]
fn quiet_flag_is_accepted_by_report_commands() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(
        [
            "dart-decimate",
            "check",
            "--format",
            "json",
            "--quiet",
            "--root",
            ".",
        ],
        &mut output,
    )?;

    assert!(matches!(code, 0 | 1));
    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(json["schema_version"], "dart-decimate.report.v1");
    assert_eq!(json["command"], "check");

    Ok(())
}

#[test]
fn schema_command_lists_coverage_workflow_commands() -> Result<(), Box<dyn std::error::Error>> {
    let json = schema_json()?;

    assert!(has_command_with_flags(
        &json,
        "coverage analyze",
        "runtime-coverage",
        &["--runtime-coverage", "--cloud", "--repo"],
    ));
    assert!(has_command_with_flags(
        &json,
        "coverage setup",
        "coverage-setup",
        &["--yes", "--non-interactive"],
    ));
    assert!(has_command_with_flags(
        &json,
        "coverage upload-source-maps",
        "coverage-upload-source-maps",
        &["--dir", "--git-sha", "--dry-run"],
    ));
    assert!(has_command_with_flags(
        &json,
        "coverage upload-inventory",
        "coverage-upload-inventory",
        &["--dry-run"],
    ));

    Ok(())
}

#[test]
fn schema_command_lists_actual_cli_flags() -> Result<(), Box<dyn std::error::Error>> {
    let json = schema_json()?;

    assert_manifest_flags(
        &json,
        "check",
        &["--root", "--baseline", "--max-crap", "--min-occurrences"],
    );
    assert_manifest_flags(
        &json,
        "audit",
        &[
            "--gate",
            "--dead-code-baseline",
            "--no-production",
            "--complexity-breakdown",
        ],
    );
    assert_manifest_flags(
        &json,
        "dead-code",
        &["--changed-workspaces", "--save-baseline"],
    );
    assert_manifest_flags(
        &json,
        "dupes",
        &["--min-occurrences", "--threshold", "--cross-language"],
    );
    assert_manifest_flags(&json, "health", &["--max-crap", "--min-score", "--top"]);
    assert_manifest_flags(
        &json,
        "trace-clone",
        &["--min-occurrences", "--fingerprint"],
    );
    assert_manifest_flags(&json, "trace", &["--root", "--format"]);
    assert_manifest_flags(&json, "config", &["--root", "--format", "--path"]);
    assert_manifest_flags(
        &json,
        "coverage upload-source-maps",
        &["--repo", "--git-sha", "--strip-path", "--dry-run"],
    );
    assert_manifest_flags(&json, "list", &["--files", "--entry-points", "--plugins"]);
    assert_manifest_flags(
        &json,
        "fix",
        &["--config", "--workspace", "--changed-workspaces"],
    );
    assert_manifest_flags(&json, "migrate", &["--dry-run"]);
    assert_manifest_flags(&json, "telemetry", &["status", "enable", "disable"]);
    assert_manifest_flags(&json, "license", &["status", "activate"]);
    assert_manifest_flags(&json, "init", &["--format", "--agents", "--force"]);
    assert_manifest_flags(&json, "hooks", &["install", "--target", "--branch"]);
    assert_manifest_flags(
        &json,
        "setup-hooks",
        &["--agent", "--dry-run", "--uninstall"],
    );
    assert_manifest_flags(&json, "watch", &["--no-clear", "--once", "--interval-ms"]);
    assert_manifest_flags(
        &json,
        "ci",
        &["reconcile-review", "--provider", "--envelope", "--dry-run"],
    );
    assert_manifest_omits_flags(&json, "list", &["--section"]);

    Ok(())
}

fn schema_json() -> Result<Value, Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(["dart-decimate", "schema", "--format", "json"], &mut output)?;
    assert_eq!(code, 0);
    Ok(serde_json::from_slice::<Value>(&output)?)
}

fn assert_manifest_flags(json: &Value, command_name: &str, expected: &[&str]) {
    let command = manifest_command(json, command_name);
    for flag in expected {
        assert!(
            command["flags"]
                .as_array()
                .is_some_and(|flags| flags.iter().any(|candidate| candidate == flag)),
            "{command_name} missing {flag}"
        );
    }
}

fn assert_manifest_omits_flags(json: &Value, command_name: &str, unexpected: &[&str]) {
    let command = manifest_command(json, command_name);
    for flag in unexpected {
        assert!(
            command["flags"]
                .as_array()
                .is_none_or(|flags| flags.iter().all(|candidate| candidate != flag)),
            "{command_name} unexpectedly lists {flag}"
        );
    }
}

fn has_command_with_flags(json: &Value, name: &str, kind: &str, expected_flags: &[&str]) -> bool {
    json["commands"].as_array().is_some_and(|commands| {
        commands.iter().any(|command| {
            command["name"] == name
                && command["kind"] == kind
                && command["schema"] == "dart-decimate.coverage.v1"
                && command["flags"].as_array().is_some_and(|flags| {
                    expected_flags
                        .iter()
                        .all(|expected| flags.iter().any(|flag| flag == expected))
                })
        })
    })
}

fn manifest_command<'a>(json: &'a Value, command_name: &str) -> &'a Value {
    json["commands"]
        .as_array()
        .and_then(|commands| {
            commands
                .iter()
                .find(|command| command["name"] == command_name)
        })
        .unwrap_or_else(|| panic!("missing manifest command {command_name}"))
}
