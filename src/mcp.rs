use std::io::{self, BufRead, Write};

use serde_json::{Map, Value, json};

use crate::cli::{CliError, run_from};

mod cli_args;
#[cfg(test)]
mod cli_args_tests;
pub(crate) mod code_execute;
mod tools;

use cli_args::cli_args_for_tool;
use code_execute::execute as execute_code;
use tools::tools;

const PROTOCOL_VERSION: &str = "2025-11-25";

/// # Errors
///
/// Returns an IO error if stdin or stdout cannot be read or written.
pub fn run_stdio() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = handle_message(&line) {
            serde_json::to_writer(&mut stdout, &response)?;
            writeln!(stdout)?;
            stdout.flush()?;
        }
    }
    Ok(())
}

/// Handle one newline-delimited JSON-RPC MCP message.
#[must_use]
pub fn handle_message(message: &str) -> Option<Value> {
    let value = match serde_json::from_str::<Value>(message) {
        Ok(value) => value,
        Err(error) => return Some(response_error(&Value::Null, -32700, error.to_string())),
    };
    let id = value.get("id").cloned();
    let Some(method) = value.get("method").and_then(Value::as_str) else {
        return id.map(|id| response_error(&id, -32600, "missing method"));
    };
    id.as_ref()?;
    let id = id.unwrap_or(Value::Null);
    match method {
        "initialize" => Some(response_result(&id, &initialize_result(&value))),
        "ping" => Some(response_result(&id, &json!({}))),
        "tools/list" => Some(response_result(&id, &json!({ "tools": tools() }))),
        "tools/call" => Some(call_tool(&id, &value)),
        _ => Some(response_error(
            &id,
            -32601,
            format!("unknown method {method}"),
        )),
    }
}

fn initialize_result(message: &Value) -> Value {
    let requested = message
        .get("params")
        .and_then(|params| params.get("protocolVersion"))
        .and_then(Value::as_str)
        .unwrap_or(PROTOCOL_VERSION);
    let protocol_version = if requested == PROTOCOL_VERSION {
        requested
    } else {
        PROTOCOL_VERSION
    };
    json!({
        "protocolVersion": protocol_version,
        "capabilities": {
            "tools": { "listChanged": false }
        },
        "serverInfo": {
            "name": "dart-decimate-mcp",
            "title": "Dart Decimate MCP",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": "Dart and Flutter codebase intelligence. Mutating fix_apply is available only with explicit yes: true confirmation."
    })
}

fn call_tool(id: &Value, message: &Value) -> Value {
    let params = message.get("params").unwrap_or(&Value::Null);
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return response_error(id, -32602, "tools/call requires params.name");
    };
    let Some(arguments) = params.get("arguments") else {
        return match cli_args_for_tool(name, &Map::new()) {
            Ok(args) => response_result(id, &tool_result(run_tool_json(name, args))),
            Err(message) => response_error(id, -32602, message),
        };
    };
    let Some(arguments) = arguments.as_object() else {
        return response_error(id, -32602, "tools/call params.arguments must be an object");
    };
    if name == "code_execute" {
        return response_result(id, &tool_result(execute_code(arguments)));
    }
    match cli_args_for_tool(name, arguments) {
        Ok(args) => response_result(id, &tool_result(run_tool_json(name, args))),
        Err(message) => response_error(id, -32602, message),
    }
}

fn run_tool_json(name: &str, args: Vec<String>) -> CliToolOutput {
    let mut output = run_cli_json(args);
    if let Some(structured) = runtime_slice_content(name, output.structured.as_ref()) {
        output.text =
            serde_json::to_string_pretty(&structured).unwrap_or_else(|_| structured.to_string());
        output.structured = Some(structured);
    }
    output
}

fn run_cli_json(args: Vec<String>) -> CliToolOutput {
    let mut output = Vec::new();
    let code = match run_from(args, &mut output) {
        Ok(code) => code,
        Err(error) => {
            return CliToolOutput::json(error_exit_code(&error), cli_error_json(&error), true);
        }
    };
    let text = String::from_utf8_lossy(&output).into_owned();
    let structured = serde_json::from_str::<Value>(&text).ok();
    CliToolOutput::new(code, text, structured, code == 2)
}

fn error_exit_code(error: &CliError) -> i32 {
    match error {
        CliError::Clap(error) => error.exit_code(),
        _ => 2,
    }
}

