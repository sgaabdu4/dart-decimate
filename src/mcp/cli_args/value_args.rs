use serde_json::{Map, Value};

pub(super) fn push_string_flag(
    cli: &mut Vec<String>,
    args: &Map<String, Value>,
    key: &str,
    flag: &str,
) -> Result<(), String> {
    if let Some(value) = string_arg(args, key)? {
        cli.extend([flag.to_owned(), value]);
    }
    Ok(())
}

pub(super) fn push_string_flags(
    cli: &mut Vec<String>,
    args: &Map<String, Value>,
    key: &str,
    flag: &str,
) -> Result<(), String> {
    match args.get(key) {
        Some(Value::String(value)) => cli.extend([flag.to_owned(), value.clone()]),
        Some(value @ Value::Array(_)) => {
            for value in array_strings(value, key)? {
                cli.extend([flag.to_owned(), value.to_owned()]);
            }
        }
        Some(_) => return Err(format!("{key} must be a string or string array")),
        None => {}
    }
    Ok(())
}

pub(super) fn push_required_string(
    cli: &mut Vec<String>,
    args: &Map<String, Value>,
    key: &str,
    flag: &str,
) -> Result<(), String> {
    let value = string_arg(args, key)?.ok_or_else(|| format!("{key} is required"))?;
    cli.extend([flag.to_owned(), value]);
    Ok(())
}

pub(super) fn push_number_flag(
    cli: &mut Vec<String>,
    args: &Map<String, Value>,
    key: &str,
    flag: &str,
) -> Result<(), String> {
    let Some(value) = args.get(key) else {
        return Ok(());
    };
    let Some(number) = value.as_u64() else {
        return Err(format!("{key} must be a non-negative integer"));
    };
    cli.extend([flag.to_owned(), number.to_string()]);
    Ok(())
}

pub(super) fn push_float_flag(
    cli: &mut Vec<String>,
    args: &Map<String, Value>,
    key: &str,
    flag: &str,
) -> Result<(), String> {
    let Some(value) = args.get(key) else {
        return Ok(());
    };
    let Some(number) = value.as_f64() else {
        return Err(format!("{key} must be a non-negative number"));
    };
    if !number.is_finite() || number.is_sign_negative() {
        return Err(format!("{key} must be a non-negative number"));
    }
    cli.extend([flag.to_owned(), number.to_string()]);
    Ok(())
}

pub(super) fn push_bool_flag(
    cli: &mut Vec<String>,
    args: &Map<String, Value>,
    key: &str,
    flag: &str,
) -> Result<(), String> {
    if bool_arg(args, key)? == Some(true) {
        cli.push(flag.to_owned());
    }
    Ok(())
}

pub(super) fn push_bool_mode(
    cli: &mut Vec<String>,
    args: &Map<String, Value>,
    key: &str,
    true_flag: &str,
    false_flag: &str,
) -> Result<(), String> {
    match bool_arg(args, key)? {
        Some(true) => cli.push(true_flag.to_owned()),
        Some(false) => cli.push(false_flag.to_owned()),
        None => {}
    }
    Ok(())
}

pub(super) fn push_noop_bool(args: &Map<String, Value>, key: &str) -> Result<(), String> {
    let _ = bool_arg(args, key)?;
    Ok(())
}

pub(super) fn string_arg(args: &Map<String, Value>, key: &str) -> Result<Option<String>, String> {
    match args.get(key) {
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(format!("{key} must be a string")),
        None => Ok(None),
    }
}

pub(super) fn bool_arg(args: &Map<String, Value>, key: &str) -> Result<Option<bool>, String> {
    match args.get(key) {
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(format!("{key} must be a boolean")),
        None => Ok(None),
    }
}

pub(super) fn array_strings<'value>(
    value: &'value Value,
    key: &str,
) -> Result<Vec<&'value str>, String> {
    let Some(values) = value.as_array() else {
        return Err(format!("{key} must be a string array"));
    };
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| format!("{key} entries must be strings"))
        })
        .collect()
}

pub(super) fn issue_filter_flag(issue_type: &str) -> Result<String, String> {
    match issue_type {
        "unused-files" | "unused-file" => Ok("--unused-files".to_owned()),
        "unused-exports" | "unused-export" => Ok("--unused-exports".to_owned()),
        "unused-types" | "unused-type" => Ok("--unused-types".to_owned()),
        "unused-deps" | "unused-dependency" | "unused-dependencies" => {
            Ok("--unused-deps".to_owned())
        }
        "unlisted-deps" | "unlisted-dependency" | "unlisted-dependencies" => {
            Ok("--unlisted-deps".to_owned())
        }
        "private-src-import" | "private-src-imports" => Ok("--private-src-imports".to_owned()),
        "duplicate-exports" | "duplicate-export" => Ok("--duplicate-exports".to_owned()),
        "circular-deps" | "circular-dependency" => Ok("--circular-deps".to_owned()),
        "re-export-cycles" | "re-export-cycle" => Ok("--re-export-cycles".to_owned()),
        "boundary-violations" | "boundary-violation" => Ok("--boundary-violations".to_owned()),
        "policy-violations" | "policy-violation" => Ok("--policy-violations".to_owned()),
        "unused-enum-members" | "unused-enum-member" => Ok("--unused-enum-members".to_owned()),
        "unused-class-members" | "unused-class-member" => Ok("--unused-class-members".to_owned()),
        "unresolved-imports" | "unresolved-dependency" => Ok("--unresolved-imports".to_owned()),
        "stale-suppressions" | "stale-suppression" => Ok("--stale-suppressions".to_owned()),
        "unused-dependency-overrides" | "unused-dependency-override" => {
            Ok("--unused-dependency-overrides".to_owned())
        }
        "misconfigured-dependency-overrides" | "misconfigured-dependency-override" => {
            Ok("--misconfigured-dependency-overrides".to_owned())
        }
        "package-cycles" | "package-cycle" => Ok("--package-cycles".to_owned()),
        "private-type-leak" | "private-type-leaks" => Ok("--private-type-leaks".to_owned()),
        _ => Err(format!("unsupported issue_type {issue_type}")),
    }
}
