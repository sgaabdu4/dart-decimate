use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use petgraph::visit::EdgeRef;
use thiserror::Error;

use crate::changed_scope::RefSuggestions;
use crate::graph::normalize_against;
use crate::output::{FindingKind, JsonReport, JsonSecurityOccurrence, Severity, Verdict};
use crate::{ModuleGraph, find_dead_code};

/// Fallow-style security review gate mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SecurityGateMode {
    /// Fail only on candidates introduced by added diff lines.
    New,
    /// Fail when changed code makes a candidate newly reachable.
    NewlyReachable,
}

/// Source for line-level security gate scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SecurityDiffSource {
    /// Read a unified diff from a file.
    File(PathBuf),
    /// Read a unified diff from standard input.
    Stdin,
}

/// Errors returned while applying a security gate.
#[derive(Debug, Error)]
pub enum SecurityGateError {
    /// Security gates need an explicit diff.
    #[error(
        "security --gate new or newly-reachable requires --diff-file PATH, --diff-stdin, --changed-since REF, or --compare REF"
    )]
    MissingDiffFile,
    /// Diff file could not be read.
    #[error("failed to read diff file {path}: {source}")]
    ReadDiff {
        /// Diff path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Git could not be executed.
    #[error("failed to run git diff for security gate: {source}")]
    Git {
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Git returned a non-zero status.
    #[error("git diff failed for security gate base {base:?}: {stderr}{suggestions}")]
    GitDiff {
        /// Base revision passed by the caller.
        base: String,
        /// Stderr from git.
        stderr: String,
        /// Similar refs found in the local repository.
        suggestions: RefSuggestions,
    },
    /// Untracked file scope could not be read.
    #[error("failed to read untracked file {path}: {source}")]
    ReadUntracked {
        /// File path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ChangedLineScope {
    lines: BTreeMap<String, BTreeSet<usize>>,
}

impl ChangedLineScope {
    fn insert(&mut self, path: &str, line: usize) {
        self.lines.entry(path.to_owned()).or_default().insert(line);
    }

    fn contains(&self, path: &str, line: usize) -> bool {
        self.lines
            .get(path)
            .is_some_and(|lines| lines.contains(&line))
    }

    fn paths(&self) -> impl Iterator<Item = &str> {
        self.lines.keys().map(String::as_str)
    }
}

/// Load changed-line scope from a unified diff file.
///
/// # Errors
///
/// Returns [`SecurityGateError`] if the diff cannot be read.
pub(crate) fn changed_lines_from_diff_file(
    root: &Path,
    path: &Path,
) -> Result<ChangedLineScope, SecurityGateError> {
    let source = fs::read_to_string(path).map_err(|source| SecurityGateError::ReadDiff {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(changed_lines_from_unified_diff(root, &source))
}

/// Load changed-line scope from a unified diff string.
pub(crate) fn changed_lines_from_diff(root: &Path, source: &str) -> ChangedLineScope {
    changed_lines_from_unified_diff(root, source)
}

/// Load changed-line scope from Git changes since `base`.
///
/// # Errors
///
/// Returns [`SecurityGateError`] if Git fails or an untracked file cannot be read.
pub(crate) fn changed_lines_from_git(
    root: &Path,
    base: &str,
) -> Result<ChangedLineScope, SecurityGateError> {
    let output = Command::new("git")
        .args(["diff", "--unified=0", "--diff-filter=ACMRTUXB", base, "--"])
        .current_dir(root)
        .output()
        .map_err(|source| SecurityGateError::Git { source })?;

    if !output.status.success() {
        return Err(SecurityGateError::GitDiff {
            base: base.to_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            suggestions: RefSuggestions::for_base(root, base),
        });
    }

    let mut scope =
        changed_lines_from_unified_diff(root, String::from_utf8_lossy(&output.stdout).as_ref());
    add_untracked_lines(root, base, &mut scope)?;
    Ok(scope)
}

/// Restrict a security report to candidates introduced by changed lines.
pub(crate) fn apply_changed_line_gate(report: &mut JsonReport, scope: &ChangedLineScope) {
    apply_security_occurrence_filter(report, |path, line| scope.contains(path, line));
}

/// Restrict a security report to reachable candidates affected by changed files.
pub(crate) fn apply_changed_reachability_gate<P>(
    root: &Path,
    graph: &ModuleGraph,
    report: &mut JsonReport,
    entry_points: impl IntoIterator<Item = P>,
    scope: &ChangedLineScope,
) where
    P: AsRef<Path>,
{
    let reachable = find_dead_code(graph, entry_points)
        .reachable_files
        .into_iter()
        .collect::<BTreeSet<_>>();
    let affected = downstream_changed_files(root, graph, scope);
    let visible = affected
        .into_iter()
        .filter(|path| reachable.contains(path))
        .map(|path| display_path(root, &path))
        .collect::<BTreeSet<_>>();
    apply_security_occurrence_filter(report, |path, _line| visible.contains(path));
}

fn apply_security_occurrence_filter<F>(report: &mut JsonReport, mut keep: F)
where
    F: FnMut(&str, usize) -> bool,
{
    let mut changed = BTreeMap::<String, ChangedCandidate>::new();
    report.security_candidates.retain_mut(|candidate| {
        candidate
            .occurrences
            .retain(|occurrence| keep(&occurrence.path, occurrence.line));
        if candidate.occurrences.is_empty() {
            return false;
        }
        changed.insert(
            candidate.fingerprint.clone(),
            ChangedCandidate::from_occurrences(&candidate.occurrences),
        );
        true
    });
    report
        .attack_surface
        .retain(|entry| keep(&entry.path, entry.line));
    report.findings.retain_mut(|finding| {
        if finding.kind != FindingKind::SecurityCandidate {
            return false;
        }
        let Some(fingerprint) = finding.fingerprint.as_ref() else {
            return keep(&finding.path, finding.line);
        };
        let Some(candidate) = changed.get(fingerprint) else {
            return false;
        };
        finding.path.clone_from(&candidate.primary.path);
        finding.line = candidate.primary.line;
        finding.column = candidate.primary.column;
        finding.files.clone_from(&candidate.files);
        for action in &mut finding.actions {
            if action.target_path.is_some() {
                action.target_path = Some(candidate.primary.path.clone());
            }
            if action.command.is_some() || !action.argv.is_empty() {
                action.argv = vec![
                    "dart-decimate".to_owned(),
                    "inspect".to_owned(),
                    "--format".to_owned(),
                    "json".to_owned(),
                    "--file".to_owned(),
                    candidate.primary.path.clone(),
                ];
                action.command = Some(shell_command(&action.argv));
            }
        }
        true
    });
    report.next_steps.clear();
    recompute_summary(report);
}

fn downstream_changed_files(
    root: &Path,
    graph: &ModuleGraph,
    scope: &ChangedLineScope,
) -> BTreeSet<PathBuf> {
    let mut pending = scope
        .paths()
        .filter_map(|path| graph.node_index(normalize_against(root, Path::new(path))))
        .collect::<Vec<_>>();
    let mut affected = BTreeSet::new();

    while let Some(node) = pending.pop() {
        if !affected.insert(graph.graph()[node].path.clone()) {
            continue;
        }
        pending.extend(graph.graph().edges(node).map(|edge| edge.target()));
    }

    affected
}

fn display_path(root: &Path, path: &Path) -> String {
    let normalized = normalize_against(root, path);
    normalized
        .strip_prefix(root)
        .unwrap_or(&normalized)
        .to_string_lossy()
        .replace('\\', "/")
}

fn add_untracked_lines(
    root: &Path,
    base: &str,
    scope: &mut ChangedLineScope,
) -> Result<(), SecurityGateError> {
    let output = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(root)
        .output()
        .map_err(|source| SecurityGateError::Git { source })?;

    if !output.status.success() {
        return Err(SecurityGateError::GitDiff {
            base: base.to_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            suggestions: RefSuggestions::default(),
        });
    }

    for line in output.stdout.split(|byte| *byte == b'\n') {
        let Some(path) = std::str::from_utf8(line)
            .ok()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .and_then(|path| normalize_untracked_path(root, path))
        else {
            continue;
        };
        add_file_lines(root, &path, scope)?;
    }

    Ok(())
}

fn add_file_lines(
    root: &Path,
    display_path: &str,
    scope: &mut ChangedLineScope,
) -> Result<(), SecurityGateError> {
    let path = root.join(display_path);
    let source = fs::read_to_string(&path).map_err(|source| SecurityGateError::ReadUntracked {
        path: path.clone(),
        source,
    })?;
    for line in 1..=source.lines().count() {
        scope.insert(display_path, line);
    }
    Ok(())
}

fn changed_lines_from_unified_diff(root: &Path, diff: &str) -> ChangedLineScope {
    let mut scope = ChangedLineScope::default();
    let mut current_path = None::<String>;
    let mut new_line = 0usize;
    let mut in_hunk = false;

    for line in diff.lines() {
        if let Some(raw_path) = line.strip_prefix("+++ ") {
            current_path = normalize_diff_path(root, raw_path);
            in_hunk = false;
            continue;
        }
        if let Some(start) = parse_new_hunk_start(line) {
            new_line = start;
            in_hunk = current_path.is_some();
            continue;
        }
        if !in_hunk {
            continue;
        }
        if line.starts_with('+') {
            if let Some(path) = current_path.as_deref() {
                scope.insert(path, new_line);
            }
            new_line += 1;
        } else if !line.starts_with('-') && !line.starts_with('\\') {
            new_line += 1;
        }
    }

    scope
}

fn parse_new_hunk_start(line: &str) -> Option<usize> {
    let hunk = line.strip_prefix("@@ ")?;
    let plus = hunk.find('+')?;
    let after_plus = &hunk[plus + 1..];
    let digits = after_plus
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    digits.parse().ok()
}

fn normalize_diff_path(root: &Path, raw: &str) -> Option<String> {
    let token = raw.trim().split('\t').next()?.trim();
    if token == "/dev/null" || token.is_empty() {
        return None;
    }
    let token = token
        .strip_prefix("a/")
        .or_else(|| token.strip_prefix("b/"))
        .unwrap_or(token)
        .strip_prefix("./")
        .unwrap_or_else(|| {
            token
                .strip_prefix("a/")
                .or_else(|| token.strip_prefix("b/"))
                .unwrap_or(token)
        });
    let path = Path::new(token);
    let relative = if path.is_absolute() {
        path.strip_prefix(root).ok()?
    } else {
        path
    };
    Some(relative.to_string_lossy().replace('\\', "/"))
}

fn normalize_untracked_path(root: &Path, raw: &str) -> Option<String> {
    let path = normalize_against(root, Path::new(raw));
    let relative = path.strip_prefix(root).ok()?;
    Some(relative.to_string_lossy().replace('\\', "/"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ChangedCandidate {
    primary: JsonSecurityOccurrence,
    files: Vec<String>,
}

impl ChangedCandidate {
    fn from_occurrences(occurrences: &[JsonSecurityOccurrence]) -> Self {
        let primary = occurrences[0].clone();
        let files = occurrences
            .iter()
            .map(|occurrence| occurrence.path.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Self { primary, files }
    }
}

fn recompute_summary(report: &mut JsonReport) {
    report.summary.unresolved_dependencies = kind_count(report, FindingKind::UnresolvedDependency);
    report.summary.part_of_violations = kind_count(report, FindingKind::PartOfViolation);
    report.summary.unused_dependencies = dependency_count(report);
    report.summary.unused_dev_dependencies = kind_count(report, FindingKind::UnusedDevDependency);
    report.summary.test_only_dependencies = kind_count(report, FindingKind::TestOnlyDependency);
    report.summary.dependency_overrides = kind_count(report, FindingKind::UnusedDependencyOverride)
        + kind_count(report, FindingKind::MisconfiguredDependencyOverride);
    report.summary.unused_dependency_overrides =
        kind_count(report, FindingKind::UnusedDependencyOverride);
    report.summary.misconfigured_dependency_overrides =
        kind_count(report, FindingKind::MisconfiguredDependencyOverride);
    report.summary.unlisted_dependencies = kind_count(report, FindingKind::UnlistedDependency);
    report.summary.private_src_imports = kind_count(report, FindingKind::PrivateSrcImport);
    report.summary.dead_files = kind_count(report, FindingKind::DeadFile);
    report.summary.unused_exports = kind_count(report, FindingKind::UnusedExport);
    report.summary.unused_types = kind_count(report, FindingKind::UnusedType);
    report.summary.private_type_leaks = kind_count(report, FindingKind::PrivateTypeLeak);
    report.summary.unused_enum_members = kind_count(report, FindingKind::UnusedEnumMember);
    report.summary.unused_class_members = kind_count(report, FindingKind::UnusedClassMember);
    report.summary.duplicate_exports = kind_count(report, FindingKind::DuplicateExport);
    report.summary.route_collisions = kind_count(report, FindingKind::RouteCollision);
    report.summary.private_widget_classes = kind_count(report, FindingKind::PrivateWidgetClass);
    report.summary.widget_top_level_functions =
        kind_count(report, FindingKind::WidgetTopLevelFunctionBoundary);
    report.summary.unused_widget_params = kind_count(report, FindingKind::UnusedWidgetParam);
    report.summary.unrendered_widgets = kind_count(report, FindingKind::UnrenderedWidget);
    report.summary.missing_context_mounted_after_await =
        kind_count(report, FindingKind::MissingContextMountedAfterAwait);
    report.summary.code_duplications = report.clone_groups.len();
    report.summary.complex_functions = complexity_count(report);
    report.summary.coverage_gaps = kind_count(report, FindingKind::CoverageGap);
    report.summary.crap_functions = kind_count(report, FindingKind::HighCrapScore);
    report.summary.file_scores = report.file_scores.len();
    report.summary.hotspots = report.hotspots.len();
    report.summary.refactoring_targets = report.refactoring_targets.len();
    report.summary.feature_flags = report.feature_flags.len();
    report.summary.feature_flag_occurrences = report
        .feature_flags
        .iter()
        .map(|flag| flag.occurrences.len())
        .sum();
    report.summary.security_candidates = report.security_candidates.len();
    report.summary.security_candidate_occurrences = report
        .security_candidates
        .iter()
        .map(|candidate| candidate.occurrences.len())
        .sum();
    report.summary.attack_surface = report.attack_surface.len();
    report.summary.missing_entry_points = kind_count(report, FindingKind::MissingEntryPoint);
    report.summary.cycles = kind_count(report, FindingKind::CircularDependency);
    report.summary.re_export_cycles = kind_count(report, FindingKind::ReExportCycle);
    report.summary.boundary_violations = kind_count(report, FindingKind::BoundaryViolation);
    report.summary.boundary_coverage = kind_count(report, FindingKind::BoundaryCoverage);
    report.summary.boundary_call_violations =
        kind_count(report, FindingKind::BoundaryCallViolation);
    report.summary.policy_violations = kind_count(report, FindingKind::PolicyViolation);
    report.summary.missing_suppression_reasons =
        kind_count(report, FindingKind::MissingSuppressionReason);
    report.summary.findings = report.findings.len();
    report.verdict = if report
        .findings
        .iter()
        .any(|finding| finding.severity == Severity::Error)
    {
        Verdict::Fail
    } else {
        Verdict::Pass
    };
}

fn kind_count(report: &JsonReport, kind: FindingKind) -> usize {
    report
        .findings
        .iter()
        .filter(|finding| finding.kind == kind)
        .count()
}

fn dependency_count(report: &JsonReport) -> usize {
    report
        .findings
        .iter()
        .filter(|finding| {
            matches!(
                finding.kind,
                FindingKind::UnusedDependency
                    | FindingKind::UnusedDevDependency
                    | FindingKind::TestOnlyDependency
                    | FindingKind::UnusedDependencyOverride
            )
        })
        .count()
}

fn complexity_count(report: &JsonReport) -> usize {
    report
        .findings
        .iter()
        .filter(|finding| {
            matches!(
                finding.kind,
                FindingKind::HighCyclomaticComplexity
                    | FindingKind::HighCognitiveComplexity
                    | FindingKind::HighComplexity
                    | FindingKind::HighCrapScore
            )
        })
        .count()
}

fn shell_command(argv: &[String]) -> String {
    argv.iter()
        .map(|arg| shell_escape(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_escape(arg: &str) -> String {
    if arg.chars().all(is_shell_safe) {
        return arg.to_owned();
    }
    format!("'{}'", arg.replace('\'', "'\"'\"'"))
}

fn is_shell_safe(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(
            character,
            '/' | '.' | ':' | '_' | '-' | '=' | '+' | ',' | '@'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_added_lines_from_git_diff() {
        let scope = changed_lines_from_unified_diff(
            Path::new("/repo"),
            "diff --git a/lib/main.dart b/lib/main.dart
--- a/lib/main.dart
+++ b/lib/main.dart
@@ -1,2 +1,3 @@
 context
+added
-removed
+second
",
        );

        assert!(scope.contains("lib/main.dart", 2));
        assert!(scope.contains("lib/main.dart", 3));
        assert!(!scope.contains("lib/main.dart", 1));
    }
}