fn cli_error_json(error: &CliError) -> Value {
    json!({
        "error": true,
        "message": error.to_string(),
        "exit_code": error_exit_code(error)
    })
}

fn runtime_slice_content(name: &str, structured: Option<&Value>) -> Option<Value> {
    let runtime = structured?.get("runtime_coverage")?;
    let mut slice = runtime_slice_base(name, structured?, runtime)?;
    match name {
        "get_hot_paths" => {
            slice.insert(
                "hot_paths".to_owned(),
                runtime
                    .get("hot_paths")
                    .cloned()
                    .unwrap_or_else(|| json!([])),
            );
        }
        "get_blast_radius" => {
            slice.insert(
                "blast_radius".to_owned(),
                runtime
                    .get("blast_radius")
                    .cloned()
                    .unwrap_or_else(|| json!([])),
            );
        }
        "get_importance" => {
            slice.insert(
                "importance".to_owned(),
                runtime
                    .get("importance")
                    .cloned()
                    .unwrap_or_else(|| json!([])),
            );
        }
        "get_cleanup_candidates" => {
            slice.insert(
                "findings".to_owned(),
                cleanup_findings(runtime.get("findings")),
            );
            slice.insert(
                "coverage_intelligence".to_owned(),
                cleanup_intelligence(runtime.get("coverage_intelligence")),
            );
            slice.insert(
                "actionable".to_owned(),
                runtime
                    .get("actionable")
                    .cloned()
                    .unwrap_or_else(|| json!({})),
            );
        }
        _ => return None,
    }
    Some(Value::Object(slice))
}

fn runtime_slice_base(
    name: &str,
    structured: &Value,
    runtime: &Value,
) -> Option<Map<String, Value>> {
    let kind = match name {
        "get_hot_paths" => "runtime-hot-paths",
        "get_blast_radius" => "runtime-blast-radius",
        "get_importance" => "runtime-importance",
        "get_cleanup_candidates" => "runtime-cleanup-candidates",
        _ => return None,
    };
    let mut slice = Map::new();
    for key in ["schema_version", "tool", "command"] {
        if let Some(value) = structured.get(key) {
            slice.insert(key.to_owned(), value.clone());
        }
    }
    slice.insert("kind".to_owned(), Value::String(kind.to_owned()));
    for key in ["summary", "provenance", "watermark", "warnings"] {
        if let Some(value) = runtime.get(key) {
            slice.insert(key.to_owned(), value.clone());
        }
    }
    Some(slice)
}

fn cleanup_findings(findings: Option<&Value>) -> Value {
    let values = findings
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|finding| {
            finding["safe_to_delete"].as_bool().unwrap_or_default()
                || matches!(
                    finding["kind"].as_str(),
                    Some("low-traffic" | "coverage-unavailable")
                )
        })
        .cloned()
        .collect::<Vec<_>>();
    Value::Array(values)
}

fn cleanup_intelligence(intelligence: Option<&Value>) -> Value {
    let values = intelligence
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|row| {
            matches!(
                row["kind"].as_str(),
                Some("low-traffic" | "coverage-unavailable")
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    Value::Array(values)
}

fn tool_result(output: CliToolOutput) -> Value {
    let mut result = Map::new();
    result.insert(
        "content".to_owned(),
        json!([{ "type": "text", "text": output.text }]),
    );
    result.insert("isError".to_owned(), Value::Bool(output.is_error));
    result.insert("_meta".to_owned(), json!({ "exit_code": output.exit_code }));
    if let Some(structured) = output.structured {
        result.insert("structuredContent".to_owned(), structured);
    }
    Value::Object(result)
}

#[derive(Debug)]
struct CliToolOutput {
    exit_code: i32,
    text: String,
    structured: Option<Value>,
    is_error: bool,
}

impl CliToolOutput {
    fn new(exit_code: i32, text: String, structured: Option<Value>, is_error: bool) -> Self {
        Self {
            exit_code,
            text,
            structured,
            is_error,
        }
    }

    fn json(exit_code: i32, structured: Value, is_error: bool) -> Self {
        Self::new(
            exit_code,
            structured.to_string(),
            Some(structured),
            is_error,
        )
    }
}

fn response_result(id: &Value, result: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn response_error(id: &Value, code: i32, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message.into()
        }
    })
}
