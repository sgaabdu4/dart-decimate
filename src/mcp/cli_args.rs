use serde_json::{Map, Value};

mod value_args;

use value_args::{
    array_strings, bool_arg, issue_filter_flag, push_bool_flag, push_bool_mode, push_float_flag,
    push_noop_bool, push_number_flag, push_required_string, push_string_flag, push_string_flags,
    string_arg,
};

const GLOBAL_KEYS: &[&str] = &["root", "config"];
const REPORT_SCOPE_KEYS: &[&str] = &[
    "entry",
    "dart_platform",
    "file",
    "workspace",
    "changed_workspaces",
    "changed_since",
    "compare",
    "production",
];
const LIST_SCOPE_KEYS: &[&str] = &[
    "entry",
    "dart_platform",
    "file",
    "workspace",
    "changed_workspaces",
    "production",
];
const BASELINE_KEYS: &[&str] = &[
    "baseline",
    "regression_baseline",
    "fail_on_regression",
    "tolerance",
];
const SYMBOL_KEYS: &[&str] = &["include_entry_exports", "private_type_leaks"];
const BOUNDARY_KEYS: &[&str] = &[
    "boundary",
    "boundary_coverage",
    "boundary_call",
    "policy_pack",
    "policy_violations",
];
const DUPLICATE_KEYS: &[&str] = &[
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
];
const HEALTH_KEYS: &[&str] = &[
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
];
const FIX_KEYS: &[&str] = &["action", "no_create_config"];

pub(super) fn cli_args_for_tool(
    name: &str,
    args: &Map<String, Value>,
) -> Result<Vec<String>, String> {
    reject_unknown_args(name, args)?;
    match name {
        "analyze" => report_args("check", args, analyze_args),
        "check_changed" => report_args("check", args, check_changed_args),
        "project_info" => report_args("list", args, project_info_args),
        "list_boundaries" => report_args("list", args, list_boundaries_args),
        "inspect_target" => report_args("inspect", args, inspect_args),
        "trace_file" => report_args("trace-file", args, trace_file_args),
        "trace_export" => report_args("trace-symbol", args, trace_export_args),
        "trace_dependency" => report_args("trace-dependency", args, trace_dependency_args),
        "trace_clone" => report_args("trace-clone", args, trace_clone_args),
        "find_dupes" => report_args("dupes", args, dupes_args),
        "check_health" => report_args("health", args, health_args),
        "check_runtime_coverage"
        | "get_hot_paths"
        | "get_blast_radius"
        | "get_importance"
        | "get_cleanup_candidates" => coverage_analyze_args(args),
        "security_candidates" => report_args("security", args, security_args),
        "feature_flags" => report_args("flags", args, flags_args),
        "impact" => impact_args(args),
        "impact_all" => impact_all_args(args),
        "fix_preview" => fix_preview_args(args),
        "fix_apply" => fix_apply_args(args),
        "audit" => report_args("audit", args, audit_args),
        "decision_surface" => report_args("decision-surface", args, decision_surface_args),
        "dart_decimate_explain" | "fallow_explain" => explain_args(name, args),
        _ => Err(format!("unknown tool {name}")),
    }
}

fn reject_unknown_args(name: &str, args: &Map<String, Value>) -> Result<(), String> {
    let allowed = allowed_args(name)?;
    for key in args.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("{name} does not accept argument {key}"));
        }
    }
    Ok(())
}

