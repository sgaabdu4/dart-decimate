use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::de::{Error as DeError, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ScannedProject;
use crate::generated::is_generated_dart_path;
use crate::graph::normalize_against;
use crate::output::TRACE_SCHEMA_VERSION;

mod lex;
use lex::normalized_lines;
mod packages;
use packages::CopiedPackageFilter;

/// Dart duplicate-code detection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DuplicateMode {
    /// Exact token match after comments and whitespace are removed.
    Strict,
    /// Default mode. Equivalent to strict for the current Dart tokenizer.
    Mild,
    /// Normalize string literal values.
    Weak,
    /// Normalize string literals, numeric literals, and non-keyword identifiers.
    Semantic,
}

impl DuplicateMode {
    /// Parse a CLI mode value.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "strict" => Some(Self::Strict),
            "mild" => Some(Self::Mild),
            "weak" => Some(Self::Weak),
            "semantic" => Some(Self::Semantic),
            _ => None,
        }
    }
}

/// Duplicate-code detector options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateOptions {
    /// Detection mode.
    pub mode: DuplicateMode,
    /// Minimum normalized token count per clone window.
    pub min_tokens: usize,
    /// Minimum normalized line count per clone window; sparse windows may grow
    /// past this to satisfy `min_tokens`.
    pub min_lines: usize,
    /// Minimum occurrences before a clone group is reported.
    pub min_occurrences: usize,
    /// Report only clones that cross directory boundaries.
    pub skip_local: bool,
    /// Ignore import/export/part/augment directives.
    pub ignore_imports: bool,
    /// Limit output to the N largest clone groups.
    pub top: Option<usize>,
    /// Fail when duplicated lines exceed this percentage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold: Option<DuplicationThreshold>,
}

impl Default for DuplicateOptions {
    fn default() -> Self {
        Self {
            mode: DuplicateMode::Mild,
            min_tokens: 50,
            min_lines: 5,
            min_occurrences: 2,
            skip_local: false,
            ignore_imports: true,
            top: None,
            threshold: None,
        }
    }
}

/// Duplicate percentage threshold represented as percentage basis points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuplicationThreshold(u32);

impl DuplicationThreshold {
    /// Build a threshold from a human percentage in the inclusive range `0..=100`.
    ///
    /// # Errors
    ///
    /// Returns an error when the value is not finite or is outside `0..=100`.
    pub fn from_percent(value: f64) -> Result<Self, String> {
        if !value.is_finite() || !(0.0..=100.0).contains(&value) {
            return Err("threshold must be a percentage from 0 to 100".to_owned());
        }
        let scaled = (value * 100.0).round();
        let basis_points = scaled
            .to_string()
            .parse::<u32>()
            .map_err(|_| "threshold must be a percentage from 0 to 100".to_owned())?;
        Ok(Self(basis_points))
    }

    fn from_percent_u64(value: u64) -> Result<Self, String> {
        let percent = u32::try_from(value)
            .ok()
            .filter(|value| *value <= 100)
            .ok_or_else(|| "threshold must be a percentage from 0 to 100".to_owned())?;
        Ok(Self(percent * 100))
    }

    fn from_percent_i64(value: i64) -> Result<Self, String> {
        let value = u64::try_from(value)
            .map_err(|_| "threshold must be a percentage from 0 to 100".to_owned())?;
        Self::from_percent_u64(value)
    }

    /// Return the underlying percentage basis points.
    #[must_use]
    pub const fn basis_points(self) -> u32 {
        self.0
    }

    fn as_percent(self) -> f64 {
        f64::from(self.0) / 100.0
    }

    fn is_exceeded_by(self, percentage_basis_points: u32) -> bool {
        percentage_basis_points > self.0
    }
}

impl Serialize for DuplicationThreshold {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(self.as_percent())
    }
}

impl<'de> Deserialize<'de> for DuplicationThreshold {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ThresholdVisitor;

        impl Visitor<'_> for ThresholdVisitor {
            type Value = DuplicationThreshold;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a percentage from 0 to 100")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                DuplicationThreshold::from_percent_u64(value).map_err(E::custom)
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                DuplicationThreshold::from_percent_i64(value).map_err(E::custom)
            }

            fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                DuplicationThreshold::from_percent(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_any(ThresholdVisitor)
    }
}

