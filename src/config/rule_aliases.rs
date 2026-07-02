use crate::output::FindingKind;

pub(super) fn known_rule(rule: &str) -> bool {
    rule == "all"
        || rule.starts_with("dart-decimate/policy/")
        || all_security_aliases()
            .into_iter()
            .chain(all_dependency_aliases())
            .chain(all_known_aliases())
            .any(|alias| alias == rule)
}

pub(super) fn aliases(rule_id: &str, kind: FindingKind) -> Vec<&'static str> {
    let suffix = rule_id.rsplit('/').next().unwrap_or(rule_id);
    let mut aliases = match suffix {
        "security-hardcoded-secret" => vec![
            "dart-decimate/security-hardcoded-secret",
            "security-hardcoded-secret",
            "hardcoded-secret",
        ],
        "security-firebase-api-key" => vec![
            "dart-decimate/security-firebase-api-key",
            "security-firebase-api-key",
            "firebase-api-key",
        ],
        "security-insecure-transport" => vec![
            "dart-decimate/security-insecure-transport",
            "security-insecure-transport",
            "insecure-transport",
        ],
        "security-tls-bypass" => vec![
            "dart-decimate/security-tls-bypass",
            "security-tls-bypass",
            "tls-bypass",
        ],
        "security-webview-risk" => vec![
            "dart-decimate/security-webview-risk",
            "security-webview-risk",
            "webview-risk",
        ],
        "security-process-execution" => vec![
            "dart-decimate/security-process-execution",
            "dart-decimate/security-process-exec",
            "security-process-execution",
            "process-execution",
            "process-exec",
        ],
        "security-raw-sql" => vec![
            "dart-decimate/security-raw-sql",
            "security-raw-sql",
            "raw-sql",
        ],
        "security-plain-secret-storage" => vec![
            "dart-decimate/security-plain-secret-storage",
            "security-plain-secret-storage",
            "plain-secret-storage",
        ],
        "unused-dev-dependency" => vec![
            "dart-decimate/unused-dev-dependency",
            "unused-dev-dependency",
            "unused-dev-dependencies",
            "unused-dev-deps",
        ],
        "test-only-dependency" => vec![
            "dart-decimate/test-only-dependency",
            "test-only-dependency",
            "test-only-dependencies",
            "test-only-deps",
        ],
        "unused-dependency-override" => unused_dependency_override_aliases().into(),
        "misconfigured-dependency-override" => misconfigured_dependency_override_aliases().into(),
        _ => Vec::new(),
    };
    aliases.extend(kind_aliases(kind));
    aliases.push("all");
    aliases
}

pub(super) fn missing_suppression_reason_aliases() -> [&'static str; 3] {
    [
        "dart-decimate/missing-suppression-reason",
        "missing-suppression-reason",
        "missing-suppression-reasons",
    ]
}

pub(super) fn private_type_leak_aliases() -> [&'static str; 3] {
    [
        "dart-decimate/private-type-leak",
        "private-type-leak",
        "private-type-leaks",
    ]
}

fn all_security_aliases() -> Vec<&'static str> {
    [
        "dart-decimate/security-hardcoded-secret",
        "security-hardcoded-secret",
        "hardcoded-secret",
        "dart-decimate/security-firebase-api-key",
        "security-firebase-api-key",
        "firebase-api-key",
        "dart-decimate/security-insecure-transport",
        "security-insecure-transport",
        "insecure-transport",
        "dart-decimate/security-tls-bypass",
        "security-tls-bypass",
        "tls-bypass",
        "dart-decimate/security-webview-risk",
        "security-webview-risk",
        "webview-risk",
        "dart-decimate/security-process-execution",
        "dart-decimate/security-process-exec",
        "security-process-execution",
        "process-execution",
        "process-exec",
        "dart-decimate/security-raw-sql",
        "security-raw-sql",
        "raw-sql",
        "dart-decimate/security-plain-secret-storage",
        "security-plain-secret-storage",
        "plain-secret-storage",
    ]
    .into()
}