fn allowed_args(name: &str) -> Result<Vec<&'static str>, String> {
    let mut allowed = GLOBAL_KEYS.to_vec();
    match name {
        "analyze" => extend_analyze_allowed(&mut allowed),
        "check_changed" => extend_check_changed_allowed(&mut allowed),
        "project_info" => extend_project_info_allowed(&mut allowed),
        "list_boundaries" => allowed.extend(LIST_SCOPE_KEYS),
        "inspect_target" => allowed.extend(["target", "file", "symbol", "production"]),
        "trace_file" => allowed.extend(["file"]),
        "trace_export" => allowed.extend(["file", "symbol", "export_name"]),
        "trace_dependency" => allowed.extend(["dependency", "package_name"]),
        "trace_clone" => extend_trace_clone_allowed(&mut allowed),
        "find_dupes" => extend_dupes_allowed(&mut allowed),
        "check_health" => extend_health_allowed(&mut allowed),
        "check_runtime_coverage"
        | "get_hot_paths"
        | "get_blast_radius"
        | "get_importance"
        | "get_cleanup_candidates" => extend_runtime_allowed(&mut allowed),
        "security_candidates" => extend_security_allowed(&mut allowed),
        "feature_flags" => extend_flags_allowed(&mut allowed),
        "impact" => return Ok(vec!["root"]),
        "impact_all" => return Ok(vec!["sort", "limit"]),
        "fix_preview" => extend_fix_allowed(&mut allowed),
        "fix_apply" => extend_fix_apply_allowed(&mut allowed),
        "audit" => extend_audit_allowed(&mut allowed),
        "decision_surface" => allowed.extend(["base", "max_decisions"]),
        "dart_decimate_explain" | "fallow_explain" => return Ok(vec!["issue_type", "rule_id"]),
        _ => return Err(format!("unknown tool {name}")),
    }
    allowed.sort_unstable();
    allowed.dedup();
    Ok(allowed)
}

fn extend_analyze_allowed(allowed: &mut Vec<&'static str>) {
    allowed.extend(["issue_types"]);
    allowed.extend(REPORT_SCOPE_KEYS);
    allowed.extend(BASELINE_KEYS);
    allowed.extend(SYMBOL_KEYS);
    allowed.extend(BOUNDARY_KEYS);
    allowed.extend(DUPLICATE_KEYS);
    allowed.extend(HEALTH_KEYS);
}

fn extend_check_changed_allowed(allowed: &mut Vec<&'static str>) {
    allowed.extend(["since", "changed_since", "compare"]);
    allowed.extend(BASELINE_KEYS);
    allowed.extend(["dart_platform", "production"]);
}

fn extend_project_info_allowed(allowed: &mut Vec<&'static str>) {
    allowed.extend([
        "files",
        "entry_points",
        "plugins",
        "boundaries",
        "workspaces",
    ]);
    allowed.extend(LIST_SCOPE_KEYS);
}

fn extend_dupes_allowed(allowed: &mut Vec<&'static str>) {
    allowed.extend(REPORT_SCOPE_KEYS);
    allowed.extend(BASELINE_KEYS);
    allowed.extend(DUPLICATE_KEYS);
}

fn extend_trace_clone_allowed(allowed: &mut Vec<&'static str>) {
    allowed.extend(["fingerprint", "file", "line"]);
    allowed.extend(DUPLICATE_KEYS);
}

fn extend_health_allowed(allowed: &mut Vec<&'static str>) {
    allowed.extend(REPORT_SCOPE_KEYS);
    allowed.extend(BASELINE_KEYS);
    allowed.extend(HEALTH_KEYS);
}

fn extend_runtime_allowed(allowed: &mut Vec<&'static str>) {
    allowed.extend([
        "coverage",
        "min_invocations_hot",
        "min_observation_volume",
        "low_traffic_threshold",
        "top",
        "repo",
    ]);
}

fn extend_security_allowed(allowed: &mut Vec<&'static str>) {
    allowed.extend(REPORT_SCOPE_KEYS);
    allowed.extend(BASELINE_KEYS);
    allowed.extend([
        "top",
        "surface",
        "gate",
        "paths",
        "diff_file",
        "ci",
        "fail_on_issues",
        "summary",
    ]);
}

fn extend_flags_allowed(allowed: &mut Vec<&'static str>) {
    allowed.extend(REPORT_SCOPE_KEYS);
    allowed.extend(BASELINE_KEYS);
    allowed.extend(["top"]);
}

fn extend_fix_allowed(allowed: &mut Vec<&'static str>) {
    allowed.extend(REPORT_SCOPE_KEYS);
    allowed.extend(FIX_KEYS);
}

