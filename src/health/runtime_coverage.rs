use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::graph::normalize_against;
use crate::{ScannedProject, health::is_ignored_path};

use super::FunctionMetrics;
use super::runtime_intelligence::runtime_intelligence;
use super::types::{
    HealthError, HealthOptions, LowTrafficThreshold, RuntimeCoverageConfidence,
    RuntimeCoverageFinding, RuntimeCoverageFindingKind, RuntimeCoverageFormat,
    RuntimeCoverageReport, RuntimeHotPath, SourceMapConfidence,
};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Debug, Default)]
pub(super) struct RuntimeCoverageAccumulator {
    pub(super) files: BTreeMap<PathBuf, RuntimeFileCoverage>,
    pub(super) formats: BTreeSet<RuntimeCoverageFormat>,
    pub(super) warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeFileCoverage {
    pub(super) invocations: usize,
    pub(super) source_map_confidence: SourceMapConfidence,
    pub(super) functions: Vec<RuntimeFunctionCoverage>,
}

#[derive(Debug, Clone)]
pub(super) struct RuntimeFunctionCoverage {
    pub(super) symbol: Option<String>,
    pub(super) line: Option<usize>,
    pub(super) invocations: usize,
}

pub(super) fn load_runtime_coverage(
    project: &ScannedProject,
    options: &HealthOptions,
    functions: &[FunctionMetrics],
) -> Result<Option<RuntimeCoverageReport>, HealthError> {
    let Some(path) = &options.runtime_coverage_path else {
        return Ok(None);
    };
    let source_path = normalize_against(&project.root, path);
    let mut hash = FNV_OFFSET;
    let mut accumulator = RuntimeCoverageAccumulator::default();
    for file in runtime_coverage_files(&source_path)? {
        let source =
            fs::read_to_string(&file).map_err(|source| HealthError::ReadRuntimeCoverage {
                path: file.clone(),
                source,
            })?;
        update_hash(&mut hash, file.to_string_lossy().as_bytes());
        update_hash(&mut hash, source.as_bytes());
        parse_runtime_coverage_file(project, &file, &source, &mut accumulator)?;
    }

    Ok(Some(build_report(
        project,
        options,
        functions,
        source_path,
        hash,
        accumulator,
    )))
}

fn runtime_coverage_files(path: &Path) -> Result<Vec<PathBuf>, HealthError> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    if !path.is_dir() {
        return Err(HealthError::ReadRuntimeCoverage {
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "runtime coverage path was not found",
            ),
        });
    }

    let mut files = Vec::new();
    collect_json_files(path, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_json_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), HealthError> {
    let entries = fs::read_dir(path).map_err(|source| HealthError::ReadRuntimeCoverage {
        path: path.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| HealthError::ReadRuntimeCoverage {
            path: path.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(&path, files)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            files.push(path);
        }
    }
    Ok(())
}

fn parse_runtime_coverage_file(
    project: &ScannedProject,
    path: &Path,
    source: &str,
    accumulator: &mut RuntimeCoverageAccumulator,
) -> Result<(), HealthError> {
    let value = serde_json::from_str::<Value>(source).map_err(|error| {
        HealthError::ParseRuntimeCoverage {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;

    if is_v8_coverage(&value) {
        accumulator.formats.insert(RuntimeCoverageFormat::V8);
        parse_v8_coverage(project, &value, accumulator);
    } else if is_istanbul_coverage(&value) {
        accumulator.formats.insert(RuntimeCoverageFormat::Istanbul);
        parse_istanbul_coverage(project, &value, accumulator);
    } else {
        accumulator.warnings.push(format!(
            "{} did not look like V8 or Istanbul runtime coverage",
            path.display()
        ));
    }
    Ok(())
}

fn is_v8_coverage(value: &Value) -> bool {
    value.get("result").is_some_and(Value::is_array)
        || value.as_array().is_some_and(|items| {
            items
                .iter()
                .any(|item| item.get("url").is_some() && item.get("functions").is_some())
        })
}

fn is_istanbul_coverage(value: &Value) -> bool {
    value.as_object().is_some_and(|coverage| {
        coverage.values().any(|entry| {
            entry.get("s").is_some()
                || entry.get("statementMap").is_some()
                || entry.get("fnMap").is_some()
        })
    })
}

fn parse_v8_coverage(
    project: &ScannedProject,
    value: &Value,
    accumulator: &mut RuntimeCoverageAccumulator,
) {
    let scripts = value
        .get("result")
        .and_then(Value::as_array)
        .or_else(|| value.as_array());
    let Some(scripts) = scripts else {
        return;
    };

    for script in scripts {
        let Some(raw_path) = script.get("url").and_then(Value::as_str) else {
            continue;
        };
        let Some((path, confidence)) = runtime_path(project, raw_path) else {
            continue;
        };
        let functions = script
            .get("functions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|function| v8_function_coverage(&path, function))
            .collect::<Vec<_>>();
        let invocations = functions
            .iter()
            .map(|function| function.invocations)
            .sum::<usize>();
        if invocations > 0 {
            add_file_coverage(accumulator, path, invocations, confidence, functions);
        }
    }
}

fn v8_function_coverage(path: &Path, function: &Value) -> Option<RuntimeFunctionCoverage> {
    let ranges = function.get("ranges").and_then(Value::as_array)?;
    let invocations = ranges
        .iter()
        .filter_map(|range| range.get("count"))
        .map(value_count)
        .max()
        .unwrap_or(0);
    if invocations == 0 {
        return None;
    }
    let line = ranges
        .first()
        .and_then(|range| range.get("startOffset"))
        .and_then(Value::as_u64)
        .and_then(|offset| usize::try_from(offset).ok())
        .and_then(|offset| line_for_offset(path, offset));
    Some(RuntimeFunctionCoverage {
        symbol: function
            .get("functionName")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .map(str::to_owned),
        line,
        invocations,
    })
}

fn parse_istanbul_coverage(
    project: &ScannedProject,
    value: &Value,
    accumulator: &mut RuntimeCoverageAccumulator,
) {
    let Some(entries) = value.as_object() else {
        return;
    };

    for (key, entry) in entries {
        let raw_path = entry.get("path").and_then(Value::as_str).unwrap_or(key);
        let Some((path, confidence)) = runtime_path(project, raw_path) else {
            continue;
        };
        let statement_invocations = entry
            .get("s")
            .and_then(Value::as_object)
            .map_or(0, count_map_invocations);
        let functions = istanbul_functions(entry);
        let function_invocations = functions
            .iter()
            .map(|function| function.invocations)
            .sum::<usize>();
        let invocations = statement_invocations.max(function_invocations);
        if invocations > 0 {
            add_file_coverage(accumulator, path, invocations, confidence, functions);
        }
    }
}

fn istanbul_functions(entry: &Value) -> Vec<RuntimeFunctionCoverage> {
    let Some(function_counts) = entry.get("f").and_then(Value::as_object) else {
        return Vec::new();
    };
    let function_map = entry.get("fnMap").and_then(Value::as_object);
    function_counts
        .iter()
        .filter_map(|(id, count)| {
            let invocations = value_count(count);
            if invocations == 0 {
                return None;
            }
            let metadata = function_map.and_then(|items| items.get(id));
            Some(RuntimeFunctionCoverage {
                symbol: metadata
                    .and_then(|item| item.get("name"))
                    .and_then(Value::as_str)
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned),
                line: istanbul_line(metadata),
                invocations,
            })
        })
        .collect()
}

fn istanbul_line(metadata: Option<&Value>) -> Option<usize> {
    metadata
        .and_then(|item| item.get("decl").or_else(|| item.get("loc")))
        .and_then(|location| location.get("start"))
        .and_then(|start| start.get("line"))
        .and_then(Value::as_u64)
        .and_then(|line| usize::try_from(line).ok())
}

fn add_file_coverage(
    accumulator: &mut RuntimeCoverageAccumulator,
    path: PathBuf,
    invocations: usize,
    confidence: SourceMapConfidence,
    functions: Vec<RuntimeFunctionCoverage>,
) {
    accumulator
        .files
        .entry(path)
        .and_modify(|file| {
            file.invocations += invocations;
            file.source_map_confidence = best_confidence(file.source_map_confidence, confidence);
            file.functions.extend(functions.clone());
        })
        .or_insert(RuntimeFileCoverage {
            invocations,
            source_map_confidence: confidence,
            functions,
        });
}

fn build_report(
    project: &ScannedProject,
    options: &HealthOptions,
    functions: &[FunctionMetrics],
    source_path: PathBuf,
    source_hash: u64,
    accumulator: RuntimeCoverageAccumulator,
) -> RuntimeCoverageReport {
    let total_invocations = accumulator
        .files
        .values()
        .map(|file| file.invocations)
        .sum::<usize>();
    let hot_paths = hot_paths(&accumulator, options.min_invocations_hot);
    let findings = runtime_findings(project, &accumulator, total_invocations, options);
    let intelligence = runtime_intelligence(
        project,
        options,
        functions,
        &accumulator,
        total_invocations,
        &hot_paths,
        &findings,
    );

    RuntimeCoverageReport {
        source_path,
        source_format: source_format(&accumulator.formats),
        source_hash: format!("{source_hash:016x}"),
        observed_files: accumulator.files.len(),
        total_invocations,
        min_invocations_hot: options.min_invocations_hot,
        min_observation_volume: options.min_observation_volume,
        low_traffic_threshold: options.low_traffic_threshold,
        hot_paths,
        findings,
        coverage_intelligence: intelligence.coverage_intelligence,
        blast_radius: intelligence.blast_radius,
        importance: intelligence.importance,
        warnings: accumulator.warnings,
    }
}

fn source_format(formats: &BTreeSet<RuntimeCoverageFormat>) -> RuntimeCoverageFormat {
    if formats.len() == 1 {
        *formats
            .iter()
            .next()
            .unwrap_or(&RuntimeCoverageFormat::Mixed)
    } else {
        RuntimeCoverageFormat::Mixed
    }
}

fn hot_paths(
    accumulator: &RuntimeCoverageAccumulator,
    min_invocations_hot: usize,
) -> Vec<RuntimeHotPath> {
    let mut paths = Vec::new();
    for (path, file) in &accumulator.files {
        let mut file_hot_functions = file
            .functions
            .iter()
            .filter(|function| function.invocations >= min_invocations_hot)
            .map(|function| RuntimeHotPath {
                path: path.clone(),
                line: function.line,
                symbol: function.symbol.clone(),
                invocations: function.invocations,
                source_map_confidence: file.source_map_confidence,
            })
            .collect::<Vec<_>>();
        if file_hot_functions.is_empty() && file.invocations >= min_invocations_hot {
            file_hot_functions.push(RuntimeHotPath {
                path: path.clone(),
                line: None,
                symbol: None,
                invocations: file.invocations,
                source_map_confidence: file.source_map_confidence,
            });
        }
        paths.extend(file_hot_functions);
    }
    paths.sort_by(|left, right| {
        right
            .invocations
            .cmp(&left.invocations)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.symbol.cmp(&right.symbol))
    });
    paths
}

fn runtime_findings(
    project: &ScannedProject,
    accumulator: &RuntimeCoverageAccumulator,
    total_invocations: usize,
    options: &HealthOptions,
) -> Vec<RuntimeCoverageFinding> {
    let mut findings = low_traffic_findings(accumulator, total_invocations, options);
    findings.extend(coverage_unavailable_findings(
        project,
        accumulator,
        total_invocations,
        options,
    ));
    findings.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.kind.cmp(&right.kind))
    });
    findings
}