/// Aggregate duplicate-code statistics.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateStats {
    /// Dart source lines included in duplicate analysis.
    pub analyzed_lines: usize,
    /// Unique source lines covered by at least one reported clone instance.
    pub duplicated_lines: usize,
    /// Duplicated line percentage in percentage basis points.
    pub duplication_percentage_basis_points: u32,
    /// Whether the configured duplicate threshold was exceeded.
    pub threshold_exceeded: bool,
}

/// Code duplication report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateCodeReport {
    /// Options used to compute the report.
    pub options: DuplicateOptions,
    /// Aggregate duplicate-code statistics.
    pub stats: DuplicateStats,
    /// Reported clone groups.
    pub clone_groups: Vec<CodeClone>,
}

/// One duplicated Dart code block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeClone {
    /// Stable clone fingerprint.
    pub fingerprint: String,
    /// Matching block instances.
    pub instances: Vec<CodeCloneInstance>,
    /// Normalized lines in the matched duplicated block.
    pub line_count: usize,
    /// Normalized tokens in the matched duplicated block.
    pub token_count: usize,
}

/// One clone group instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeCloneInstance {
    /// Dart file path.
    pub path: PathBuf,
    /// 1-based start line.
    pub start_line: usize,
    /// 1-based end line.
    pub end_line: usize,
    /// 0-based byte column.
    pub column: usize,
}

/// Trace-clone JSON envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloneTraceReport {
    /// Schema identifier.
    pub schema_version: String,
    /// Typed JSON envelope discriminator.
    pub kind: String,
    /// Tool name.
    pub tool: String,
    /// Trace command name.
    pub command: String,
    /// Trace selector supplied by the caller.
    pub trace: String,
    /// Whether at least one clone group matched.
    pub found: bool,
    /// Matching clone groups.
    pub clone_groups: Vec<TraceCloneGroup>,
    /// Short trace interpretation.
    pub reason: String,
}

/// Clone group included in trace output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceCloneGroup {
    /// Stable clone fingerprint.
    pub fingerprint: String,
    /// Normalized lines in the matched duplicated block.
    pub line_count: usize,
    /// Normalized tokens in the matched duplicated block.
    pub token_count: usize,
    /// Matching block instances.
    pub instances: Vec<TraceCloneInstance>,
    /// Suggested extraction target.
    pub suggestion: String,
}

/// Clone instance included in trace output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceCloneInstance {
    /// Dart file path, root-relative where possible.
    pub path: String,
    /// 1-based start line.
    pub start_line: usize,
    /// 1-based end line.
    pub end_line: usize,
    /// 0-based byte column.
    pub column: usize,
}

