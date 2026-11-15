use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::Location;
use crate::graph::normalize_against;

use super::thresholds::ThresholdContext;
use super::{CoverageGapFinding, CoverageGapReason, CrapFinding, FunctionMetrics, HealthError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CoverageMap {
    files: BTreeMap<PathBuf, CoverageFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CoverageFile {
    lines: BTreeMap<usize, usize>,
}

pub(super) fn load_lcov(root: &Path, path: &Path) -> Result<CoverageMap, HealthError> {
    let source = fs::read_to_string(path).map_err(|source| HealthError::ReadCoverage {
        path: path.to_path_buf(),
        source,
    })?;
    parse_lcov(root, path, &source)
}

pub(super) fn coverage_gap_findings(
    functions: &[FunctionMetrics],
    coverage: &CoverageMap,
) -> Vec<CoverageGapFinding> {
    let mut first_function_by_path = BTreeMap::<PathBuf, &FunctionMetrics>::new();
    for function in functions {
        first_function_by_path
            .entry(function.path.clone())
            .or_insert(function);
    }

    first_function_by_path
        .into_iter()
        .filter_map(|(path, function)| coverage_gap_for_file(path, function, coverage))
        .collect()
}

pub(super) fn crap_findings(
    functions: &[FunctionMetrics],
    coverage: &CoverageMap,
    threshold_context: &mut ThresholdContext,
) -> Vec<CrapFinding> {
    functions
        .iter()
        .filter_map(|function| crap_finding(function, coverage, threshold_context))
        .collect()
}

impl CoverageMap {
    pub(super) fn file_count(&self) -> usize {
        self.files.len()
    }

    pub(super) fn file(&self, path: &Path) -> Option<&CoverageFile> {
        self.files.get(path)
    }
}

impl CoverageFile {
    fn new() -> Self {
        Self {
            lines: BTreeMap::new(),
        }
    }

    pub(super) fn covered_lines(&self) -> usize {
        self.lines.values().filter(|hits| **hits > 0).count()
    }

    pub(super) fn executable_lines(&self) -> usize {
        self.lines.len()
    }

    pub(super) fn first_executable_line(&self) -> Option<usize> {
        self.lines.keys().next().copied()
    }

    pub(super) fn covered_lines_in_range(&self, start: usize, end: usize) -> usize {
        self.lines
            .range(start..=end)
            .filter(|(_, hits)| **hits > 0)
            .count()
    }

    pub(super) fn executable_lines_in_range(&self, start: usize, end: usize) -> usize {
        self.lines.range(start..=end).count()
    }

    fn add_line(&mut self, line: usize, hits: usize) {
        *self.lines.entry(line).or_default() += hits;
    }
}

fn parse_lcov(root: &Path, path: &Path, source: &str) -> Result<CoverageMap, HealthError> {
    let mut files = BTreeMap::<PathBuf, CoverageFile>::new();
    let mut current_path = None::<PathBuf>;
    let mut current_file = CoverageFile::new();

    for (index, raw_line) in source.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(value) = line.strip_prefix("SF:") {
            commit_record(&mut files, &mut current_path, &mut current_file);
            current_path = Some(coverage_path(root, path, value, line_number)?);
            continue;
        }

        if let Some(value) = line.strip_prefix("DA:") {
            let Some(_) = current_path else {
                return Err(parse_error(
                    path,
                    line_number,
                    "DA record appeared before SF",
                ));
            };
            let (line, hits) = parse_da(path, line_number, value)?;
            current_file.add_line(line, hits);
            continue;
        }

        if line == "end_of_record" {
            commit_record(&mut files, &mut current_path, &mut current_file);
        }
    }

    commit_record(&mut files, &mut current_path, &mut current_file);
    Ok(CoverageMap { files })
}

fn coverage_path(
    root: &Path,
    lcov_path: &Path,
    value: &str,
    line: usize,
) -> Result<PathBuf, HealthError> {
    if value.is_empty() {
        return Err(parse_error(lcov_path, line, "SF path was empty"));
    }
    let path = Path::new(value);
    Ok(normalize_against(root, path))
}

fn parse_da(path: &Path, line: usize, value: &str) -> Result<(usize, usize), HealthError> {
    let mut fields = value.split(',');
    let source_line = parse_positive_usize(path, line, fields.next(), "DA line")?;
    let hits = parse_usize(path, line, fields.next(), "DA hit count")?;
    Ok((source_line, hits))
}

fn parse_positive_usize(
    path: &Path,
    line: usize,
    value: Option<&str>,
    label: &str,
) -> Result<usize, HealthError> {
    let value = parse_usize(path, line, value, label)?;
    if value == 0 {
        return Err(parse_error(
            path,
            line,
            &format!("{label} must be positive"),
        ));
    }
    Ok(value)
}

fn parse_usize(
    path: &Path,
    line: usize,
    value: Option<&str>,
    label: &str,
) -> Result<usize, HealthError> {
    let Some(value) = value else {
        return Err(parse_error(path, line, &format!("{label} was missing")));
    };
    value
        .parse::<usize>()
        .map_err(|_| parse_error(path, line, &format!("{label} was not a number")))
}

fn commit_record(
    files: &mut BTreeMap<PathBuf, CoverageFile>,
    current_path: &mut Option<PathBuf>,
    current_file: &mut CoverageFile,
) {
    let Some(path) = current_path.take() else {
        return;
    };
    if path.extension().and_then(|extension| extension.to_str()) != Some("dart") {
        *current_file = CoverageFile::new();
        return;
    }
    let record = std::mem::replace(current_file, CoverageFile::new());
    let entry = files.entry(path).or_insert_with(CoverageFile::new);
    for (line, hits) in record.lines {
        entry.add_line(line, hits);
    }
}

fn coverage_gap_for_file(
    path: PathBuf,
    function: &FunctionMetrics,
    coverage: &CoverageMap,
) -> Option<CoverageGapFinding> {
    let Some(file) = coverage.file(&path) else {
        return Some(CoverageGapFinding {
            path,
            location: function.location,
            reason: CoverageGapReason::MissingFromCoverage,
            covered_lines: 0,
            executable_lines: 0,
        });
    };

    let covered_lines = file.covered_lines();
    let executable_lines = file.executable_lines();
    let reason = if executable_lines == 0 {
        CoverageGapReason::NoExecutableLines
    } else if covered_lines == 0 {
        CoverageGapReason::ZeroCoveredLines
    } else {
        return None;
    };

    Some(CoverageGapFinding {
        path,
        location: file
            .first_executable_line()
            .map_or(function.location, |line| Location { line, column: 0 }),
        reason,
        covered_lines,
        executable_lines,
    })
}

fn crap_finding(
    function: &FunctionMetrics,
    coverage: &CoverageMap,
    threshold_context: &mut ThresholdContext,
) -> Option<CrapFinding> {
    let file = coverage.file(&function.path)?;
    let executable_lines =
        file.executable_lines_in_range(function.location.line, function.end_line);
    if executable_lines == 0 {
        return None;
    }
    let covered_lines = file.covered_lines_in_range(function.location.line, function.end_line);
    let crap_score = crap_score(function.cyclomatic, covered_lines, executable_lines);
    let thresholds = threshold_context.crap_thresholds(function, crap_score)?;
    let max_crap = thresholds.effective.max_crap?;
    if crap_score < max_crap {
        return None;
    }

    Some(CrapFinding {
        path: function.path.clone(),
        symbol: function.symbol.clone(),
        kind: function.kind,
        location: function.location,
        cyclomatic_complexity: function.cyclomatic,
        cognitive_complexity: function.cognitive,
        covered_lines,
        executable_lines,
        line_coverage_percent: coverage_percent(covered_lines, executable_lines),
        crap_score,
        effective_thresholds: thresholds.source.map(|_| thresholds.effective.clone()),
        threshold_source: thresholds.source,
        threshold_reason: thresholds.reason,
    })
}

fn crap_score(cyclomatic: usize, covered_lines: usize, executable_lines: usize) -> usize {
    let complexity = cyclomatic as u128;
    let missed_lines = executable_lines.saturating_sub(covered_lines) as u128;
    let executable_lines = executable_lines as u128;
    let numerator = complexity
        .saturating_mul(complexity)
        .saturating_mul(missed_lines.pow(3));
    let denominator = executable_lines.pow(3);
    let uncovered_risk = numerator.saturating_add(denominator - 1) / denominator;
    usize::try_from(uncovered_risk.saturating_add(complexity)).unwrap_or(usize::MAX)
}

fn coverage_percent(covered_lines: usize, executable_lines: usize) -> usize {
    covered_lines
        .saturating_mul(100)
        .saturating_add(executable_lines / 2)
        / executable_lines
}

fn parse_error(path: &Path, line: usize, message: &str) -> HealthError {
    HealthError::ParseCoverage {
        path: path.to_path_buf(),
        line,
        message: message.to_owned(),
    }
}