fn all_dependency_aliases() -> Vec<&'static str> {
    [
        "dart-decimate/unused-dev-dependency",
        "unused-dev-dependency",
        "unused-dev-dependencies",
        "unused-dev-deps",
        "dart-decimate/test-only-dependency",
        "test-only-dependency",
        "test-only-dependencies",
        "test-only-deps",
        "dart-decimate/unused-dependency-override",
        "unused-dependency-override",
        "unused-dependency-overrides",
        "dart-decimate/misconfigured-dependency-override",
        "misconfigured-dependency-override",
        "misconfigured-dependency-overrides",
        "dart-decimate/private-src-import",
        "private-src-import",
        "private-src-imports",
    ]
    .into()
}

fn all_known_aliases() -> Vec<&'static str> {
    [
        FindingKind::DeadFile,
        FindingKind::UnusedExport,
        FindingKind::UnusedType,
        FindingKind::PrivateTypeLeak,
        FindingKind::UnusedEnumMember,
        FindingKind::UnusedClassMember,
        FindingKind::DuplicateExport,
        FindingKind::RouteCollision,
        FindingKind::PrivateWidgetClass,
        FindingKind::WidgetTopLevelFunctionBoundary,
        FindingKind::UnusedWidgetParam,
        FindingKind::UnrenderedWidget,
        FindingKind::MissingContextMountedAfterAwait,
        FindingKind::MissingEntryPoint,
        FindingKind::CircularDependency,
        FindingKind::ReExportCycle,
        FindingKind::BoundaryViolation,
        FindingKind::BoundaryCoverage,
        FindingKind::BoundaryCallViolation,
        FindingKind::PolicyViolation,
        FindingKind::UnresolvedDependency,
        FindingKind::PartOfViolation,
        FindingKind::UnusedDependency,
        FindingKind::UnusedDevDependency,
        FindingKind::TestOnlyDependency,
        FindingKind::UnusedDependencyOverride,
        FindingKind::MisconfiguredDependencyOverride,
        FindingKind::UnlistedDependency,
        FindingKind::CodeDuplication,
        FindingKind::HighCyclomaticComplexity,
        FindingKind::HighCognitiveComplexity,
        FindingKind::HighComplexity,
        FindingKind::CoverageGap,
        FindingKind::HighCrapScore,
        FindingKind::HealthHotspot,
        FindingKind::RefactoringTarget,
        FindingKind::FeatureFlag,
        FindingKind::SecurityCandidate,
        FindingKind::StaleSuppression,
        FindingKind::MissingSuppressionReason,
    ]
    .into_iter()
    .flat_map(|kind| kind_aliases(kind).into_iter())
    .collect()
}

fn kind_aliases(kind: FindingKind) -> Vec<&'static str> {
    cleanup_kind_aliases(kind)
        .or_else(|| widget_kind_aliases(kind))
        .or_else(|| graph_kind_aliases(kind))
        .or_else(|| dependency_kind_aliases(kind))
        .or_else(|| quality_kind_aliases(kind))
        .unwrap_or_default()
}

fn cleanup_kind_aliases(kind: FindingKind) -> Option<Vec<&'static str>> {
    match kind {
        FindingKind::DeadFile => Some(vec![
            "dart-decimate/dead-file",
            "dead-file",
            "dead-files",
            "unused-file",
            "unused-files",
        ]),
        FindingKind::UnusedExport => Some(vec![
            "dart-decimate/unused-export",
            "unused-export",
            "unused-exports",
        ]),
        FindingKind::UnusedType => Some(vec![
            "dart-decimate/unused-type",
            "unused-type",
            "unused-types",
        ]),
        FindingKind::PrivateTypeLeak => Some(private_type_leak_aliases().into()),
        FindingKind::UnusedEnumMember => Some(vec![
            "dart-decimate/unused-enum-member",
            "unused-enum-member",
            "unused-enum-members",
        ]),
        FindingKind::UnusedClassMember => Some(vec![
            "dart-decimate/unused-class-member",
            "unused-class-member",
            "unused-class-members",
        ]),
        FindingKind::DuplicateExport => Some(vec![
            "dart-decimate/duplicate-export",
            "duplicate-export",
            "duplicate-exports",
        ]),
        FindingKind::StaleSuppression => Some(vec![
            "dart-decimate/stale-suppression",
            "stale-suppression",
            "stale-suppressions",
            "unused-suppression",
            "unused-suppressions",
        ]),
        FindingKind::MissingSuppressionReason => Some(missing_suppression_reason_aliases().into()),
        _ => None,
    }
}