/// Errors returned while detecting duplicated code.
#[derive(Debug, Error)]
pub enum DuplicateCodeError {
    /// A Dart file could not be read.
    #[error("failed to read Dart file {path}: {source}")]
    ReadFile {
        /// File path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CloneOccurrence {
    path: PathBuf,
    start_line: usize,
    end_line: usize,
    column: usize,
    parent: PathBuf,
    line_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CloneWindow {
    start: usize,
    end: usize,
    token_count: usize,
}

/// Detect duplicated Dart code blocks.
///
/// Clone windows satisfy both `min_lines` and `min_tokens`; sparse duplicated
/// blocks can therefore span more than `min_lines`. Identical matches across
/// copied local Pub packages with the same package name are ignored.
///
/// # Errors
///
/// Returns [`DuplicateCodeError`] if a scanned Dart file cannot be read.
pub fn detect_duplicates(
    project: &ScannedProject,
    options: &DuplicateOptions,
) -> Result<DuplicateCodeReport, DuplicateCodeError> {
    let mut by_fingerprint = BTreeMap::<String, Vec<(CloneOccurrence, usize)>>::new();
    let mut analyzed_lines = 0usize;

    for file in &project.files {
        let path = normalize_against(&project.root, &file.path);
        if !path.starts_with(&project.root) || is_ignored_path(&path) {
            continue;
        }
        let source = fs::read_to_string(&path).map_err(|source| DuplicateCodeError::ReadFile {
            path: path.clone(),
            source,
        })?;
        analyzed_lines += source.lines().count();
        let lines = normalized_lines(&source, options);
        if lines.len() < options.min_lines {
            continue;
        }

        for clone_window in clone_windows(&lines, options) {
            let window = &lines[clone_window.start..=clone_window.end];
            let text = window
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let Some(first) = window.first() else {
                continue;
            };
            let Some(last) = window.last() else {
                continue;
            };
            let fingerprint = fingerprint(&text);
            by_fingerprint.entry(fingerprint).or_default().push((
                CloneOccurrence {
                    parent: path.parent().map_or_else(PathBuf::new, Path::to_path_buf),
                    path: path.clone(),
                    start_line: first.line,
                    end_line: last.line,
                    column: first.column,
                    line_count: window.len(),
                },
                clone_window.token_count,
            ));
        }
    }

    let mut clone_groups = by_fingerprint
        .into_iter()
        .filter_map(|(fingerprint, occurrences)| {
            clone_group_from_occurrences(&fingerprint, occurrences, options)
        })
        .collect::<Vec<_>>();

    clone_groups.sort_by(|left, right| {
        (
            std::cmp::Reverse(left.instances.len()),
            std::cmp::Reverse(left.line_count),
            &left.instances[0].path,
            left.instances[0].start_line,
            &left.fingerprint,
        )
            .cmp(&(
                std::cmp::Reverse(right.instances.len()),
                std::cmp::Reverse(right.line_count),
                &right.instances[0].path,
                right.instances[0].start_line,
                &right.fingerprint,
            ))
    });
    clone_groups = collapse_overlapping_groups(clone_groups);
    let mut copied_packages = CopiedPackageFilter::new(&project.root);
    clone_groups.retain(|group| !copied_packages.is_copied_package_clone(group));
    let stats = duplicate_stats(analyzed_lines, &clone_groups, options.threshold);
    if let Some(top) = options.top {
        clone_groups.truncate(top);
    }

    Ok(DuplicateCodeReport {
        options: options.clone(),
        stats,
        clone_groups,
    })
}

/// Trace clone groups by fingerprint or `FILE:LINE`.
#[must_use]
pub fn trace_clone(
    project: &ScannedProject,
    report: &DuplicateCodeReport,
    trace: &str,
) -> CloneTraceReport {
    let clone_groups = report
        .clone_groups
        .iter()
        .filter(|group| clone_matches(project, group, trace))
        .map(|group| trace_group(&project.root, group))
        .collect::<Vec<_>>();
    let found = !clone_groups.is_empty();

    CloneTraceReport {
        schema_version: TRACE_SCHEMA_VERSION.to_owned(),
        kind: "trace-clone".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: "trace-clone".to_owned(),
        trace: trace.to_owned(),
        found,
        reason: if found {
            "clone trace matched one or more duplicate code groups"
        } else {
            "clone trace did not match any duplicate code group"
        }
        .to_owned(),
        clone_groups,
    }
}

/// Render a concise human clone trace.
#[must_use]
pub fn render_clone_trace(report: &CloneTraceReport) -> String {
    format!(
        "trace-clone {}: found={} groups={}\n{}\n",
        report.trace,
        report.found,
        report.clone_groups.len(),
        report.reason
    )
}

fn collapse_overlapping_groups(groups: Vec<CodeClone>) -> Vec<CodeClone> {
    let mut collapsed = Vec::new();
    for group in groups {
        if !collapsed
            .iter()
            .any(|existing| groups_overlap(existing, &group))
        {
            collapsed.push(group);
        }
    }
    collapsed
}

fn groups_overlap(left: &CodeClone, right: &CodeClone) -> bool {
    right.instances.iter().all(|right_instance| {
        left.instances.iter().any(|left_instance| {
            left_instance.path == right_instance.path
                && ranges_overlap(
                    left_instance.start_line,
                    left_instance.end_line,
                    right_instance.start_line,
                    right_instance.end_line,
                )
        })
    })
}

fn ranges_overlap(
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
) -> bool {
    left_start <= right_end && right_start <= left_end
}

fn clone_windows(lines: &[lex::NormalizedLine], options: &DuplicateOptions) -> Vec<CloneWindow> {
    let min_lines = options.min_lines.max(1);
    let mut windows = Vec::new();
    let mut end = 0;
    let mut token_count = 0;

    for start in 0..lines.len() {
        while end < lines.len() && (end - start < min_lines || token_count < options.min_tokens) {
            token_count += lines[end].token_count;
            end += 1;
        }

        if end - start >= min_lines && token_count >= options.min_tokens {
            windows.push(CloneWindow {
                start,
                end: end - 1,
                token_count,
            });
        }

        token_count -= lines[start].token_count;
    }

    windows
}

fn duplicate_stats(
    analyzed_lines: usize,
    clone_groups: &[CodeClone],
    threshold: Option<DuplicationThreshold>,
) -> DuplicateStats {
    let mut duplicated = BTreeSet::<(PathBuf, usize)>::new();
    for group in clone_groups {
        for instance in &group.instances {
            for line in instance.start_line..=instance.end_line {
                duplicated.insert((instance.path.clone(), line));
            }
        }
    }
    let duplicated_lines = duplicated.len();
    let duplication_percentage_basis_points = if analyzed_lines == 0 {
        0
    } else {
        let raw = ((duplicated_lines as u128) * 10_000) / (analyzed_lines as u128);
        u32::try_from(raw).unwrap_or(u32::MAX)
    };
    let threshold_exceeded = threshold
        .is_some_and(|threshold| threshold.is_exceeded_by(duplication_percentage_basis_points));

    DuplicateStats {
        analyzed_lines,
        duplicated_lines,
        duplication_percentage_basis_points,
        threshold_exceeded,
    }
}

fn clone_group_from_occurrences(
    fingerprint: &str,
    occurrences: Vec<(CloneOccurrence, usize)>,
    options: &DuplicateOptions,
) -> Option<CodeClone> {
    let mut seen = BTreeSet::<(PathBuf, usize, usize)>::new();
    let mut instances = Vec::new();
    let mut token_count = 0;
    let mut line_count = 0;
    let mut parents = BTreeSet::new();

    for (occurrence, tokens) in occurrences {
        if !seen.insert((
            occurrence.path.clone(),
            occurrence.start_line,
            occurrence.end_line,
        )) {
            continue;
        }
        parents.insert(occurrence.parent);
        token_count = token_count.max(tokens);
        line_count = line_count.max(occurrence.line_count);
        instances.push(CodeCloneInstance {
            path: occurrence.path,
            start_line: occurrence.start_line,
            end_line: occurrence.end_line,
            column: occurrence.column,
        });
    }

    if instances.len() < options.min_occurrences || (options.skip_local && parents.len() < 2) {
        return None;
    }
    instances.sort_by(|left, right| {
        (&left.path, left.start_line, left.end_line).cmp(&(
            &right.path,
            right.start_line,
            right.end_line,
        ))
    });

    Some(CodeClone {
        fingerprint: fingerprint.to_owned(),
        instances,
        line_count,
        token_count,
    })
}

fn is_ignored_path(path: &Path) -> bool {
    if is_generated_dart_path(path) {
        return true;
    }

    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some("test" | "integration_test" | "test_driver" | "__tests__" | "__mocks__")
        )
    })
}

