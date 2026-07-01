use super::types::{Finding, FindingAction, FindingKind, ReportSummary, Verdict};

pub(super) fn summary_groups(summary: &ReportSummary) -> [(&'static str, Vec<String>); 6] {
    [
        ("Architecture", architecture_items(summary)),
        ("Dependency Hygiene", dependency_items(summary)),
        ("Cleanup", cleanup_items(summary)),
        ("Quality", quality_items(summary)),
        ("Flutter", flutter_items(summary)),
        ("Security", security_items(summary)),
    ]
}

pub(super) fn omitted_findings_message(summary: &ReportSummary, verdict: Verdict) -> String {
    match summary.findings {
        1 => "1 finding was omitted from this summary output. Re-run without --summary to inspect it."
            .to_owned(),
        count if count > 1 => format!(
            "{count} findings were omitted from this summary output. Re-run without --summary to inspect them."
        ),
        _ if verdict == Verdict::Fail => {
            "Finding details were omitted from this failing report. Re-run without --summary to inspect them."
                .to_owned()
        }
        _ => "Finding details were omitted from this report. Re-run without --summary to inspect them."
            .to_owned(),
    }
}

fn architecture_items(summary: &ReportSummary) -> Vec<String> {
    let mut items = Vec::new();
    push_count(
        &mut items,
        summary.cycles,
        "circular dependency",
        "circular dependencies",
    );
    push_count(
        &mut items,
        summary.re_export_cycles,
        "re-export cycle",
        "re-export cycles",
    );
    push_count(
        &mut items,
        summary.boundary_violations,
        "boundary violation",
        "boundary violations",
    );
    push_count(
        &mut items,
        summary.boundary_coverage,
        "boundary coverage gap",
        "boundary coverage gaps",
    );
    push_count(
        &mut items,
        summary.boundary_call_violations,
        "boundary call violation",
        "boundary call violations",
    );
    push_count(
        &mut items,
        summary.policy_violations,
        "policy violation",
        "policy violations",
    );
    push_count(
        &mut items,
        summary.route_collisions,
        "route collision",
        "route collisions",
    );
    push_count(
        &mut items,
        summary.part_of_violations,
        "part-of violation",
        "part-of violations",
    );
    push_count(
        &mut items,
        summary.unresolved_dependencies,
        "unresolved local dependency",
        "unresolved local dependencies",
    );
    items
}

fn dependency_items(summary: &ReportSummary) -> Vec<String> {
    let mut items = Vec::new();
    push_count(
        &mut items,
        summary.unused_dependencies,
        "unused dependency",
        "unused dependencies",
    );
    push_count(
        &mut items,
        summary.unused_dev_dependencies,
        "unused dev dependency",
        "unused dev dependencies",
    );
    push_count(
        &mut items,
        summary.test_only_dependencies,
        "test-only production dependency",
        "test-only production dependencies",
    );
    push_count(
        &mut items,
        summary.unlisted_dependencies,
        "unlisted dependency",
        "unlisted dependencies",
    );
    push_count(
        &mut items,
        summary.private_src_imports,
        "private lib/src import",
        "private lib/src imports",
    );
    push_count(
        &mut items,
        summary.dependency_overrides,
        "dependency override issue",
        "dependency override issues",
    );
    items
}

fn cleanup_items(summary: &ReportSummary) -> Vec<String> {
    let mut items = Vec::new();
    push_count(&mut items, summary.dead_files, "dead file", "dead files");
    push_count(
        &mut items,
        summary.unused_exports,
        "unused export",
        "unused exports",
    );
    push_count(
        &mut items,
        summary.unused_types,
        "unused type",
        "unused types",
    );
    push_count(
        &mut items,
        summary.unused_enum_members,
        "unused enum member",
        "unused enum members",
    );
    push_count(
        &mut items,
        summary.unused_class_members,
        "unused class member",
        "unused class members",
    );
    push_count(
        &mut items,
        summary.duplicate_exports,
        "duplicate export",
        "duplicate exports",
    );
    push_count(
        &mut items,
        summary.missing_suppression_reasons,
        "suppression without reason",
        "suppressions without reasons",
    );
    items
}

fn quality_items(summary: &ReportSummary) -> Vec<String> {
    let mut items = Vec::new();
    if summary.code_duplications > 0 {
        items.push(format!(
            "{} duplicate {} covering {} {} ({})",
            summary.code_duplications,
            plural(summary.code_duplications, "group", "groups"),
            summary.duplicated_lines,
            plural(summary.duplicated_lines, "line", "lines"),
            format_basis_points(summary.duplication_percentage_basis_points)
        ));
    }
    push_count(
        &mut items,
        summary.complex_functions,
        "complex function",
        "complex functions",
    );
    push_count(
        &mut items,
        summary.coverage_gaps,
        "coverage gap",
        "coverage gaps",
    );
    push_count(
        &mut items,
        summary.crap_functions,
        "high CRAP function",
        "high CRAP functions",
    );
    push_count(&mut items, summary.hotspots, "hotspot", "hotspots");
    push_count(
        &mut items,
        summary.refactoring_targets,
        "refactoring target",
        "refactoring targets",
    );
    items
}

fn flutter_items(summary: &ReportSummary) -> Vec<String> {
    let mut items = Vec::new();
    push_count(
        &mut items,
        summary.private_widget_classes,
        "private widget class",
        "private widget classes",
    );
    push_count(
        &mut items,
        summary.widget_top_level_functions,
        "top-level widget helper",
        "top-level widget helpers",
    );
    push_count(
        &mut items,
        summary.unused_widget_params,
        "unused widget parameter",
        "unused widget parameters",
    );
    push_count(
        &mut items,
        summary.unrendered_widgets,
        "unrendered widget",
        "unrendered widgets",
    );
    push_count(
        &mut items,
        summary.missing_context_mounted_after_await,
        "missing context.mounted guard",
        "missing context.mounted guards",
    );
    items
}

fn security_items(summary: &ReportSummary) -> Vec<String> {
    let mut items = Vec::new();
    push_count(
        &mut items,
        summary.security_candidates,
        "security candidate",
        "security candidates",
    );
    push_count(
        &mut items,
        summary.feature_flags,
        "feature flag",
        "feature flags",
    );
    items
}

fn push_count(items: &mut Vec<String>, count: usize, singular: &str, plural_label: &str) {
    if count > 0 {
        items.push(format!("{count} {}", plural(count, singular, plural_label)));
    }
}

const fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}