fn extend_fix_apply_allowed(allowed: &mut Vec<&'static str>) {
    extend_fix_allowed(allowed);
    allowed.extend(["yes"]);
}

fn extend_audit_allowed(allowed: &mut Vec<&'static str>) {
    allowed.extend(REPORT_SCOPE_KEYS);
    allowed.extend(SYMBOL_KEYS);
    allowed.extend(BOUNDARY_KEYS);
    allowed.extend(DUPLICATE_KEYS);
    allowed.extend(HEALTH_KEYS);
    allowed.extend([
        "base",
        "gate",
        "brief",
        "dead_code_baseline",
        "health_baseline",
        "dupes_baseline",
        "max_decisions",
    ]);
}

fn report_args<F>(
    command: &str,
    args: &Map<String, Value>,
    append: F,
) -> Result<Vec<String>, String>
where
    F: FnOnce(&mut Vec<String>, &Map<String, Value>) -> Result<(), String>,
{
    let mut cli = json_command_args(&[command]);
    push_string_flag(&mut cli, args, "root", "--root")?;
    push_string_flag(&mut cli, args, "config", "--config")?;
    append(&mut cli, args)?;
    Ok(cli)
}

fn json_command_args(command: &[&str]) -> Vec<String> {
    let mut cli = vec!["dart-decimate".to_owned()];
    cli.extend(command.iter().map(|part| (*part).to_owned()));
    cli.extend(["--format".to_owned(), "json".to_owned()]);
    cli
}

fn analyze_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_report_scope_args(cli, args)?;
    push_baseline_args(cli, args)?;
    push_symbol_args(cli, args)?;
    push_boundary_args(cli, args)?;
    push_duplicate_args(cli, args)?;
    push_health_args(cli, args)?;
    if let Some(issue_types) = args.get("issue_types") {
        for issue_type in array_strings(issue_types, "issue_types")? {
            cli.push(issue_filter_flag(issue_type)?);
        }
    }
    Ok(())
}

fn check_changed_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    let since = string_arg(args, "since")?
        .or(string_arg(args, "changed_since")?)
        .or(string_arg(args, "compare")?)
        .ok_or_else(|| "check_changed requires since".to_owned())?;
    cli.extend(["--changed-since".to_owned(), since]);
    push_baseline_args(cli, args)?;
    push_string_flag(cli, args, "dart_platform", "--dart-platform")?;
    push_bool_mode(cli, args, "production", "--production", "--no-production")
}

fn project_info_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_string_flags(cli, args, "entry", "--entry")?;
    push_string_flag(cli, args, "dart_platform", "--dart-platform")?;
    push_string_flags(cli, args, "file", "--file")?;
    push_string_flags(cli, args, "workspace", "--workspace")?;
    push_string_flag(cli, args, "changed_workspaces", "--changed-workspaces")?;
    push_bool_mode(cli, args, "production", "--production", "--no-production")?;
    for (key, flag) in [
        ("files", "--files"),
        ("entry_points", "--entry-points"),
        ("plugins", "--plugins"),
        ("boundaries", "--boundaries"),
        ("workspaces", "--workspaces"),
    ] {
        push_bool_flag(cli, args, key, flag)?;
    }
    Ok(())
}

fn list_boundaries_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_string_flags(cli, args, "entry", "--entry")?;
    push_string_flag(cli, args, "dart_platform", "--dart-platform")?;
    push_string_flags(cli, args, "file", "--file")?;
    push_string_flags(cli, args, "workspace", "--workspace")?;
    push_string_flag(cli, args, "changed_workspaces", "--changed-workspaces")?;
    push_bool_mode(cli, args, "production", "--production", "--no-production")?;
    cli.push("--boundaries".to_owned());
    Ok(())
}