fn low_traffic_findings(
    accumulator: &RuntimeCoverageAccumulator,
    total_invocations: usize,
    options: &HealthOptions,
) -> Vec<RuntimeCoverageFinding> {
    if total_invocations == 0 {
        return Vec::new();
    }
    accumulator
        .files
        .iter()
        .filter_map(|(path, file)| {
            let traffic = LowTrafficThreshold::from_fraction(file.invocations, total_invocations);
            (traffic <= options.low_traffic_threshold).then(|| RuntimeCoverageFinding {
                path: path.clone(),
                kind: RuntimeCoverageFindingKind::LowTraffic,
                line: 1,
                invocations: file.invocations,
                traffic_fraction: traffic,
                safe_to_delete: false,
                review_required: true,
                confidence: observation_confidence(total_invocations, options),
                reason: "runtime coverage observed this file below the low-traffic threshold"
                    .to_owned(),
            })
        })
        .collect()
}

fn coverage_unavailable_findings(
    project: &ScannedProject,
    accumulator: &RuntimeCoverageAccumulator,
    total_invocations: usize,
    options: &HealthOptions,
) -> Vec<RuntimeCoverageFinding> {
    project
        .files
        .iter()
        .map(|file| normalize_against(&project.root, &file.path))
        .filter(|path| path.starts_with(&project.root) && !is_ignored_path(path))
        .filter(|path| !accumulator.files.contains_key(path))
        .map(|path| RuntimeCoverageFinding {
            path,
            kind: RuntimeCoverageFindingKind::CoverageUnavailable,
            line: 1,
            invocations: 0,
            traffic_fraction: LowTrafficThreshold::from_ratio(0.0),
            safe_to_delete: false,
            review_required: true,
            confidence: observation_confidence(total_invocations, options),
            reason: "runtime coverage did not include this scanned Dart file".to_owned(),
        })
        .collect()
}

