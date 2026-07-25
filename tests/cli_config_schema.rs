use dart_decimate::cli::run_from;
use serde_json::Value;

#[test]
fn config_schema_command_emits_json_schema() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        ["dart-decimate", "config-schema", "--format", "json"],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "dart-decimate.config.v1");
    assert_eq!(json["properties"]["production"]["type"], "boolean");
    assert_eq!(json["properties"]["includeEntryExports"]["type"], "boolean");
    assert_eq!(json["properties"]["boundaryCoverage"]["type"], "boolean");
    assert_boundary_config_schema(&json);
    assert_eq!(json["properties"]["boundaryCalls"]["type"], "array");
    assert_eq!(json["properties"]["rulePacks"]["type"], "array");
    assert_eq!(
        json["properties"]["cli"]["properties"]["production"]["type"],
        "boolean"
    );
    assert_eq!(
        json["properties"]["cli"]["properties"]["includeEntryExports"]["type"],
        "boolean"
    );
    assert_eq!(
        json["properties"]["cli"]["properties"]["boundaries"]["oneOf"][1]["type"],
        "object"
    );
    assert_eq!(
        json["properties"]["cli"]["properties"]["boundaryCalls"]["type"],
        "array"
    );
    assert_eq!(
        json["properties"]["cli"]["properties"]["rulePacks"]["type"],
        "array"
    );
    assert_eq!(json["properties"]["health"]["type"], "object");
    assert_eq!(
        json["properties"]["health"]["properties"]["coverage_gaps"]["type"],
        "boolean"
    );
    assert_eq!(
        json["properties"]["health"]["properties"]["fileScores"]["type"],
        "boolean"
    );
    assert_eq!(
        json["properties"]["health"]["properties"]["max_crap"]["minimum"],
        1
    );
    assert_eq!(
        json["properties"]["health"]["properties"]["runtime_coverage"]["type"],
        "string"
    );
    assert_eq!(
        json["properties"]["health"]["properties"]["lowTrafficThreshold"]["maximum"],
        1
    );
    assert_eq!(
        json["properties"]["health"]["properties"]["minScore"]["maximum"],
        100
    );
    assert_eq!(
        json["properties"]["dupes"]["properties"]["mode"]["enum"][3],
        "semantic"
    );
    assert_eq!(
        json["properties"]["dupes"]["properties"]["threshold"]["maximum"],
        100
    );
    assert_eq!(
        json["properties"]["dupes"]["properties"]["ignore_mapper_pairs"]["type"],
        "boolean"
    );
    assert_eq!(
        json["properties"]["dupes"]["properties"]["ignoreMapperPairs"]["type"],
        "boolean"
    );
    assert_eq!(
        json["properties"]["flags"]["properties"]["allow"]["items"]["type"],
        "string"
    );
    assert_eq!(
        json["properties"]["featureFlags"]["properties"]["allow"]["items"]["type"],
        "string"
    );
    assert_eq!(
        json["properties"]["ignoreDependencies"]["items"]["type"],
        "string"
    );
    assert_eq!(
        json["properties"]["ignoreDependencyOverrides"]["items"]["required"][0],
        "package"
    );
    assert_eq!(
        json["properties"]["ignoreDependencyOverrides"]["items"]["properties"]["source"]["type"][1],
        "null"
    );
    assert_eq!(
        json["properties"]["security"]["properties"]["categories"]["items"]["enum"][0],
        "hardcoded-secret"
    );

    Ok(())
}

fn assert_boundary_config_schema(json: &Value) {
    let boundary_object = &json["properties"]["boundaries"]["oneOf"][1];
    assert_eq!(boundary_object["type"], "object");
    for preset in ["layered", "hexagonal", "feature-sliced", "bulletproof"] {
        assert_array_contains(&boundary_object["properties"]["preset"]["enum"], preset);
    }
    assert_eq!(
        boundary_object["properties"]["coverage"]["properties"]["requireAllFiles"]["type"],
        "boolean"
    );
    assert_eq!(
        boundary_object["properties"]["coverage"]["properties"]["allowUnmatched"]["items"]["type"],
        "string"
    );
}

#[test]
fn report_schema_command_emits_json_schema() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        ["dart-decimate", "report-schema", "--format", "json"],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_report_schema_envelope(&json);
    assert_report_schema_finding_kinds(&json);
    assert_report_schema_summary_fields(&json);
    assert_report_schema_action_contract(&json);

    Ok(())
}

fn assert_report_schema_envelope(json: &Value) {
    assert_eq!(json["schema_version"], "dart-decimate.report.v1");
    assert_eq!(json["properties"]["kind"]["type"], "string");
    assert_eq!(json["properties"]["findings"]["type"], "array");
    assert_eq!(json["properties"]["file_scores"]["type"], "array");
    assert_eq!(json["properties"]["hotspots"]["type"], "array");
    assert_eq!(json["properties"]["refactoring_targets"]["type"], "array");
    assert_eq!(
        json["properties"]["runtime_coverage"]["$ref"],
        "#/$defs/runtime_coverage"
    );
    assert!(
        json["properties"]["command"]["enum"]
            .as_array()
            .is_some_and(|commands| commands.iter().any(|command| command == "audit"))
    );
    assert!(
        json["properties"]["command"]["enum"]
            .as_array()
            .is_some_and(|commands| commands.iter().all(|command| command != "trace-file"))
    );
}

fn assert_report_schema_finding_kinds(json: &Value) {
    for kind in [
        "security-candidate",
        "unused-type",
        "private-type-leak",
        "boundary-coverage",
        "boundary-call-violation",
        "policy-violation",
        "part-of-violation",
        "missing-suppression-reason",
        "unused-enum-member",
        "unused-class-member",
        "coverage-gap",
        "unused-dev-dependency",
        "test-only-dependency",
        "unused-dependency-override",
        "misconfigured-dependency-override",
        "high-crap-score",
        "health-hotspot",
        "refactoring-target",
    ] {
        assert_array_contains(
            &json["$defs"]["finding"]["properties"]["kind"]["enum"],
            kind,
        );
    }
}

fn assert_report_schema_summary_fields(json: &Value) {
    for field in [
        "unused_types",
        "private_type_leaks",
        "boundary_coverage",
        "boundary_call_violations",
        "policy_violations",
        "missing_suppression_reasons",
        "unused_dev_dependencies",
        "test_only_dependencies",
        "dependency_overrides",
        "unused_dependency_overrides",
        "misconfigured_dependency_overrides",
        "unused_class_members",
        "coverage_gaps",
        "crap_functions",
        "file_scores",
        "hotspots",
        "refactoring_targets",
    ] {
        assert_array_contains(&json["$defs"]["summary"]["required"], field);
    }
}

fn assert_array_contains(array: &Value, expected: &str) {
    assert!(
        array
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item == expected)),
        "expected array to contain {expected}"
    );
}

fn assert_report_schema_action_contract(json: &Value) {
    assert!(
        json["$defs"]["finding_action"]["required"]
            .as_array()
            .is_some_and(|fields| fields.iter().any(|field| field == "type"))
    );
    assert_eq!(
        json["$defs"]["finding_action"]["properties"]["command"]["type"],
        "string"
    );
    assert_eq!(
        json["$defs"]["finding_action"]["properties"]["argv"]["type"],
        "array"
    );
    assert_eq!(
        json["$defs"]["finding_action"]["properties"]["target_dependency"]["type"],
        "string"
    );
    assert_eq!(
        json["$defs"]["finding_action"]["properties"]["suppression_comment"]["type"],
        "string"
    );
}