fn widget_kind_aliases(kind: FindingKind) -> Option<Vec<&'static str>> {
    match kind {
        FindingKind::PrivateWidgetClass => Some(vec![
            "dart-decimate/private-widget-class",
            "private-widget-class",
            "private-widget-classes",
            "flutter-private-widget-class",
            "flutter-private-widget-classes",
        ]),
        FindingKind::WidgetTopLevelFunctionBoundary => Some(vec![
            "dart-decimate/widget-top-level-function-boundary",
            "widget-top-level-function-boundary",
            "top-level-widget-helper",
            "top-level-widget-helpers",
            "flutter-widget-helper-function",
        ]),
        FindingKind::UnusedWidgetParam => Some(vec![
            "dart-decimate/unused-widget-param",
            "unused-widget-param",
            "unused-widget-params",
            "unused-component-prop",
            "unused-component-props",
            "flutter-unused-widget-param",
            "flutter-unused-widget-params",
        ]),
        FindingKind::UnrenderedWidget => Some(vec![
            "dart-decimate/unrendered-widget",
            "unrendered-widget",
            "unrendered-widgets",
            "unused-widget-class",
            "unused-widget-classes",
            "unused-component",
            "unused-components",
        ]),
        FindingKind::MissingContextMountedAfterAwait => Some(vec![
            "dart-decimate/missing-context-mounted-after-await",
            "missing-context-mounted-after-await",
            "context-mounted-after-await",
            "flutter-context-mounted",
            "use-build-context-synchronously",
        ]),
        _ => None,
    }
}

fn graph_kind_aliases(kind: FindingKind) -> Option<Vec<&'static str>> {
    match kind {
        FindingKind::MissingEntryPoint => Some(vec![
            "dart-decimate/missing-entry-point",
            "missing-entry-point",
            "missing-entry-points",
        ]),
        FindingKind::CircularDependency => Some(vec![
            "dart-decimate/circular-dependency",
            "circular-dependency",
            "circular-dependencies",
            "circular-deps",
        ]),
        FindingKind::ReExportCycle => Some(vec![
            "dart-decimate/re-export-cycle",
            "re-export-cycle",
            "re-export-cycles",
        ]),
        FindingKind::RouteCollision => Some(vec![
            "dart-decimate/route-collision",
            "route-collision",
            "route-collisions",
            "flutter-route-collision",
            "flutter-route-collisions",
        ]),
        FindingKind::BoundaryViolation => Some(vec![
            "dart-decimate/boundary-violation",
            "boundary-violation",
            "boundary-violations",
        ]),
        FindingKind::BoundaryCoverage => Some(vec![
            "dart-decimate/boundary-coverage",
            "boundary-coverage",
            "boundary-coverages",
            "dart-decimate/boundary-violation",
            "boundary-violation",
            "boundary-violations",
        ]),
        FindingKind::BoundaryCallViolation => Some(vec![
            "dart-decimate/boundary-call-violation",
            "boundary-call-violation",
            "boundary-call-violations",
            "dart-decimate/boundary-violation",
            "boundary-violation",
            "boundary-violations",
        ]),
        FindingKind::PolicyViolation => Some(vec![
            "dart-decimate/policy-violation",
            "policy-violation",
            "policy-violations",
        ]),
        FindingKind::UnresolvedDependency => Some(vec![
            "dart-decimate/unresolved-dependency",
            "unresolved-dependency",
            "unresolved-import",
            "unresolved-imports",
            "unresolved-augment",
            "unresolved-augments",
        ]),
        FindingKind::PartOfViolation => Some(vec![
            "dart-decimate/part-of-violation",
            "part-of-violation",
            "part-of-violations",
            "invalid-part-of",
            "invalid-part",
        ]),
        _ => None,
    }
}