fn inspect_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    if let Some(target) = args.get("target") {
        let target = target
            .as_object()
            .ok_or_else(|| "target must be an object".to_owned())?;
        match target.get("type").and_then(Value::as_str) {
            Some("file") => {
                reject_nested_unknown(target, &["type", "file"])?;
                push_required_string(cli, target, "file", "--file")
            }
            Some("symbol") => push_symbol_target(cli, target),
            _ => Err("target.type must be file or symbol".to_owned()),
        }?;
    } else if args.contains_key("file") {
        push_required_string(cli, args, "file", "--file")?;
    } else if args.contains_key("symbol") {
        push_required_string(cli, args, "symbol", "--symbol")?;
    } else {
        return Err("inspect_target requires target, file, or symbol".to_owned());
    }
    push_bool_mode(cli, args, "production", "--production", "--no-production")
}

fn trace_file_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_required_string(cli, args, "file", "--file")
}

fn trace_export_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_required_string(cli, args, "file", "--file")?;
    if let Some(export_name) = string_arg(args, "export_name")? {
        cli.extend(["--symbol".to_owned(), export_name]);
        return Ok(());
    }
    push_required_string(cli, args, "symbol", "--symbol")
}

fn trace_dependency_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    if let Some(package) = string_arg(args, "package_name")? {
        cli.extend(["--dependency".to_owned(), package]);
        return Ok(());
    }
    push_required_string(cli, args, "dependency", "--dependency")
}

fn trace_clone_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    if args.contains_key("fingerprint") && (args.contains_key("file") || args.contains_key("line"))
    {
        return Err("trace_clone accepts either fingerprint or file and line".to_owned());
    }
    if let Some(fingerprint) = string_arg(args, "fingerprint")? {
        cli.extend(["--fingerprint".to_owned(), fingerprint]);
    } else {
        let file = string_arg(args, "file")?
            .ok_or_else(|| "trace_clone requires fingerprint or file and line".to_owned())?;
        let line = args
            .get("line")
            .and_then(Value::as_u64)
            .ok_or_else(|| "trace_clone line must be a positive integer".to_owned())?;
        if line == 0 {
            return Err("trace_clone line must be a positive integer".to_owned());
        }
        cli.extend(["--fingerprint".to_owned(), format!("{file}:{line}")]);
    }
    push_duplicate_args(cli, args)
}

fn dupes_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_report_scope_args(cli, args)?;
    push_baseline_args(cli, args)?;
    push_duplicate_args(cli, args)
}

fn push_duplicate_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_string_flag(cli, args, "mode", "--mode")?;
    push_number_flag(cli, args, "min_tokens", "--min-tokens")?;
    push_number_flag(cli, args, "min_lines", "--min-lines")?;
    push_number_flag(cli, args, "min_occurrences", "--min-occurrences")?;
    push_number_flag(cli, args, "top", "--top")?;
    push_float_flag(cli, args, "threshold", "--threshold")?;
    if bool_arg(args, "cross_language")?.unwrap_or_default() {
        return Err("cross_language is not supported for Dart-only analysis".to_owned());
    }
    for (key, flag) in [
        ("skip_local", "--skip-local"),
        ("ignore_imports", "--ignore-imports"),
        ("no_ignore_imports", "--no-ignore-imports"),
    ] {
        push_bool_flag(cli, args, key, flag)?;
    }
    Ok(())
}

fn health_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_report_scope_args(cli, args)?;
    push_baseline_args(cli, args)?;
    push_health_args(cli, args)
}

fn coverage_analyze_args(args: &Map<String, Value>) -> Result<Vec<String>, String> {
    let mut cli = json_command_args(&["coverage", "analyze"]);
    push_string_flag(&mut cli, args, "root", "--root")?;
    push_string_flag(&mut cli, args, "config", "--config")?;
    push_required_string(&mut cli, args, "coverage", "--runtime-coverage")?;
    push_number_flag(
        &mut cli,
        args,
        "min_invocations_hot",
        "--min-invocations-hot",
    )?;
    push_number_flag(
        &mut cli,
        args,
        "min_observation_volume",
        "--min-observation-volume",
    )?;
    push_float_flag(
        &mut cli,
        args,
        "low_traffic_threshold",
        "--low-traffic-threshold",
    )?;
    push_number_flag(&mut cli, args, "top", "--top")?;
    push_string_flag(&mut cli, args, "repo", "--repo")?;
    Ok(cli)
}

