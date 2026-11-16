use dart_decimate::cli::run_from;
use serde_json::Value;

#[test]
fn explain_command_emits_json_contract() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "explain",
            "unused-export",
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "dart-decimate.explain.v1");
    assert_eq!(json["kind"], "explain");
    assert_eq!(json["id"], "dart-decimate/unused-export");
    assert_eq!(json["issue_type"], "unused-export");
    assert_eq!(json["name"], "Unused export");
    assert!(
        json["summary"]
            .as_str()
            .is_some_and(|text| text.contains("public"))
    );
    assert!(
        json["related_commands"]
            .as_array()
            .is_some_and(|commands| commands.iter().any(|command| command
                .as_str()
                .is_some_and(|text| text.contains("trace-symbol"))))
    );

    Ok(())
}

#[test]
fn explain_command_accepts_fallow_and_spaced_aliases() -> Result<(), Box<dyn std::error::Error>> {
    let mut spaced = Vec::new();
    let spaced_code = run_from(
        [
            "dart-decimate",
            "explain",
            "unused exports",
            "--format",
            "json",
        ],
        &mut spaced,
    )?;
    let mut fallow = Vec::new();
    let fallow_code = run_from(
        [
            "dart-decimate",
            "explain",
            "fallow/code-duplication",
            "--format",
            "json",
        ],
        &mut fallow,
    )?;

    let spaced_json = serde_json::from_slice::<Value>(&spaced)?;
    let fallow_json = serde_json::from_slice::<Value>(&fallow)?;
    assert_eq!(spaced_code, 0);
    assert_eq!(spaced_json["id"], "dart-decimate/unused-export");
    assert_eq!(fallow_code, 0);
    assert_eq!(fallow_json["id"], "dart-decimate/code-duplication");

    Ok(())
}

#[test]
fn explain_command_emits_unused_type_contract() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "explain",
            "unused types",
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["id"], "dart-decimate/unused-type");
    assert_eq!(json["issue_type"], "unused-type");
    assert_eq!(json["name"], "Unused type");
    assert!(json["suppressions"].as_array().is_some_and(|comments| {
        comments
            .iter()
            .any(|comment| comment == "// dart-decimate-ignore-next-line unused-type")
    }));

    Ok(())
}

#[test]
fn unused_class_member_explanation_states_its_limits() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "explain",
            "unused-class-member",
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let text = json.to_string();
    assert_eq!(code, 0);
    assert!(text.contains("reads the SDK and package sources"), "{text}");
    assert!(text.contains("outside_types_unread"), "{text}");
    assert!(text.contains("pub get"), "{text}");
    assert!(
        !text.contains("does not read SDK or package sources"),
        "{text}"
    );
    assert!(!text.contains("protected as a whole everywhere"), "{text}");

    Ok(())
}

#[test]
fn explain_command_emits_unused_widget_param_contract() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "explain",
            "unused-component-prop",
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["id"], "dart-decimate/unused-widget-param");
    assert_eq!(json["issue_type"], "unused-widget-param");
    assert_eq!(json["name"], "Unused widget parameter");
    assert!(json["suppressions"].as_array().is_some_and(|comments| {
        comments
            .iter()
            .any(|comment| comment == "// dart-decimate-ignore-next-line unused-widget-param")
    }));

    Ok(())
}

#[test]
fn explain_command_emits_private_widget_class_contract() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "explain",
            "flutter-private-widget-class",
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["id"], "dart-decimate/private-widget-class");
    assert_eq!(json["issue_type"], "private-widget-class");
    assert_eq!(json["name"], "Private widget class");
    assert!(json["suppressions"].as_array().is_some_and(|comments| {
        comments
            .iter()
            .any(|comment| comment == "// dart-decimate-ignore-next-line private-widget-class")
    }));

    Ok(())
}

#[test]
fn explain_command_emits_missing_context_mounted_contract() -> Result<(), Box<dyn std::error::Error>>
{
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "explain",
            "missing-context-mounted-after-await",
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(
        json["id"],
        "dart-decimate/missing-context-mounted-after-await"
    );
    assert_eq!(json["issue_type"], "missing-context-mounted-after-await");
    assert_eq!(json["name"], "Missing context.mounted guard");
    assert!(json["suppressions"].as_array().is_some_and(|comments| {
        comments.iter().any(|comment| {
            comment == "// dart-decimate-ignore-next-line missing-context-mounted-after-await"
        })
    }));

    Ok(())
}

#[test]
fn explain_command_emits_widget_top_level_function_contract()
-> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "explain",
            "top-level-widget-helper",
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(
        json["id"],
        "dart-decimate/widget-top-level-function-boundary"
    );
    assert_eq!(json["issue_type"], "widget-top-level-function-boundary");
    assert_eq!(json["name"], "Widget top-level function boundary");
    assert!(json["suppressions"].as_array().is_some_and(|comments| {
        comments.iter().any(|comment| {
            comment == "// dart-decimate-ignore-next-line widget-top-level-function-boundary"
        })
    }));

    Ok(())
}

