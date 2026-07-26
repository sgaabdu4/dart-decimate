use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::format::display_path;
use super::{Finding, FindingAction, FindingKind, Severity};
use crate::DartFile;

pub(super) fn filter_suppressed_findings(
    root: &Path,
    files: &[DartFile],
    findings: Vec<Finding>,
    require_reasons: bool,
) -> Vec<Finding> {
    let mut state = SuppressionState::new(root, files);
    let mut filtered = findings
        .into_iter()
        .filter(|finding| !state.is_suppressed(finding))
        .collect::<Vec<_>>();
    filtered.extend(state.stale_findings());
    if require_reasons {
        filtered.extend(state.missing_reason_findings());
    }
    filtered
}

#[derive(Debug)]
struct SuppressionState {
    root: PathBuf,
    cache: BTreeMap<PathBuf, Option<Vec<String>>>,
    directives: BTreeMap<SuppressionKey, SuppressionDirective>,
    used: BTreeSet<SuppressionKey>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SuppressionKey {
    path: String,
    line: usize,
}

#[derive(Debug)]
struct SuppressionDirective {
    column: usize,
    text: String,
    has_reason: bool,
}

impl SuppressionState {
    fn new(root: &Path, files: &[DartFile]) -> Self {
        let mut cache = BTreeMap::new();
        let mut directives = BTreeMap::new();
        for file in files {
            let path = file.path.clone();
            let lines = read_lines(&path);
            if let Some(lines) = &lines {
                collect_directives(root, &path, lines, &mut directives);
            }
            cache.insert(path, lines);
        }
        Self {
            root: root.to_path_buf(),
            cache,
            directives,
            used: BTreeSet::new(),
        }
    }

    fn is_suppressed(&mut self, finding: &Finding) -> bool {
        let Some(previous_line) = finding.line.checked_sub(2) else {
            return false;
        };
        let path = finding_path(&self.root, &finding.path);
        let lines = self
            .cache
            .entry(path.clone())
            .or_insert_with(|| read_lines(&path));
        let Some(lines) = lines.as_ref() else {
            return false;
        };
        let Some(line) = lines.get(previous_line) else {
            return false;
        };
        if !suppression_matches(line, finding) {
            return false;
        }
        self.used.insert(SuppressionKey {
            path: display_path(&self.root, &path),
            line: previous_line + 1,
        });
        true
    }

    fn stale_findings(&self) -> Vec<Finding> {
        self.directives
            .iter()
            .filter(|(key, _)| !self.used.contains(key))
            .map(|(key, directive)| stale_finding(key, directive))
            .collect()
    }