fn push_health_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_number_flag(cli, args, "max_cyclomatic", "--max-cyclomatic")?;
    push_number_flag(cli, args, "max_cognitive", "--max-cognitive")?;
    push_number_flag(cli, args, "max_crap", "--max-crap")?;
    push_string_flag(cli, args, "coverage", "--coverage")?;
    push_string_flag(cli, args, "runtime_coverage", "--runtime-coverage")?;
    push_number_flag(cli, args, "min_invocations_hot", "--min-invocations-hot")?;
    push_number_flag(
        cli,
        args,
        "min_observation_volume",
        "--min-observation-volume",
    )?;
    push_float_flag(
        cli,
        args,
        "low_traffic_threshold",
        "--low-traffic-threshold",
    )?;
    for (key, flag) in [
        ("coverage_gaps", "--coverage-gaps"),
        ("file_scores", "--file-scores"),
        ("hotspots", "--hotspots"),
        ("targets", "--targets"),
        ("ownership", "--ownership"),
        ("complexity_breakdown", "--complexity-breakdown"),
    ] {
        push_bool_flag(cli, args, key, flag)?;
    }
    push_number_flag(cli, args, "min_score", "--min-score")?;
    push_number_flag(cli, args, "top", "--top")?;
    Ok(())
}

fn security_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_report_scope_args(cli, args)?;
    push_baseline_args(cli, args)?;
    push_number_flag(cli, args, "top", "--top")?;
    push_bool_flag(cli, args, "surface", "--surface")?;
    push_string_flag(cli, args, "gate", "--gate")?;
    push_string_flags(cli, args, "paths", "--file")?;
    push_string_flag(cli, args, "diff_file", "--diff-file")?;
    for (key, flag) in [
        ("ci", "--ci"),
        ("fail_on_issues", "--fail-on-issues"),
        ("summary", "--summary"),
    ] {
        push_bool_flag(cli, args, key, flag)?;
    }
    Ok(())
}

fn flags_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_report_scope_args(cli, args)?;
    push_baseline_args(cli, args)?;
    push_number_flag(cli, args, "top", "--top")?;
    Ok(())
}

fn impact_args(args: &Map<String, Value>) -> Result<Vec<String>, String> {
    let mut cli = json_command_args(&["impact"]);
    cli.push("--quiet".to_owned());
    push_string_flag(&mut cli, args, "root", "--root")?;
    Ok(cli)
}

fn impact_all_args(args: &Map<String, Value>) -> Result<Vec<String>, String> {
    let mut cli = json_command_args(&["impact"]);
    cli.extend(["--quiet".to_owned(), "--all".to_owned()]);
    push_string_flag(&mut cli, args, "sort", "--sort")?;
    push_number_flag(&mut cli, args, "limit", "--limit")?;
    Ok(cli)
}

fn fix_preview_args(args: &Map<String, Value>) -> Result<Vec<String>, String> {
    let mut cli = json_command_args(&["fix"]);
    push_string_flag(&mut cli, args, "root", "--root")?;
    push_string_flag(&mut cli, args, "config", "--config")?;
    push_report_scope_args(&mut cli, args)?;
    push_string_flags(&mut cli, args, "action", "--action")?;
    push_noop_bool(args, "no_create_config")?;
    cli.push("--dry-run".to_owned());
    Ok(cli)
}

fn fix_apply_args(args: &Map<String, Value>) -> Result<Vec<String>, String> {
    if bool_arg(args, "yes")? != Some(true) {
        return Err("fix_apply requires yes: true".to_owned());
    }
    let mut cli = json_command_args(&["fix"]);
    push_string_flag(&mut cli, args, "root", "--root")?;
    push_string_flag(&mut cli, args, "config", "--config")?;
    push_report_scope_args(&mut cli, args)?;
    push_string_flags(&mut cli, args, "action", "--action")?;
    push_noop_bool(args, "no_create_config")?;
    cli.push("--yes".to_owned());
    Ok(cli)
}

