use std::ffi::OsString;

use super::common_args::REPORT_FORMAT_VALUES;

const COMMANDS: &[&str] = &[
    "audit",
    "check",
    "ci",
    "ci-template",
    "config",
    "config-schema",
    "coverage",
    "cycles",
    "dead-code",
    "decision-surface",
    "dupes",
    "explain",
    "fix",
    "flags",
    "health",
    "html",
    "hooks",
    "impact",
    "init",
    "inspect",
    "license",
    "list",
    "migrate",
    "report-schema",
    "review",
    "rule-pack-schema",
    "schema",
    "security",
    "setup-hooks",
    "telemetry",
    "trace",
    "trace-clone",
    "trace-dependency",
    "trace-file",
    "trace-symbol",
    "watch",
    "workspaces",
    "human",
    "json",
];

pub(super) fn args_with_default_check<I, T>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let mut args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    if let Some(expanded) = args_with_output_alias(&args) {
        return expanded;
    }
    if should_insert_check(&args) {
        args.insert(1, OsString::from("check"));
    }
    args
}

fn args_with_output_alias(args: &[OsString]) -> Option<Vec<OsString>> {
    let alias = args.get(1)?.to_str()?;
    if args
        .iter()
        .skip(2)
        .take_while(|arg| !is_delimiter(arg))
        .any(is_help_arg)
    {
        return None;
    }
    let format = match alias {
        "human" => "human",
        "json" => "json",
        "html" => "html",
        _ => return None,
    };
    let mut expanded = Vec::with_capacity(args.len() + 3);
    expanded.push(args[0].clone());
    expanded.push(OsString::from("check"));
    let mut stdout_html = false;
    let mut open_html = false;
    let mut explicit_format_error = false;
    let mut index = 2;
    while let Some(arg) = args.get(index) {
        if is_delimiter(arg) {
            if !explicit_format_error {
                push_output_alias_flags(&mut expanded, alias, format, stdout_html, open_html);
            }
            expanded.push(arg.clone());
            expanded.extend(args.iter().skip(index + 1).cloned());
            return Some(expanded);
        }
        if alias == "html" && arg == "--stdout" {
            stdout_html = true;
            index += 1;
            continue;
        }
        if alias == "html" && arg == "--open" {
            open_html = true;
        }
        if arg == "--format" {
            let Some(value) = args.get(index + 1) else {
                expanded.push(arg.clone());
                explicit_format_error = true;
                index += 1;
                continue;
            };
            match value.to_str() {
                Some(value) if value.starts_with('-') => {
                    expanded.push(arg.clone());
                    explicit_format_error = true;
                    index += 1;
                }
                Some(value) if is_report_format(value) => {
                    index += 2;
                }
                _ => {
                    expanded.push(arg.clone());
                    expanded.push(value.clone());
                    explicit_format_error = true;
                    index += 2;
                }
            }
            continue;
        }
        if let Some(value) = arg
            .to_str()
            .and_then(|value| value.strip_prefix("--format="))
        {
            if !is_report_format(value) {
                expanded.push(arg.clone());
                explicit_format_error = true;
            }
            index += 1;
            continue;
        }
        expanded.push(arg.clone());
        index += 1;
    }
    if !explicit_format_error {
        push_output_alias_flags(&mut expanded, alias, format, stdout_html, open_html);
    }
    Some(expanded)
}

fn push_output_alias_flags(
    expanded: &mut Vec<OsString>,
    alias: &str,
    format: &str,
    stdout_html: bool,
    open_html: bool,
) {
    expanded.push(OsString::from("--format"));
    expanded.push(OsString::from(format));
    if alias == "html" && !stdout_html && !open_html {
        expanded.push(OsString::from("--open"));
    }
}

pub(super) fn output_alias_help_requested(args: &[OsString]) -> bool {
    matches!(
        args.get(1).and_then(|arg| arg.to_str()),
        Some("human" | "json" | "html")
    ) && args
        .iter()
        .skip(2)
        .take_while(|arg| !is_delimiter(arg))
        .any(is_help_arg)
}

pub(super) fn json_output_alias_requested(args: &[OsString]) -> bool {
    args.get(1).and_then(|arg| arg.to_str()) == Some("json")
        && !args
            .iter()
            .skip(2)
            .take_while(|arg| !is_delimiter(arg))
            .any(is_help_arg)
}

fn is_report_format(value: &str) -> bool {
    REPORT_FORMAT_VALUES.contains(&value)
}

pub(super) fn is_help_arg(arg: &OsString) -> bool {
    arg.to_str()
        .is_some_and(|value| matches!(value, "-h" | "--help"))
}

fn is_delimiter(arg: &OsString) -> bool {
    arg == "--"
}