fn observation_confidence(
    total_invocations: usize,
    options: &HealthOptions,
) -> RuntimeCoverageConfidence {
    if total_invocations == 0 {
        RuntimeCoverageConfidence::Low
    } else if total_invocations >= options.min_observation_volume {
        RuntimeCoverageConfidence::High
    } else {
        RuntimeCoverageConfidence::Medium
    }
}

fn runtime_path(
    project: &ScannedProject,
    raw_path: &str,
) -> Option<(PathBuf, SourceMapConfidence)> {
    let decoded = decode_runtime_path(raw_path);
    if decoded.is_empty() || !decoded.contains(".dart") {
        return None;
    }
    let path = if let Some(rest) = decoded.strip_prefix("package:") {
        let (_, package_path) = rest.split_once('/')?;
        project.root.join("lib").join(package_path)
    } else if let Some(rest) = decoded.strip_prefix("org-dartlang-app:///") {
        project.root.join(rest.trim_start_matches('/'))
    } else if let Some(rest) = decoded.strip_prefix("file://") {
        PathBuf::from(rest)
    } else if decoded.starts_with("http://") || decoded.starts_with("https://") {
        let lib_path = decoded.split("/lib/").nth(1)?;
        project.root.join("lib").join(lib_path)
    } else {
        PathBuf::from(&decoded)
    };
    let normalized = normalize_against(&project.root, &path);
    let confidence = if normalized.starts_with(&project.root) && normalized.is_file() {
        SourceMapConfidence::Resolved
    } else if normalized.starts_with(&project.root) {
        SourceMapConfidence::Fallback
    } else {
        SourceMapConfidence::Unresolved
    };
    Some((normalized, confidence))
}