fn audit_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_required_string(cli, args, "base", "--base")?;
    push_string_flag(cli, args, "gate", "--gate")?;
    push_report_scope_args(cli, args)?;
    push_symbol_args(cli, args)?;
    push_boundary_args(cli, args)?;
    push_duplicate_args(cli, args)?;
    push_health_args(cli, args)?;
    for (key, flag) in [
        ("dead_code_baseline", "--dead-code-baseline"),
        ("health_baseline", "--health-baseline"),
        ("dupes_baseline", "--dupes-baseline"),
    ] {
        push_string_flag(cli, args, key, flag)?;
    }
    push_number_flag(cli, args, "max_decisions", "--max-decisions")?;
    push_bool_flag(cli, args, "brief", "--brief")?;
    Ok(())
}

fn decision_surface_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_required_string(cli, args, "base", "--base")?;
    push_number_flag(cli, args, "max_decisions", "--max-decisions")?;
    Ok(())
}

fn explain_args(name: &str, args: &Map<String, Value>) -> Result<Vec<String>, String> {
    let issue_type = string_arg(args, "issue_type")?
        .or(string_arg(args, "rule_id")?)
        .ok_or_else(|| format!("{name} requires issue_type"))?;
    Ok(vec![
        "dart-decimate".to_owned(),
        "explain".to_owned(),
        issue_type,
        "--format".to_owned(),
        "json".to_owned(),
    ])
}

fn push_report_scope_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_string_flags(cli, args, "entry", "--entry")?;
    push_string_flag(cli, args, "dart_platform", "--dart-platform")?;
    push_string_flags(cli, args, "file", "--file")?;
    push_string_flags(cli, args, "workspace", "--workspace")?;
    push_string_flag(cli, args, "changed_workspaces", "--changed-workspaces")?;
    push_string_flag(cli, args, "changed_since", "--changed-since")?;
    push_string_flag(cli, args, "compare", "--compare")?;
    push_bool_mode(cli, args, "production", "--production", "--no-production")?;
    Ok(())
}

fn push_baseline_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_string_flag(cli, args, "baseline", "--baseline")?;
    push_string_flag(cli, args, "regression_baseline", "--regression-baseline")?;
    push_bool_flag(cli, args, "fail_on_regression", "--fail-on-regression")?;
    push_string_flag(cli, args, "tolerance", "--tolerance")?;
    Ok(())
}

fn push_symbol_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_bool_flag(
        cli,
        args,
        "include_entry_exports",
        "--include-entry-exports",
    )?;
    push_bool_flag(cli, args, "private_type_leaks", "--private-type-leaks")
}

fn push_boundary_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_string_flags(cli, args, "boundary", "--boundary")?;
    push_bool_flag(cli, args, "boundary_coverage", "--boundary-coverage")?;
    push_string_flags(cli, args, "boundary_call", "--boundary-call")?;
    push_string_flags(cli, args, "policy_pack", "--policy-pack")?;
    push_bool_flag(cli, args, "policy_violations", "--policy-violations")
}

fn push_symbol_target(cli: &mut Vec<String>, target: &Map<String, Value>) -> Result<(), String> {
    reject_nested_unknown(target, &["type", "file", "symbol", "export_name"])?;
    let file = target
        .get("file")
        .and_then(Value::as_str)
        .ok_or_else(|| "symbol target requires file".to_owned())?;
    let symbol = target
        .get("export_name")
        .or_else(|| target.get("symbol"))
        .and_then(Value::as_str)
        .ok_or_else(|| "symbol target requires export_name or symbol".to_owned())?;
    cli.extend(["--symbol".to_owned(), format!("{file}:{symbol}")]);
    Ok(())
}

fn reject_nested_unknown(value: &Map<String, Value>, allowed: &[&str]) -> Result<(), String> {
    for key in value.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("target does not accept argument {key}"));
        }
    }
    Ok(())
}