    fn missing_reason_findings(&self) -> Vec<Finding> {
        self.directives
            .iter()
            .filter(|(_, directive)| !directive.has_reason)
            .map(|(key, directive)| missing_reason_finding(key, directive))
            .collect()
    }
}

fn collect_directives(
    root: &Path,
    path: &Path,
    lines: &[String],
    directives: &mut BTreeMap<SuppressionKey, SuppressionDirective>,
) {
    let display = display_path(root, path);
    for (index, line) in lines.iter().enumerate() {
        if let Some(suppression) = parse_suppression(line) {
            directives.insert(
                SuppressionKey {
                    path: display.clone(),
                    line: index + 1,
                },
                SuppressionDirective {
                    column: line.find("//").unwrap_or_default(),
                    text: line.trim().to_owned(),
                    has_reason: suppression.has_reason,
                },
            );
        }
    }
}

fn stale_finding(key: &SuppressionKey, directive: &SuppressionDirective) -> Finding {
    Finding {
        rule_id: "dart-decimate/stale-suppression".to_owned(),
        fingerprint: Some(format!("stale-suppression:{}:{}", key.path, key.line)),
        kind: FindingKind::StaleSuppression,
        severity: Severity::Error,
        message: format!(
            "Suppression no longer matches a finding: {}",
            directive.text
        ),
        path: key.path.clone(),
        line: key.line,
        column: directive.column,
        safe_to_delete: true,
        files: Vec::new(),
        edge: None,
        actions: vec![
            FindingAction::new(
                "remove-suppression",
                "Remove the unused suppression comment",
                true,
            )
            .with_target_path(key.path.clone())
            .with_suppression_comment(directive.text.clone()),
        ],
    }
}

fn missing_reason_finding(key: &SuppressionKey, directive: &SuppressionDirective) -> Finding {
    Finding {
        rule_id: "dart-decimate/missing-suppression-reason".to_owned(),
        fingerprint: Some(format!(
            "missing-suppression-reason:{}:{}",
            key.path, key.line
        )),
        kind: FindingKind::MissingSuppressionReason,
        severity: Severity::Error,
        message: format!("Suppression must include a reason: {}", directive.text),
        path: key.path.clone(),
        line: key.line,
        column: directive.column,
        safe_to_delete: false,
        files: Vec::new(),
        edge: None,
        actions: vec![
            FindingAction::new(
                "document-suppression",
                "Add a short reason after the suppression comment",
                false,
            )
            .with_target_path(key.path.clone())
            .with_suppression_comment(format!(
                "{} -- explain why this is intentional",
                directive.text
            )),
        ],
    }
}

fn finding_path(root: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn read_lines(path: &Path) -> Option<Vec<String>> {
    fs::read_to_string(path)
        .ok()
        .map(|source| source.lines().map(str::to_owned).collect())
}

fn suppression_matches(line: &str, finding: &Finding) -> bool {
    let Some(suppression) = parse_suppression(line) else {
        return false;
    };
    let rules = suppression.rules;
    rules.is_empty() || rules.iter().any(|rule| rule_matches(rule, finding))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedSuppression {
    rules: Vec<String>,
    has_reason: bool,
}

fn parse_suppression(line: &str) -> Option<ParsedSuppression> {
    let comment = line.trim_start().strip_prefix("//")?.trim_start();
    for directive in ["dart-decimate-ignore-next-line"] {
        if let Some(rest) = comment.strip_prefix(directive) {
            if rest.chars().next().is_none_or(char::is_whitespace) {
                let (rules, reason) = split_reason(rest.trim_start());
                return Some(ParsedSuppression {
                    rules: rules
                        .split(|character: char| character == ',' || character.is_whitespace())
                        .filter(|rule| !rule.is_empty())
                        .map(str::to_owned)
                        .collect(),
                    has_reason: reason.is_some_and(|value| !value.trim().is_empty()),
                });
            }
        }
    }
    None
}

fn split_reason(rest: &str) -> (&str, Option<&str>) {
    if let Some(index) = rest.find("--") {
        return (&rest[..index], Some(&rest[index + 2..]));
    }
    if let Some(reason) = rest.strip_prefix("because ") {
        return ("", Some(reason));
    }
    if let Some(index) = rest.find(" because ") {
        return (&rest[..index], Some(&rest[index + " because ".len()..]));
    }
    if let Some(reason) = rest.strip_prefix("reason:") {
        return ("", Some(reason));
    }
    if let Some(index) = rest.find(" reason:") {
        return (&rest[..index], Some(&rest[index + " reason:".len()..]));
    }
    (rest, None)
}

fn rule_matches(rule: &str, finding: &Finding) -> bool {
    rule == "all"
        || rule == finding.rule_id
        || finding
            .rule_id
            .rsplit('/')
            .next()
            .is_some_and(|rule_id| rule == rule_id)
        || rule == kind_key(finding.kind)
        || security_rule_matches(rule, finding)
}

fn security_rule_matches(rule: &str, finding: &Finding) -> bool {
    finding.kind == FindingKind::SecurityCandidate
        && (rule == "security-sink"
            || (rule == "hardcoded-secret" && finding.rule_id.ends_with("hardcoded-secret")))
}

const fn kind_key(kind: FindingKind) -> &'static str {
    match kind {
        FindingKind::DeadFile => "dead-file",
        FindingKind::UnusedExport => "unused-export",
        FindingKind::UnusedType => "unused-type",
        FindingKind::PrivateTypeLeak => "private-type-leak",
        FindingKind::UnusedEnumMember => "unused-enum-member",
        FindingKind::UnusedClassMember => "unused-class-member",
        FindingKind::DuplicateExport => "duplicate-export",
        FindingKind::RouteCollision => "route-collision",
        FindingKind::MixedGoRouterStyle => "mixed-go-router-style",
        FindingKind::PrivateWidgetClass => "private-widget-class",
        FindingKind::WidgetTopLevelFunctionBoundary => "widget-top-level-function-boundary",
        FindingKind::UnusedWidgetParam => "unused-widget-param",
        FindingKind::UnrenderedWidget => "unrendered-widget",
        FindingKind::MissingContextMountedAfterAwait => "missing-context-mounted-after-await",
        FindingKind::MissingEntryPoint => "missing-entry-point",
        FindingKind::CircularDependency => "circular-dependency",
        FindingKind::ReExportCycle => "re-export-cycle",
        FindingKind::BoundaryViolation => "boundary-violation",
        FindingKind::BoundaryCoverage => "boundary-coverage",
        FindingKind::BoundaryCallViolation => "boundary-call-violation",
        FindingKind::PolicyViolation => "policy-violation",
        FindingKind::UnresolvedDependency => "unresolved-dependency",
        FindingKind::PartOfViolation => "part-of-violation",
        FindingKind::UnusedDependency => "unused-dependency",
        FindingKind::UnusedDevDependency => "unused-dev-dependency",
        FindingKind::TestOnlyDependency => "test-only-dependency",
        FindingKind::UnusedDependencyOverride => "unused-dependency-override",
        FindingKind::MisconfiguredDependencyOverride => "misconfigured-dependency-override",
        FindingKind::UnlistedDependency => "unlisted-dependency",
        FindingKind::PrivateSrcImport => "private-src-import",
        FindingKind::CodeDuplication => "code-duplication",
        FindingKind::HighCyclomaticComplexity => "high-cyclomatic-complexity",
        FindingKind::HighCognitiveComplexity => "high-cognitive-complexity",
        FindingKind::HighComplexity => "high-complexity",
        FindingKind::CoverageGap => "coverage-gap",
        FindingKind::HighCrapScore => "high-crap-score",
        FindingKind::HealthHotspot => "health-hotspot",
        FindingKind::RefactoringTarget => "refactoring-target",
        FindingKind::FeatureFlag => "feature-flag",
        FindingKind::SecurityCandidate => "security-candidate",
        FindingKind::StaleSuppression => "stale-suppression",
        FindingKind::MissingSuppressionReason => "missing-suppression-reason",
    }
}