fn format_basis_points(points: u32) -> String {
    let whole = points / 100;
    let fraction = points % 100;
    if fraction == 0 {
        return format!("{whole}%");
    }
    format!("{whole}.{fraction:02}%")
}

pub(super) const fn kind_label(kind: FindingKind) -> &'static str {
    match kind {
        FindingKind::DeadFile => "Dead file",
        FindingKind::UnusedExport => "Unused export",
        FindingKind::UnusedType => "Unused type",
        FindingKind::PrivateTypeLeak => "Private type leak",
        FindingKind::UnusedEnumMember => "Unused enum member",
        FindingKind::UnusedClassMember => "Unused class member",
        FindingKind::DuplicateExport => "Duplicate export",
        FindingKind::RouteCollision => "Route collision",
        FindingKind::PrivateWidgetClass => "Private widget class",
        FindingKind::WidgetTopLevelFunctionBoundary => "Top-level widget helper",
        FindingKind::UnusedWidgetParam => "Unused widget parameter",
        FindingKind::UnrenderedWidget => "Unrendered widget",
        FindingKind::MissingContextMountedAfterAwait => "Missing context.mounted guard",
        FindingKind::MissingEntryPoint => "Missing entry point",
        FindingKind::CircularDependency => "Circular dependency",
        FindingKind::ReExportCycle => "Re-export cycle",
        FindingKind::BoundaryViolation => "Boundary violation",
        FindingKind::BoundaryCoverage => "Boundary coverage gap",
        FindingKind::BoundaryCallViolation => "Boundary call violation",
        FindingKind::PolicyViolation => "Policy violation",
        FindingKind::UnresolvedDependency => "Unresolved dependency",
        FindingKind::PartOfViolation => "Part-of violation",
        FindingKind::UnusedDependency => "Unused dependency",
        FindingKind::UnusedDevDependency => "Unused dev dependency",
        FindingKind::TestOnlyDependency => "Test-only production dependency",
        FindingKind::UnusedDependencyOverride => "Unused dependency override",
        FindingKind::MisconfiguredDependencyOverride => "Misconfigured dependency override",
        FindingKind::UnlistedDependency => "Unlisted dependency",
        FindingKind::PrivateSrcImport => "Private lib/src import",
        FindingKind::CodeDuplication => "Code duplication",
        FindingKind::HighCyclomaticComplexity => "High cyclomatic complexity",
        FindingKind::HighCognitiveComplexity => "High cognitive complexity",
        FindingKind::HighComplexity => "High complexity",
        FindingKind::CoverageGap => "Coverage gap",
        FindingKind::HighCrapScore => "High CRAP score",
        FindingKind::HealthHotspot => "Health hotspot",
        FindingKind::RefactoringTarget => "Refactoring target",
        FindingKind::FeatureFlag => "Feature flag",
        FindingKind::SecurityCandidate => "Security candidate",
        FindingKind::StaleSuppression => "Stale suppression",
        FindingKind::MissingSuppressionReason => "Missing suppression reason",
    }
}