fn fingerprint(text: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("dup:{:08x}", hash & 0xffff_ffff)
}

fn clone_matches(project: &ScannedProject, group: &CodeClone, trace: &str) -> bool {
    if trace == group.fingerprint {
        return true;
    }
    let Some((path, line)) = parse_file_line(trace) else {
        return false;
    };
    let path = normalize_against(&project.root, &path);
    group.instances.iter().any(|instance| {
        instance.path == path && line >= instance.start_line && line <= instance.end_line
    })
}

fn parse_file_line(trace: &str) -> Option<(PathBuf, usize)> {
    let (path, line) = trace.rsplit_once(':')?;
    Some((PathBuf::from(path), line.parse().ok()?))
}

fn trace_group(root: &Path, group: &CodeClone) -> TraceCloneGroup {
    TraceCloneGroup {
        fingerprint: group.fingerprint.clone(),
        line_count: group.line_count,
        token_count: group.token_count,
        instances: group
            .instances
            .iter()
            .map(|instance| TraceCloneInstance {
                path: display_path(root, &instance.path),
                start_line: instance.start_line,
                end_line: instance.end_line,
                column: instance.column,
            })
            .collect(),
        suggestion: format!(
            "Extract the {} duplicated lines into one shared function, method, widget, or mixin owner",
            group.line_count
        ),
    }
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests;
