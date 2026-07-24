use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::generated::is_generated_dart_path;
use crate::graph::normalize_against;
use crate::{Location, ScannedProject};

/// Feature flag detector options.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureFlagOptions {
    /// Limit output to the N most frequently referenced flags.
    pub top: Option<usize>,
    /// Flag names kept in inventory but acknowledged as intentional.
    #[serde(default)]
    pub allow: BTreeSet<String>,
}

/// Feature flag report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureFlagReport {
    /// Options used to compute this report.
    pub options: FeatureFlagOptions,
    /// Dart files included in flag detection.
    pub analyzed_files: usize,
    /// Grouped feature flags.
    pub flags: Vec<FeatureFlag>,
    /// Raw feature flag occurrence count.
    pub total_occurrences: usize,
}

/// One grouped feature flag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureFlag {
    /// Flag key/name.
    pub name: String,
    /// Detection source category.
    pub source: FeatureFlagSource,
    /// Provider or platform surface.
    pub provider: String,
    /// Detection confidence.
    pub confidence: FeatureFlagConfidence,
    /// Occurrences for this flag.
    pub occurrences: Vec<FeatureFlagOccurrence>,
}

/// Feature flag source category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FeatureFlagSource {
    /// Dart compilation configuration environment.
    CompileTimeEnvironment,
    /// Native process environment.
    ProcessEnvironment,
    /// Feature flag SDK or service call.
    SdkCall,
}

/// Detection confidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FeatureFlagConfidence {
    /// Low-confidence heuristic.
    Low,
    /// Medium-confidence heuristic.
    Medium,
    /// High-confidence known flag surface.
    High,
}

/// One feature flag occurrence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureFlagOccurrence {
    /// Dart file path.
    pub path: PathBuf,
    /// Location of the call/property access.
    pub location: Location,
    /// Matched expression or API surface.
    pub expression: String,
}