fn should_insert_check(args: &[OsString]) -> bool {
    let Some(first) = args.get(1).and_then(|arg| arg.to_str()) else {
        return args.len() == 1;
    };
    if matches!(first, "-h" | "--help" | "-V" | "--version") {
        return false;
    }
    !COMMANDS.contains(&first)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values(items: &[&str]) -> Vec<OsString> {
        items.iter().map(OsString::from).collect()
    }

    #[test]
    fn output_aliases_expand_to_check_formats() {
        assert_eq!(
            args_with_default_check(["dart-decimate", "human", "app"]),
            values(&["dart-decimate", "check", "app", "--format", "human"])
        );
        assert_eq!(
            args_with_default_check(["dart-decimate", "json", "app", "--format", "human"]),
            values(&["dart-decimate", "check", "app", "--format", "json"])
        );
    }

    #[test]
    fn output_aliases_preserve_malformed_explicit_format_errors() {
        assert_eq!(
            args_with_default_check(["dart-decimate", "json", "app", "--format"]),
            values(&["dart-decimate", "check", "app", "--format"])
        );
        assert_eq!(
            args_with_default_check(["dart-decimate", "json", "app", "--format", "--quiet"]),
            values(&["dart-decimate", "check", "app", "--format", "--quiet"])
        );
    }

    #[test]
    fn output_aliases_preserve_invalid_explicit_format_errors() {
        for alias in ["human", "json", "html"] {
            assert_eq!(
                args_with_default_check(["dart-decimate", alias, "app", "--format", "xml"]),
                values(&["dart-decimate", "check", "app", "--format", "xml"])
            );
            assert_eq!(
                args_with_default_check(["dart-decimate", alias, "app", "--format=xml"]),
                values(&["dart-decimate", "check", "app", "--format=xml"])
            );
        }
    }

    #[test]
    fn output_aliases_keep_check_flags_with_invalid_explicit_format_errors() {
        assert_eq!(
            args_with_default_check([
                "dart-decimate",
                "json",
                "app",
                "--file",
                "lib/main.dart",
                "--format",
                "xml",
            ]),
            values(&[
                "dart-decimate",
                "check",
                "app",
                "--file",
                "lib/main.dart",
                "--format",
                "xml",
            ])
        );
    }

    #[test]
    fn output_aliases_insert_forced_flags_before_delimiter() {
        assert_eq!(
            args_with_default_check(["dart-decimate", "json", "--", "app"]),
            values(&["dart-decimate", "check", "--format", "json", "--", "app"])
        );
        assert_eq!(
            args_with_default_check(["dart-decimate", "html", "--stdout", "--", "app"]),
            values(&["dart-decimate", "check", "--format", "html", "--", "app"])
        );
        assert_eq!(
            args_with_default_check(["dart-decimate", "html", "--", "app"]),
            values(&[
                "dart-decimate",
                "check",
                "--format",
                "html",
                "--open",
                "--",
                "app",
            ])
        );
    }

    #[test]
    fn output_aliases_treat_help_after_delimiter_as_positional() {
        assert_eq!(
            args_with_default_check(["dart-decimate", "json", "--", "--help"]),
            values(&["dart-decimate", "check", "--format", "json", "--", "--help",])
        );
    }

    #[test]
    fn html_alias_opens_unless_stdout_is_requested() {
        assert_eq!(
            args_with_default_check(["dart-decimate", "html", "app"]),
            values(&[
                "dart-decimate",
                "check",
                "app",
                "--format",
                "html",
                "--open",
            ])
        );
        assert_eq!(
            args_with_default_check(["dart-decimate", "html", "app", "--stdout"]),
            values(&["dart-decimate", "check", "app", "--format", "html"])
        );
    }

    #[test]
    fn html_alias_does_not_duplicate_explicit_open() {
        assert_eq!(
            args_with_default_check(["dart-decimate", "html", "app", "--open"]),
            values(&[
                "dart-decimate",
                "check",
                "app",
                "--open",
                "--format",
                "html",
            ])
        );
        assert_eq!(
            args_with_default_check(["dart-decimate", "html", "--open", "--", "app"]),
            values(&[
                "dart-decimate",
                "check",
                "--open",
                "--format",
                "html",
                "--",
                "app",
            ])
        );
    }

    #[test]
    fn leading_format_values_do_not_block_default_check() {
        assert_eq!(
            args_with_default_check(["dart-decimate", "--format", "json"]),
            values(&["dart-decimate", "check", "--format", "json"])
        );
        assert_eq!(
            args_with_default_check(["dart-decimate", "--format", "html", "app"]),
            values(&["dart-decimate", "check", "--format", "html", "app"])
        );
        assert_eq!(
            args_with_default_check(["dart-decimate", "--format=json", "app"]),
            values(&["dart-decimate", "check", "--format=json", "app"])
        );
    }
}