fn dependency_kind_aliases(kind: FindingKind) -> Option<Vec<&'static str>> {
    match kind {
        FindingKind::UnusedDependency => Some(vec![
            "dart-decimate/unused-dependency",
            "unused-dependency",
            "unused-dependencies",
            "unused-deps",
        ]),
        FindingKind::UnusedDevDependency => Some(vec![
            "dart-decimate/unused-dev-dependency",
            "unused-dev-dependency",
            "unused-dev-dependencies",
            "unused-dev-deps",
        ]),
        FindingKind::TestOnlyDependency => Some(vec![
            "dart-decimate/test-only-dependency",
            "test-only-dependency",
            "test-only-dependencies",
            "test-only-deps",
        ]),
        FindingKind::UnusedDependencyOverride => Some(unused_dependency_override_aliases().into()),
        FindingKind::MisconfiguredDependencyOverride => {
            Some(misconfigured_dependency_override_aliases().into())
        }
        FindingKind::UnlistedDependency => Some(vec![
            "dart-decimate/unlisted-dependency",
            "unlisted-dependency",
            "unlisted-dependencies",
            "unlisted-deps",
        ]),
        FindingKind::PrivateSrcImport => Some(vec![
            "dart-decimate/private-src-import",
            "private-src-import",
            "private-src-imports",
        ]),
        _ => None,
    }
}

fn unused_dependency_override_aliases() -> [&'static str; 6] {
    [
        "dart-decimate/unused-dependency-override",
        "unused-dependency-override",
        "unused-dependency-overrides",
        "dart-decimate/dependency-override",
        "dependency-override",
        "dependency-overrides",
    ]
}

fn misconfigured_dependency_override_aliases() -> [&'static str; 6] {
    [
        "dart-decimate/misconfigured-dependency-override",
        "misconfigured-dependency-override",
        "misconfigured-dependency-overrides",
        "dart-decimate/dependency-override",
        "dependency-override",
        "dependency-overrides",
    ]
}

fn quality_kind_aliases(kind: FindingKind) -> Option<Vec<&'static str>> {
    match kind {
        FindingKind::CodeDuplication => Some(vec![
            "dart-decimate/code-duplication",
            "code-duplication",
            "duplication",
            "dupes",
        ]),
        FindingKind::HighCyclomaticComplexity => Some(vec![
            "dart-decimate/high-cyclomatic-complexity",
            "high-cyclomatic-complexity",
            "complexity",
        ]),
        FindingKind::HighCognitiveComplexity => Some(vec![
            "dart-decimate/high-cognitive-complexity",
            "high-cognitive-complexity",
            "complexity",
        ]),
        FindingKind::HighComplexity => Some(vec![
            "dart-decimate/high-complexity",
            "high-complexity",
            "complexity",
        ]),
        FindingKind::CoverageGap => Some(vec![
            "dart-decimate/coverage-gap",
            "coverage-gap",
            "coverage-gaps",
            "untested-file",
            "untested-files",
        ]),
        FindingKind::HighCrapScore => Some(vec![
            "dart-decimate/high-crap-score",
            "high-crap-score",
            "crap-score",
            "crap",
            "complexity",
        ]),
        FindingKind::HealthHotspot => Some(vec![
            "dart-decimate/health-hotspot",
            "health-hotspot",
            "health-hotspots",
            "hotspot",
            "hotspots",
        ]),
        FindingKind::RefactoringTarget => Some(vec![
            "dart-decimate/refactoring-target",
            "refactoring-target",
            "refactoring-targets",
            "target",
            "targets",
        ]),
        FindingKind::FeatureFlag => Some(vec![
            "dart-decimate/feature-flag",
            "feature-flag",
            "feature-flags",
            "flags",
        ]),
        FindingKind::SecurityCandidate => Some(vec![
            "security",
            "security-candidate",
            "security-candidates",
            "security-sink",
        ]),
        _ => None,
    }
}