#[test]
fn explain_command_emits_private_type_leak_contract() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "explain",
            "private-type-leaks",
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["id"], "dart-decimate/private-type-leak");
    assert_eq!(json["issue_type"], "private-type-leak");
    assert_eq!(json["name"], "Private type leak");
    assert!(json["suppressions"].as_array().is_some_and(|comments| {
        comments
            .iter()
            .any(|comment| comment == "// dart-decimate-ignore-next-line private-type-leak")
    }));

    Ok(())
}

#[test]
fn explain_command_emits_boundary_coverage_contract() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "explain",
            "boundary-coverage",
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["id"], "dart-decimate/boundary-coverage");
    assert_eq!(json["issue_type"], "boundary-coverage");
    assert!(json["related_commands"].as_array().is_some_and(|commands| {
        commands.iter().any(|command| {
            command
                .as_str()
                .is_some_and(|text| text.contains("list --boundaries"))
        })
    }));

    Ok(())
}

#[test]
fn explain_command_emits_part_of_violation_contract() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "explain",
            "invalid-part-of",
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["id"], "dart-decimate/part-of-violation");
    assert_eq!(json["issue_type"], "part-of-violation");
    assert_eq!(json["name"], "Part of violation");

    Ok(())
}

#[test]
fn explain_command_emits_policy_and_boundary_call_contracts()
-> Result<(), Box<dyn std::error::Error>> {
    let mut boundary_call = Vec::new();
    let boundary_code = run_from(
        [
            "dart-decimate",
            "explain",
            "boundary-call-violation",
            "--format",
            "json",
        ],
        &mut boundary_call,
    )?;
    let boundary = serde_json::from_slice::<Value>(&boundary_call)?;
    assert_eq!(boundary_code, 0);
    assert_eq!(boundary["issue_type"], "boundary-call-violation");
    assert_eq!(boundary["id"], "dart-decimate/boundary-violation");

    let mut policy = Vec::new();
    let policy_code = run_from(
        [
            "dart-decimate",
            "explain",
            "policy-violation",
            "--format",
            "json",
        ],
        &mut policy,
    )?;
    let policy = serde_json::from_slice::<Value>(&policy)?;
    assert_eq!(policy_code, 0);
    assert_eq!(policy["issue_type"], "policy-violation");
    assert_eq!(policy["id"], "dart-decimate/policy-violation");

    Ok(())
}

#[test]
fn explain_command_emits_missing_suppression_reason_contract()
-> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "explain",
            "missing-suppression-reason",
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["id"], "dart-decimate/missing-suppression-reason");
    assert_eq!(json["issue_type"], "missing-suppression-reason");

    Ok(())
}

#[test]
fn explain_command_emits_typed_dependency_contracts() -> Result<(), Box<dyn std::error::Error>> {
    let mut unused_dev = Vec::new();
    let unused_dev_code = run_from(
        [
            "dart-decimate",
            "explain",
            "unused-dev-dependency",
            "--format",
            "json",
        ],
        &mut unused_dev,
    )?;
    let mut override_output = Vec::new();
    let override_code = run_from(
        [
            "dart-decimate",
            "explain",
            "unused-dependency-overrides",
            "--format",
            "json",
        ],
        &mut override_output,
    )?;

    let unused_dev_json = serde_json::from_slice::<Value>(&unused_dev)?;
    let override_json = serde_json::from_slice::<Value>(&override_output)?;
    assert_eq!(unused_dev_code, 0);
    assert_eq!(unused_dev_json["id"], "dart-decimate/unused-dev-dependency");
    assert_eq!(unused_dev_json["issue_type"], "unused-dev-dependency");
    assert_eq!(override_code, 0);
    assert_eq!(
        override_json["id"],
        "dart-decimate/unused-dependency-override"
    );
    assert_eq!(override_json["issue_type"], "unused-dependency-override");

    Ok(())
}

#[test]
fn explain_command_emits_private_src_import_contract() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "explain",
            "private-src-imports",
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["id"], "dart-decimate/private-src-import");
    assert_eq!(json["issue_type"], "private-src-import");
    assert_eq!(json["name"], "Private src import");
    assert!(json["suppressions"].as_array().is_some_and(|comments| {
        comments
            .iter()
            .any(|comment| comment == "// dart-decimate-ignore-next-line private-src-import")
    }));
    assert!(json["related_commands"].as_array().is_some_and(|commands| {
        commands.iter().any(|command| {
            command
                .as_str()
                .is_some_and(|text| text.contains("--private-src-imports"))
        })
    }));

    Ok(())
}

#[test]
fn explain_command_renders_human_contract() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        ["dart-decimate", "explain", "dart-decimate/unused-export"],
        &mut output,
    )?;

    let rendered = String::from_utf8(output)?;
    assert_eq!(code, 0);
    assert!(rendered.contains("Unused export"));
    assert!(rendered.contains("dart-decimate/unused-export"));
    assert!(rendered.contains("Why it matters"));
    assert!(rendered.contains("How to fix"));

    Ok(())
}

#[test]
fn explain_command_reports_unknown_issue_as_json_error() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "explain",
            "not-a-real-issue",
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 2);
    assert_eq!(json["error"], true);
    assert_eq!(json["exit_code"], 2);
    assert!(
        json["message"]
            .as_str()
            .is_some_and(|message| message.contains("not-a-real-issue"))
    );

    Ok(())
}