pub(super) const fn why_text(kind: FindingKind) -> &'static str {
    match kind {
        FindingKind::CircularDependency => {
            "These files import or export each other in a loop. That couples builds, tests, ownership, and refactors across the whole component."
        }
        FindingKind::ReExportCycle => {
            "Barrel exports loop back into each other, so public API propagation is hard to reason about and can expose symbols accidentally."
        }
        FindingKind::BoundaryViolation => {
            "A file depends on a layer or module that the configured architecture boundary disallows."
        }
        FindingKind::BoundaryCoverage => {
            "A Dart file is outside every configured architecture zone, so boundary checks cannot prove who owns it."
        }
        FindingKind::BoundaryCallViolation => {
            "Code in a configured zone directly calls an API pattern that should sit behind an allowed boundary."
        }
        FindingKind::PolicyViolation => {
            "A declarative rule pack matched this file, import, export, or call pattern."
        }
        FindingKind::DeadFile => {
            "No configured entry point reaches this Dart file through imports, exports, parts, or augmentation edges."
        }
        FindingKind::MissingEntryPoint => {
            "The configured entry path was not found in the parsed Dart module graph."
        }
        FindingKind::UnresolvedDependency => {
            "A local import, export, part, or augmentation URI points at a Dart file that Dart Decimate could not resolve."
        }
        FindingKind::PartOfViolation => {
            "A Dart part file and its owning library disagree, or one side of the part relationship is missing."
        }
        FindingKind::UnusedDependency
        | FindingKind::UnusedDevDependency
        | FindingKind::TestOnlyDependency
        | FindingKind::UnusedDependencyOverride
        | FindingKind::MisconfiguredDependencyOverride
        | FindingKind::UnlistedDependency => {
            "The pubspec dependency graph and the Dart import graph do not agree."
        }
        FindingKind::PrivateSrcImport => {
            "The import reaches into another package's private lib/src implementation surface."
        }
        FindingKind::UnusedExport | FindingKind::UnusedType => {
            "The public declaration is not referenced from reachable Dart files in the current graph."
        }
        FindingKind::UnusedEnumMember | FindingKind::UnusedClassMember => {
            "The member is not referenced from reachable Dart files in the current graph."
        }
        FindingKind::DuplicateExport => {
            "One public API entry exposes more than one declaration with the same name."
        }
        FindingKind::RouteCollision => {
            "Two typed route declarations resolve to the same route name or path."
        }
        FindingKind::CodeDuplication => {
            "The same token sequence appears in multiple Dart locations, increasing maintenance cost when behavior changes."
        }
        FindingKind::HighCyclomaticComplexity
        | FindingKind::HighCognitiveComplexity
        | FindingKind::HighComplexity => {
            "The function has enough branch or nesting decisions to raise review and change risk."
        }
        FindingKind::CoverageGap => {
            "Runtime coverage data found no covered executable lines for this Dart file."
        }
        FindingKind::HighCrapScore => {
            "The function combines high branching complexity with low test coverage."
        }
        FindingKind::HealthHotspot | FindingKind::RefactoringTarget => {
            "Size, complexity, duplication, coupling, coverage, or ownership signals make this file expensive to change."
        }
        FindingKind::FeatureFlag => {
            "Feature flag usage is present and should be traceable before cleanup or rollout work."
        }
        FindingKind::SecurityCandidate => {
            "Static graph evidence found code that deserves security review before changing or trusting it."
        }
        FindingKind::PrivateTypeLeak => {
            "A public API exposes a private Dart library type, making the public contract harder to consume safely."
        }
        FindingKind::PrivateWidgetClass
        | FindingKind::WidgetTopLevelFunctionBoundary
        | FindingKind::UnusedWidgetParam
        | FindingKind::UnrenderedWidget
        | FindingKind::MissingContextMountedAfterAwait => {
            "Flutter-specific graph evidence found code whose structure can drift from reachable UI behavior."
        }
        FindingKind::StaleSuppression => {
            "A suppression comment no longer matches an active finding."
        }
        FindingKind::MissingSuppressionReason => {
            "A suppression comment exists without the required reason text."
        }
    }
}

pub(super) fn best_text(finding: &Finding, action: &FindingAction) -> String {
    match finding.kind {
        FindingKind::CircularDependency => {
            "Break one import/export edge first. Move shared code inward, invert the dependency, or remove an unnecessary barrel hop.".to_owned()
        }
        FindingKind::ReExportCycle => {
            "Break one export edge first. Prefer a single public barrel that points outward without looping back.".to_owned()
        }
        FindingKind::CodeDuplication => {
            "Trace the clone group, then extract shared code only if the duplicated blocks should change together.".to_owned()
        }
        _ if finding.safe_to_delete || action.auto_fixable => {
            format!("{} This is marked safe-to-delete from graph evidence, but still review generated and dynamic entry points.", action.description)
        }
        _ => action.description.clone(),
    }
}

pub(super) fn fallback_best(kind: FindingKind) -> String {
    format!(
        "Run `dart-decimate inspect --format json` for this {} before editing.",
        kind_label(kind).to_ascii_lowercase()
    )
}