/// Errors returned while detecting feature flags.
#[derive(Debug, Error)]
pub enum FeatureFlagError {
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
struct FlagGroup {
    name: String,
    source: FeatureFlagSource,
    provider: String,
    confidence: FeatureFlagConfidence,
    occurrences: Vec<FeatureFlagOccurrence>,
}

/// Detect Dart and Flutter feature flag patterns.
///
/// # Errors
///
/// Returns [`FeatureFlagError`] if a scanned Dart file cannot be read.
pub fn detect_feature_flags(
    project: &ScannedProject,
    options: &FeatureFlagOptions,
) -> Result<FeatureFlagReport, FeatureFlagError> {
    let mut groups = BTreeMap::<(String, FeatureFlagSource, String), FlagGroup>::new();
    let mut analyzed_files = 0;

    for file in &project.files {
        let path = normalize_against(&project.root, &file.path);
        if !path.starts_with(&project.root) || is_generated_dart_path(&path) {
            continue;
        }
        analyzed_files += 1;
        let source = fs::read_to_string(&path).map_err(|source| FeatureFlagError::ReadFile {
            path: path.clone(),
            source,
        })?;
        for detected in detect_in_source(&path, &source) {
            let key = (
                detected.name.clone(),
                detected.source,
                detected.provider.clone(),
            );
            let group = groups.entry(key).or_insert_with(|| FlagGroup {
                name: detected.name.clone(),
                source: detected.source,
                provider: detected.provider.clone(),
                confidence: detected.confidence,
                occurrences: Vec::new(),
            });
            group.confidence = group.confidence.max(detected.confidence);
            group.occurrences.push(detected.occurrence);
        }
    }

    let total_occurrences = groups
        .values()
        .map(|group| group.occurrences.len())
        .sum::<usize>();
    let mut flags = groups
        .into_values()
        .map(FeatureFlag::from)
        .collect::<Vec<_>>();
    flags.sort_by(|left, right| {
        (
            std::cmp::Reverse(left.occurrences.len()),
            &left.name,
            left.source,
            &left.provider,
        )
            .cmp(&(
                std::cmp::Reverse(right.occurrences.len()),
                &right.name,
                right.source,
                &right.provider,
            ))
    });
    if let Some(top) = options.top {
        flags.truncate(top);
    }

    Ok(FeatureFlagReport {
        options: options.clone(),
        analyzed_files,
        flags,
        total_occurrences,
    })
}

impl From<FlagGroup> for FeatureFlag {
    fn from(group: FlagGroup) -> Self {
        let mut seen = BTreeSet::new();
        let mut occurrences = group
            .occurrences
            .into_iter()
            .filter(|occurrence| {
                seen.insert((
                    occurrence.path.clone(),
                    occurrence.location.line,
                    occurrence.location.column,
                    occurrence.expression.clone(),
                ))
            })
            .collect::<Vec<_>>();
        occurrences.sort_by(|left, right| {
            (&left.path, left.location.line, left.location.column).cmp(&(
                &right.path,
                right.location.line,
                right.location.column,
            ))
        });
        Self {
            name: group.name,
            source: group.source,
            provider: group.provider,
            confidence: group.confidence,
            occurrences,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DetectedFlag {
    name: String,
    source: FeatureFlagSource,
    provider: String,
    confidence: FeatureFlagConfidence,
    occurrence: FeatureFlagOccurrence,
}

struct DetectionSpec<'a> {
    name: String,
    source_kind: FeatureFlagSource,
    provider: &'a str,
    confidence: FeatureFlagConfidence,
    expression: &'a str,
}

fn detect_in_source(path: &Path, source: &str) -> Vec<DetectedFlag> {
    let mut flags = Vec::new();
    detect_compile_time_environment(path, source, &mut flags);
    detect_platform_environment(path, source, &mut flags);
    detect_firebase_remote_config(path, source, &mut flags);
    detect_launchdarkly(path, source, &mut flags);
    flags
}

fn detect_compile_time_environment(path: &Path, source: &str, flags: &mut Vec<DetectedFlag>) {
    for pattern in [
        "bool.fromEnvironment(",
        "bool.hasEnvironment(",
        "String.fromEnvironment(",
        "int.fromEnvironment(",
    ] {
        for (index, name) in string_args_for_pattern(source, pattern) {
            let bool_env = pattern.starts_with("bool.");
            if bool_env || is_flag_like_name(&name) {
                flags.push(detected(
                    path,
                    source,
                    index,
                    DetectionSpec {
                        name,
                        source_kind: FeatureFlagSource::CompileTimeEnvironment,
                        provider: "dart:core",
                        confidence: if bool_env {
                            FeatureFlagConfidence::High
                        } else {
                            FeatureFlagConfidence::Medium
                        },
                        expression: pattern.trim_end_matches('('),
                    },
                ));
            }
        }
    }
}

fn detect_platform_environment(path: &Path, source: &str, flags: &mut Vec<DetectedFlag>) {
    for pattern in ["Platform.environment[", "Platform.environment.containsKey("] {
        for (index, name) in string_args_for_pattern(source, pattern) {
            if is_flag_like_name(&name) {
                flags.push(detected(
                    path,
                    source,
                    index,
                    DetectionSpec {
                        name,
                        source_kind: FeatureFlagSource::ProcessEnvironment,
                        provider: "dart:io",
                        confidence: FeatureFlagConfidence::High,
                        expression: pattern.trim_end_matches(['(', '[']),
                    },
                ));
            }
        }
    }
}

fn detect_firebase_remote_config(path: &Path, source: &str, flags: &mut Vec<DetectedFlag>) {
    if !source.contains("FirebaseRemoteConfig") && !source.contains("remoteConfig.") {
        return;
    }
    for receiver in ["remoteConfig.", "FirebaseRemoteConfig.instance."] {
        for method in ["getBool(", "getString(", "getInt(", "getDouble("] {
            let pattern = format!("{receiver}{method}");
            for (index, name) in string_args_for_pattern(source, &pattern) {
                flags.push(detected(
                    path,
                    source,
                    index,
                    DetectionSpec {
                        name,
                        source_kind: FeatureFlagSource::SdkCall,
                        provider: "firebase_remote_config",
                        confidence: FeatureFlagConfidence::High,
                        expression: pattern.trim_end_matches('('),
                    },
                ));
            }
        }
    }
}

fn detect_launchdarkly(path: &Path, source: &str, flags: &mut Vec<DetectedFlag>) {
    let launchdarkly_context = source.contains("LaunchDarkly") || source.contains("LDClient");
    for method in [
        ".boolVariation(",
        ".stringVariation(",
        ".intVariation(",
        ".doubleVariation(",
        ".jsonVariation(",
        ".variation(",
    ] {
        if method == ".variation(" && !launchdarkly_context {
            continue;
        }
        for (index, name) in string_args_for_pattern(source, method) {
            flags.push(detected(
                path,
                source,
                index,
                DetectionSpec {
                    name,
                    source_kind: FeatureFlagSource::SdkCall,
                    provider: "launchdarkly",
                    confidence: if launchdarkly_context {
                        FeatureFlagConfidence::High
                    } else {
                        FeatureFlagConfidence::Medium
                    },
                    expression: method.trim_start_matches('.').trim_end_matches('('),
                },
            ));
        }
    }
}

fn detected(path: &Path, source: &str, index: usize, spec: DetectionSpec<'_>) -> DetectedFlag {
    DetectedFlag {
        name: spec.name,
        source: spec.source_kind,
        provider: spec.provider.to_owned(),
        confidence: spec.confidence,
        occurrence: FeatureFlagOccurrence {
            path: path.to_path_buf(),
            location: location_for_index(source, index),
            expression: spec.expression.to_owned(),
        },
    }
}

fn string_args_for_pattern(source: &str, pattern: &str) -> Vec<(usize, String)> {
    let mut matches = Vec::new();
    let mut offset = 0;
    while let Some(relative) = source[offset..].find(pattern) {
        let index = offset + relative;
        offset = index + pattern.len();
        if is_comment_match(source, index) {
            continue;
        }
        if let Some((name, _literal_index)) = first_string_literal(source, offset) {
            matches.push((index, name));
        }
    }
    matches
}

fn first_string_literal(source: &str, mut index: usize) -> Option<(String, usize)> {
    let bytes = source.as_bytes();
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    if index < bytes.len()
        && matches!(bytes[index], b'r' | b'R')
        && index + 1 < bytes.len()
        && matches!(bytes[index + 1], b'\'' | b'"')
    {
        index += 1;
    }
    if index >= bytes.len() || !matches!(bytes[index], b'\'' | b'"') {
        return None;
    }
    let quote = bytes[index];
    let literal_start = index;
    index += 1;
    let value_start = index;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index = (index + 2).min(bytes.len());
            continue;
        }
        if bytes[index] == quote {
            return Some((source[value_start..index].to_owned(), literal_start));
        }
        index += 1;
    }
    None
}

fn is_comment_match(source: &str, index: usize) -> bool {
    let line_start = source[..index]
        .rfind('\n')
        .map_or(0, |position| position + 1);
    if source[line_start..index].contains("//") {
        return true;
    }
    let last_block_open = source[..index].rfind("/*");
    let last_block_close = source[..index].rfind("*/");
    last_block_open.is_some_and(|open| last_block_close.is_none_or(|close| open > close))
}

fn location_for_index(source: &str, index: usize) -> Location {
    let line = source[..index]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1;
    let column = source[..index]
        .rfind('\n')
        .map_or(index, |position| index - position - 1);
    Location { line, column }
}

fn is_flag_like_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    let lower = name.to_ascii_lowercase();
    upper.starts_with("FEATURE_")
        || upper.starts_with("ENABLE_")
        || upper.starts_with("DISABLE_")
        || upper.starts_with("EXPERIMENT_")
        || upper.starts_with("FLAG_")
        || upper.starts_with("FF_")
        || lower.contains("feature")
        || lower.contains("flag")
        || lower.contains("enable")
        || lower.contains("experiment")
        || lower.contains("toggle")
        || lower.contains("beta")
        || lower.contains("rollout")
        || lower.contains("logging")
}

#[cfg(test)]
mod tests;