fn decode_runtime_path(raw_path: &str) -> String {
    let mut output = String::with_capacity(raw_path.len());
    let mut bytes = raw_path.as_bytes().iter().copied();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = bytes.next();
            let low = bytes.next();
            if let (Some(high), Some(low)) = (high, low)
                && let Some(decoded) = decode_hex_pair(high, low)
            {
                output.push(char::from(decoded));
                continue;
            }
            output.push('%');
            if let Some(high) = high {
                output.push(char::from(high));
            }
            if let Some(low) = low {
                output.push(char::from(low));
            }
        } else {
            output.push(char::from(byte));
        }
    }
    output
}

fn decode_hex_pair(high: u8, low: u8) -> Option<u8> {
    Some(hex_value(high)? * 16 + hex_value(low)?)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn count_map_invocations(values: &serde_json::Map<String, Value>) -> usize {
    values.values().map(value_count).sum()
}

fn value_count(value: &Value) -> usize {
    let count = value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|count| u64::try_from(count).ok()))
        .unwrap_or(0);
    usize::try_from(count).unwrap_or(usize::MAX)
}

fn line_for_offset(path: &Path, offset: usize) -> Option<usize> {
    let source = fs::read_to_string(path).ok()?;
    let offset = offset.min(source.len());
    Some(
        source[..offset]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1,
    )
}

fn best_confidence(left: SourceMapConfidence, right: SourceMapConfidence) -> SourceMapConfidence {
    if confidence_rank(left) <= confidence_rank(right) {
        left
    } else {
        right
    }
}

const fn confidence_rank(confidence: SourceMapConfidence) -> usize {
    match confidence {
        SourceMapConfidence::Resolved => 0,
        SourceMapConfidence::Fallback => 1,
        SourceMapConfidence::Unresolved => 2,
    }
}

fn update_hash(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}
