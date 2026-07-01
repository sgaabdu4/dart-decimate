use std::ffi::OsString;
use std::io::{self, Write};

use serde::Serialize;

use super::{CliError, default_command, run_from};

/// Run Dart Decimate from process arguments and return an exit code.
#[must_use]
pub fn run_from_env() -> i32 {
    let args = std::env::args_os().collect::<Vec<_>>();
    let json_requested = format_json_requested(&args);

    match run_from(args, io::stdout().lock()) {
        Ok(code) => code,
        Err(CliError::Clap(error)) if json_requested => {
            let code = error.exit_code();
            let _ = write_json_error(io::stdout().lock(), &error.to_string(), code);
            code
        }
        Err(CliError::Clap(error)) => {
            let code = error.exit_code();
            let _ = error.print();
            code
        }
        Err(error) if json_requested => {
            let code = 2;
            let _ = write_json_error(io::stdout().lock(), &error.to_string(), code);
            code
        }
        Err(error) => {
            eprintln!("{error}");
            2
        }
    }
}

fn format_json_requested(args: &[OsString]) -> bool {
    if default_command::output_alias_help_requested(args) {
        return false;
    }
    if default_command::json_output_alias_requested(args) {
        return true;
    }

    let expanded_args = default_command::args_with_default_check(args.iter().cloned());
    let mut args = expanded_args.iter().skip(1).filter_map(|arg| arg.to_str());
    while let Some(arg) = args.next() {
        if arg == "--format" && args.next().is_some_and(|value| value == "json") {
            return true;
        }
        if arg == "--format=json" {
            return true;
        }
    }
    false
}

fn write_json_error<W: Write>(mut writer: W, message: &str, exit_code: i32) -> io::Result<()> {
    serde_json::to_writer_pretty(
        &mut writer,
        &JsonError {
            error: true,
            message,
            exit_code,
        },
    )?;
    writeln!(writer)
}

#[derive(Debug, Serialize)]
struct JsonError<'a> {
    error: bool,
    message: &'a str,
    exit_code: i32,
}
