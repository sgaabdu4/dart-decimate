use std::fmt::Write as FmtWrite;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::Builder;

use crate::output::{JsonReport, render_html_report};

pub(super) fn write_and_open_html_report<W: Write>(
    report: &JsonReport,
    mut writer: W,
) -> io::Result<()> {
    write_and_open_html_document_with(
        report.command.as_str(),
        &render_html_report(report),
        &mut writer,
        open_url,
    )
}

pub(super) fn write_and_open_html_document_with<W, F>(
    command: &str,
    html: &str,
    writer: &mut W,
    opener: F,
) -> io::Result<()>
where
    W: Write,
    F: FnOnce(&str) -> io::Result<()>,
{
    let path = write_temp_html_document(command, html)?;
    let url = file_url(&path);
    opener(&url)?;
    writeln!(writer, "Opened HTML report: {url}")?;
    Ok(())
}

fn write_temp_html_document(command: &str, html: &str) -> io::Result<PathBuf> {
    let prefix = format!("dart-decimate-{}-", safe_filename_part(command));
    let mut file = Builder::new()
        .prefix(&prefix)
        .suffix(".html")
        .tempfile_in(std::env::temp_dir())?;
    file.write_all(html.as_bytes())?;
    file.as_file_mut().flush()?;
    let (_file, path) = file.keep().map_err(|error| error.error)?;
    Ok(path)
}

fn safe_filename_part(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn file_url(path: &Path) -> String {
    let raw = path.to_string_lossy().replace('\\', "/");
    let encoded = percent_encode_path(&raw);
    if encoded.starts_with('/') {
        format!("file://{encoded}")
    } else {
        format!("file:///{encoded}")
    }
}

fn percent_encode_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b':' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

pub(super) fn open_url(url: &str) -> io::Result<()> {
    let command = open_command(url, OpenPlatform::current());
    let status = Command::new(command.program).args(command.args).status()?;

    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "failed to open HTML report in browser: {status}"
        )))
    }
}

struct OpenCommand {
    program: &'static str,
    args: Vec<String>,
}

#[derive(Clone, Copy)]
enum OpenPlatform {
    Macos,
    Windows,
    Other,
}

impl OpenPlatform {
    fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Other
        }
    }
}

fn open_command(url: &str, platform: OpenPlatform) -> OpenCommand {
    match platform {
        OpenPlatform::Macos => OpenCommand {
            program: "open",
            args: vec![url.to_owned()],
        },
        OpenPlatform::Windows => OpenCommand {
            program: "explorer.exe",
            args: vec![url.to_owned()],
        },
        OpenPlatform::Other => OpenCommand {
            program: "xdg-open",
            args: vec![url.to_owned()],
        },
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;

    use crate::output::{ReportCommand, ReportSummary, Verdict};

    use super::*;

    #[test]
    fn file_urls_are_percent_encoded() {
        let url = file_url(Path::new("/tmp/dart-decimate/report <1>.html"));

        assert_eq!(url, "file:///tmp/dart-decimate/report%20%3C1%3E.html");
    }

    #[test]
    fn windows_open_command_preserves_encoded_file_url() {
        let url = "file:///C:/Users/Ada%20Lovelace/report%20%C3%A9.html";
        let command = open_command(url, OpenPlatform::Windows);

        assert_eq!(command.program, "explorer.exe");
        assert_eq!(command.args, vec![url.to_owned()]);
    }

    #[test]
    fn temp_html_documents_use_random_unique_names() -> Result<(), Box<dyn std::error::Error>> {
        let first = write_temp_html_document("audit --brief", "<!doctype html>first")?;
        let second = write_temp_html_document("audit --brief", "<!doctype html>second")?;

        assert_ne!(first, second);
        assert_eq!(fs::read_to_string(&first)?, "<!doctype html>first");
        assert_eq!(fs::read_to_string(&second)?, "<!doctype html>second");
        assert!(first.file_name().is_some_and(|name| {
            let name = name.to_string_lossy();
            name.starts_with("dart-decimate-audit---brief-") && name.ends_with(".html")
        }));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn temp_html_documents_are_private() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        let path = write_temp_html_document("check", "<!doctype html>private")?;
        let mode = fs::metadata(path)?.permissions().mode() & 0o777;

        assert_eq!(mode, 0o600);
        Ok(())
    }

    #[test]
    fn writes_temp_html_and_opens_file_url() -> Result<(), Box<dyn std::error::Error>> {
        let report = JsonReport {
            schema_version: "dart-decimate.report.v1".to_owned(),
            kind: "combined".to_owned(),
            tool: "dart-decimate".to_owned(),
            command: ReportCommand::Check,
            verdict: Verdict::Pass,
            summary: ReportSummary {
                files: 1,
                ..ReportSummary::default()
            },
            findings: Vec::new(),
            clone_groups: Vec::new(),
            complexity: Vec::new(),
            file_scores: Vec::new(),
            hotspots: Vec::new(),
            refactoring_targets: Vec::new(),
            threshold_overrides: Vec::new(),
            feature_flags: Vec::new(),
            security_candidates: Vec::new(),
            attack_surface: Vec::new(),
            runtime_coverage: None,
            next_steps: Vec::new(),
        };
        let opened = RefCell::new(String::new());
        let mut output = Vec::new();

        let result = write_and_open_html_document_with(
            report.command.as_str(),
            &render_html_report(&report),
            &mut output,
            |url| {
                opened.replace(url.to_owned());
                Ok(())
            },
        );
        assert!(result.is_ok());

        let opened = opened.into_inner();
        assert!(opened.starts_with("file:///"));
        let message = String::from_utf8(output)?;
        assert!(message.contains("Opened HTML report: file:///"));
        Ok(())
    }
}
