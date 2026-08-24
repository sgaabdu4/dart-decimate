use std::path::Path;

use serde_json::{Map, Value, json};

use super::{
    AGENT_COMMAND, HookAction, HookFile, HooksError, claude_settings_path, missing_file, read_hook,
    relative_path, write_text,
};

pub(super) fn install_claude_settings(root: &Path, force: bool) -> Result<HookFile, HooksError> {
    let path = claude_settings_path(root);
    let (mut settings, action) = if path.exists() {
        let source = read_hook(&path)?;
        if settings_source_has_agent_command(&source) {
            return claude_settings_status(root, HookAction::Unchanged);
        }
        match serde_json::from_str::<Value>(&source) {
            Ok(value) if value.is_object() => (value, HookAction::Overwritten),
            _ if force => (json!({}), HookAction::Overwritten),
            _ => {
                return Err(HooksError::UnmanagedHook { path: path.clone() });
            }
        }
    } else {
        (json!({}), HookAction::Created)
    };
    add_agent_command_to_settings(&mut settings);
    let source = serde_json::to_string_pretty(&settings).map_err(|source| HooksError::Write {
        path: path.clone(),
        source: std::io::Error::other(source),
    })?;
    write_text(&path, &format!("{source}\n"))?;
    claude_settings_status(root, action)
}

pub(super) fn uninstall_claude_settings(root: &Path, force: bool) -> Result<HookFile, HooksError> {
    let path = claude_settings_path(root);
    if !path.exists() {
        return Ok(missing_file(root, &path));
    }
    let source = read_hook(&path)?;
    if !settings_source_has_agent_command(&source) {
        return claude_settings_status(root, HookAction::Unchanged);
    }
    let mut settings = match serde_json::from_str::<Value>(&source) {
        Ok(value) => value,
        Err(_) if force => json!({}),
        Err(_) => {
            return Err(HooksError::UnmanagedHook { path: path.clone() });
        }
    };
    remove_agent_command_from_settings(&mut settings);
    let source = serde_json::to_string_pretty(&settings).map_err(|source| HooksError::Write {
        path: path.clone(),
        source: std::io::Error::other(source),
    })?;
    write_text(&path, &format!("{source}\n"))?;
    claude_settings_status(root, HookAction::Removed)
}

pub(super) fn claude_settings_status(
    root: &Path,
    action: HookAction,
) -> Result<HookFile, HooksError> {
    let path = claude_settings_path(root);
    let installed = path.is_file();
    let managed = installed && settings_source_has_agent_command(&read_hook(&path)?);
    Ok(HookFile {
        path: relative_path(root, &path),
        installed,
        managed,
        action,
    })
}

fn settings_source_has_agent_command(source: &str) -> bool {
    serde_json::from_str::<Value>(source).is_ok_and(|value| value_has_agent_command(&value))
}

fn value_has_agent_command(value: &Value) -> bool {
    match value {
        Value::Object(object) => {
            object.get("command").and_then(Value::as_str) == Some(AGENT_COMMAND)
                || object.values().any(value_has_agent_command)
        }
        Value::Array(values) => values.iter().any(value_has_agent_command),
        _ => false,
    }
}

fn add_agent_command_to_settings(settings: &mut Value) {
    let Some(root) = settings.as_object_mut() else {
        return;
    };
    ensure_object_field(root, "hooks");
    let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return;
    };
    ensure_array_field(hooks, "PreToolUse");
    let Some(pre_tool_use) = hooks.get_mut("PreToolUse").and_then(Value::as_array_mut) else {
        return;
    };
    if let Some(group) = pre_tool_use
        .iter_mut()
        .find(|group| group.get("matcher").and_then(Value::as_str) == Some("Bash"))
    {
        let Some(group) = group.as_object_mut() else {
            return;
        };
        ensure_array_field(group, "hooks");
        let Some(hooks) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
            return;
        };
        if !hooks.iter().any(is_agent_command_hook) {
            hooks.push(agent_command_hook());
        }
        return;
    }
    pre_tool_use.push(json!({
        "matcher": "Bash",
        "hooks": [agent_command_hook()]
    }));
}

fn remove_agent_command_from_settings(settings: &mut Value) {
    let Some(root) = settings.as_object_mut() else {
        return;
    };
    let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return;
    };
    let Some(pre_tool_use) = hooks.get_mut("PreToolUse").and_then(Value::as_array_mut) else {
        return;
    };
    for group in pre_tool_use.iter_mut() {
        let Some(group) = group.as_object_mut() else {
            continue;
        };
        let Some(hooks) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
            continue;
        };
        hooks.retain(|hook| !is_agent_command_hook(hook));
    }
    pre_tool_use.retain(|group| {
        group
            .get("hooks")
            .and_then(Value::as_array)
            .is_none_or(|hooks| !hooks.is_empty())
    });
}

fn ensure_object_field(object: &mut Map<String, Value>, key: &str) {
    if !object.get(key).is_some_and(Value::is_object) {
        object.insert(key.to_owned(), json!({}));
    }
}

fn ensure_array_field(object: &mut Map<String, Value>, key: &str) {
    if !object.get(key).is_some_and(Value::is_array) {
        object.insert(key.to_owned(), json!([]));
    }
}

fn agent_command_hook() -> Value {
    json!({
        "type": "command",
        "command": AGENT_COMMAND
    })
}

fn is_agent_command_hook(hook: &Value) -> bool {
    hook.get("command").and_then(Value::as_str) == Some(AGENT_COMMAND)
}
