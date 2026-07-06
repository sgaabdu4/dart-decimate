use super::format::{dependency_kind, display_path};
use super::route_cycle::is_typed_go_router_registry_cycle;
use super::{Finding, FindingAction, FindingEdge, FindingKind, Severity};
use crate::{
    BoundaryCallViolation, BoundaryCoverageGap, BoundaryViolation, DeadCodeReport, DependencyCycle,
    InvalidPartReason, InvalidPartRelationship, PolicySeverity, PolicyViolation, ReExportCycle,
    UnresolvedDependency, scan::ScannedProject,
};

pub(super) fn add_dead_code_findings(
    root: &std::path::Path,
    dead_code: &DeadCodeReport,
    findings: &mut Vec<Finding>,
) {
    for path in &dead_code.missing_entry_points {
        let path = display_path(root, path);
        findings.push(Finding {
            rule_id: "dart-decimate/missing-entry-point".to_owned(),
            fingerprint: None,
            kind: FindingKind::MissingEntryPoint,
            severity: Severity::Error,
            message: format!("Entry point was not found in the module graph: {path}"),
            path: path.clone(),
            line: 1,
            column: 0,
            safe_to_delete: false,
            files: Vec::new(),
            edge: None,
            actions: vec![
                FindingAction::new(
                    "fix-entry-point",
                    "Pass an existing Dart entry point with --entry",
                    false,
                )
                .with_target_path(path.clone())
                .with_config_key("entry")
                .with_value_schema("array of Dart entry point paths"),
            ],
        });
    }

    for dead_file in &dead_code.dead_files {
        let path = display_path(root, &dead_file.path);
        findings.push(Finding {
            rule_id: "dart-decimate/dead-file".to_owned(),
            fingerprint: None,
            kind: FindingKind::DeadFile,
            severity: Severity::Error,
            message: format!("Dart file is unreachable from the configured entry points: {path}"),
            path: path.clone(),
            line: 1,
            column: 0,
            safe_to_delete: dead_file.safe_to_delete,
            files: Vec::new(),
            edge: None,
            actions: vec![FindingAction::new(
                "delete-file",
                "Delete the unreachable Dart file after confirming no dynamic entry point uses it",
                dead_file.safe_to_delete,
            )
            .with_target_path(path.clone())
            .with_dart_decimate_args([
                "inspect",
                "--format",
                "json",
                "--file",
                path.as_str(),
            ])],
        });
    }
}

pub(super) fn add_cycle_findings(
    project: &ScannedProject,
    cycles: &[DependencyCycle],
    findings: &mut Vec<Finding>,
) {
    for cycle in cycles {
        let classification = cycle_classification(project, cycle);
        let files = cycle
            .files
            .iter()
            .map(|path| display_path(&project.root, path))
            .collect::<Vec<_>>();
        let path = files.first().cloned().unwrap_or_default();
        findings.push(Finding {
            rule_id: "dart-decimate/circular-dependency".to_owned(),
            fingerprint: None,
            kind: FindingKind::CircularDependency,
            severity: classification.severity,
            message: classification.message(files.len()),
            path: path.clone(),
            line: 1,
            column: 0,
            safe_to_delete: false,
            files,
            edge: None,
            actions: vec![
                FindingAction::new(classification.action, classification.description, false)
                    .with_target_path(path.clone())
                    .with_dart_decimate_args([
                        "inspect",
                        "--format",
                        "json",
                        "--file",
                        path.as_str(),
                    ]),
            ],
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CycleClassification {
    severity: Severity,
    action: &'static str,
    description: &'static str,
    route_registry: bool,
}

impl CycleClassification {
    fn message(self, file_count: usize) -> String {
        if self.route_registry {
            format!("Typed GoRouter route registry cycle spans {file_count} Dart files")
        } else {
            format!("Circular dependency spans {file_count} Dart files")
        }
    }
}

fn cycle_classification(project: &ScannedProject, cycle: &DependencyCycle) -> CycleClassification {
    if is_typed_go_router_registry_cycle(project, cycle) {
        return CycleClassification {
            severity: Severity::Warning,
            action: "review-typed-route-cycle",
            description: "Keep typed routes if this is only the route registry to screen navigation helper cycle; split unrelated imports out of the cycle",
            route_registry: true,
        };
    }

    CycleClassification {
        severity: Severity::Error,
        action: "break-cycle",
        description: "Inspect the cycle edge; split barrels or move shared ownership before expanding imports",
        route_registry: false,
    }
}

pub(super) fn add_re_export_cycle_findings(
    root: &std::path::Path,
    cycles: &[ReExportCycle],
    findings: &mut Vec<Finding>,
) {
    for cycle in cycles {
        let files = cycle
            .files
            .iter()
            .map(|path| display_path(root, path))
            .collect::<Vec<_>>();
        let path = files.first().cloned().unwrap_or_default();
        findings.push(Finding {
            rule_id: "dart-decimate/re-export-cycle".to_owned(),
            fingerprint: None,
            kind: FindingKind::ReExportCycle,
            severity: Severity::Error,
            message: format!("Re-export cycle spans {} Dart files", files.len()),
            path: path.clone(),
            line: 1,
            column: 0,
            safe_to_delete: false,
            files,
            edge: None,
            actions: vec![
                FindingAction::new(
                    "break-re-export-cycle",
                    "Remove or redirect one barrel export so public API propagation is acyclic",
                    false,
                )
                .with_target_path(path.clone())
                .with_dart_decimate_args([
                    "inspect",
                    "--format",
                    "json",
                    "--file",
                    path.as_str(),
                ]),
            ],
        });
    }
}

pub(super) fn add_boundary_findings(
    root: &std::path::Path,
    violations: &[BoundaryViolation],
    findings: &mut Vec<Finding>,
) {
    for violation in violations {
        let from = display_path(root, &violation.from_path);
        let to = display_path(root, &violation.to_path);
        findings.push(Finding {
            rule_id: "dart-decimate/boundary-violation".to_owned(),
            fingerprint: None,
            kind: FindingKind::BoundaryViolation,
            severity: Severity::Error,
            message: format!("{from} must not depend on {to}"),
            path: from.clone(),
            line: violation.location.line,
            column: violation.location.column,
            safe_to_delete: false,
            files: vec![from.clone(), to.clone()],
            edge: Some(FindingEdge {
                from: from.clone(),
                to,
                specifier: violation.specifier.clone(),
                kind: dependency_kind(violation.kind),
            }),
            actions: vec![
                FindingAction::new(
                    "repair-boundary",
                    "Move the dependency behind an allowed boundary or invert the ownership",
                    false,
                )
                .with_target_path(from.clone())
                .with_dart_decimate_args(["inspect", "--format", "json", "--file", from.as_str()])
                .with_suppression_comment("// dart-decimate-ignore-next-line boundary-violation"),
            ],
        });
    }
}

pub(super) fn add_boundary_coverage_findings(
    root: &std::path::Path,
    gaps: &[BoundaryCoverageGap],
    findings: &mut Vec<Finding>,
) {
    for gap in gaps {
        let path = display_path(root, &gap.path);
        let zones = gap
            .configured_boundaries
            .iter()
            .map(|boundary| display_path(root, boundary))
            .collect::<Vec<_>>();
        findings.push(Finding {
            rule_id: "dart-decimate/boundary-violation".to_owned(),
            fingerprint: None,
            kind: FindingKind::BoundaryCoverage,
            severity: Severity::Error,
            message: format!("{path} is not covered by any configured architecture boundary"),
            path: path.clone(),
            line: gap.location.line,
            column: gap.location.column,
            safe_to_delete: false,
            files: zones,
            edge: None,
            actions: vec![
                FindingAction::new(
                    "assign-boundary",
                    "Move the file into a configured boundary or add an intentional boundary zone",
                    false,
                )
                .with_target_path(path.clone())
                .with_config_key("boundary")
                .with_value_schema("array of FROM:DISALLOW architecture boundary rules")
                .with_suppression_comment("// dart-decimate-ignore-next-line boundary-violation"),
            ],
        });
    }
}

pub(super) fn add_boundary_call_findings(
    root: &std::path::Path,
    violations: &[BoundaryCallViolation],
    findings: &mut Vec<Finding>,
) {
    for violation in violations {
        let path = display_path(root, &violation.path);
        findings.push(Finding {
            rule_id: "dart-decimate/boundary-violation".to_owned(),
            fingerprint: None,
            kind: FindingKind::BoundaryCallViolation,
            severity: Severity::Error,
            message: format!(
                "{path} calls {} matching forbidden boundary pattern {}",
                violation.callee, violation.pattern
            ),
            path: path.clone(),
            line: violation.location.line,
            column: violation.location.column,
            safe_to_delete: false,
            files: vec![display_path(root, &violation.from_boundary)],
            edge: None,
            actions: vec![FindingAction::new(
                "repair-boundary-call",
                "Move the call behind an allowed boundary or replace it with an owned abstraction",
                false,
            )
            .with_target_path(path.clone())
            .with_config_key("boundary_calls")
            .with_value_schema("array of FROM:PATTERN forbidden direct call rules")
            .with_suppression_comment("// dart-decimate-ignore-next-line boundary-call-violation")],
        });
    }
}

pub(super) fn add_policy_findings(
    root: &std::path::Path,
    violations: &[PolicyViolation],
    findings: &mut Vec<Finding>,
) {
    for violation in violations {
        let path = display_path(root, &violation.path);
        let message = violation.message.clone().unwrap_or_else(|| {
            format!(
                "{} matches policy pattern {}",
                violation.target, violation.pattern
            )
        });
        findings.push(Finding {
            rule_id: violation.rule_id.clone(),
            fingerprint: None,
            kind: FindingKind::PolicyViolation,
            severity: policy_severity(violation.severity),
            message,
            path: path.clone(),
            line: violation.location.line,
            column: violation.location.column,
            safe_to_delete: false,
            files: Vec::new(),
            edge: None,
            actions: vec![
                FindingAction::new(
                    "repair-policy-violation",
                    "Change the import or call so it complies with the owning rule pack",
                    false,
                )
                .with_target_path(path.clone())
                .with_config_key("rulePacks")
                .with_value_schema("array of declarative policy pack paths")
                .with_suppression_comment(format!(
                    "// dart-decimate-ignore-next-line policy-violation {}",
                    violation.rule_id
                )),
            ],
        });
    }
}

const fn policy_severity(severity: Option<PolicySeverity>) -> Severity {
    match severity {
        Some(PolicySeverity::Error) => Severity::Error,
        Some(PolicySeverity::Warn) | None => Severity::Warning,
    }
}

pub(super) fn add_unresolved_findings(project: &ScannedProject, findings: &mut Vec<Finding>) {
    for dependency in project.graph.unresolved() {
        if dependency.from_path.starts_with(&project.root) {
            findings.push(unresolved_finding(&project.root, dependency));
        }
    }
}

pub(super) fn add_part_of_findings(project: &ScannedProject, findings: &mut Vec<Finding>) {
    for relationship in project.graph.invalid_part_relationships() {
        if relationship.part_path.starts_with(&project.root) {
            findings.push(part_of_finding(&project.root, relationship));
        }
    }
}

pub(super) fn project_unresolved_count(project: &ScannedProject) -> usize {
    project
        .graph
        .unresolved()
        .iter()
        .filter(|dependency| dependency.from_path.starts_with(&project.root))
        .count()
}

pub(super) fn project_part_of_violation_count(project: &ScannedProject) -> usize {
    project
        .graph
        .invalid_part_relationships()
        .iter()
        .filter(|relationship| relationship.part_path.starts_with(&project.root))
        .count()
}

fn unresolved_finding(root: &std::path::Path, dependency: &UnresolvedDependency) -> Finding {
    let from = display_path(root, &dependency.from_path);
    let attempted = display_path(root, &dependency.attempted_path);
    Finding {
        rule_id: "dart-decimate/unresolved-dependency".to_owned(),
        fingerprint: None,
        kind: FindingKind::UnresolvedDependency,
        severity: Severity::Error,
        message: format!(
            "Local dependency target was not found: {}",
            dependency.specifier
        ),
        path: from.clone(),
        line: dependency.location.line,
        column: dependency.location.column,
        safe_to_delete: false,
        files: vec![from.clone(), attempted.clone()],
        edge: Some(FindingEdge {
            from: from.clone(),
            to: attempted,
            specifier: dependency.specifier.clone(),
            kind: dependency_kind(dependency.kind),
        }),
        actions: vec![
            FindingAction::new(
                "fix-import",
                "Update the dependency URI or add the missing Dart file",
                false,
            )
            .with_target_path(from.clone())
            .with_dart_decimate_args(["inspect", "--format", "json", "--file", from.as_str()])
            .with_suppression_comment("// dart-decimate-ignore-next-line unresolved-dependency"),
        ],
    }
}

fn part_of_finding(root: &std::path::Path, relationship: &InvalidPartRelationship) -> Finding {
    let part = display_path(root, &relationship.part_path);
    let library = relationship
        .library_path
        .as_ref()
        .map(|path| display_path(root, path));
    let files = library
        .iter()
        .cloned()
        .chain(std::iter::once(part.clone()))
        .collect::<Vec<_>>();
    Finding {
        rule_id: "dart-decimate/part-of-violation".to_owned(),
        fingerprint: None,
        kind: FindingKind::PartOfViolation,
        severity: Severity::Error,
        message: part_of_message(root, relationship),
        path: part.clone(),
        line: relationship.location.line,
        column: relationship.location.column,
        safe_to_delete: false,
        files,
        edge: library.map(|library| FindingEdge {
            from: library,
            to: part.clone(),
            specifier: relationship.specifier.clone(),
            kind: "part".to_owned(),
        }),
        actions: vec![
            FindingAction::new(
                "repair-part-of",
                "Update the library part directive or the part file's part of directive",
                false,
            )
            .with_target_path(part.clone())
            .with_dart_decimate_args(["inspect", "--format", "json", "--file", part.as_str()])
            .with_suppression_comment("// dart-decimate-ignore-next-line part-of-violation"),
        ],
    }
}

fn part_of_message(root: &std::path::Path, relationship: &InvalidPartRelationship) -> String {
    match &relationship.reason {
        InvalidPartReason::MissingPartOf => {
            "Dart part file is missing a matching part of directive".to_owned()
        }
        InvalidPartReason::EmptyPartOf => {
            "Dart part file has an empty part of directive".to_owned()
        }
        InvalidPartReason::OrphanPartOf { .. } => {
            "Dart part file has no owning library part directive".to_owned()
        }
        InvalidPartReason::DuplicatePartOwner {
            existing_library_path,
        } => format!(
            "Dart part file is already owned by another library: {}",
            display_path(root, existing_library_path)
        ),
        InvalidPartReason::PartOfUriUnresolved { actual_specifier } => {
            format!("Dart part of URI could not be resolved: {actual_specifier}")
        }
        InvalidPartReason::PartOfUriMismatch {
            actual_specifier, ..
        } => format!("Dart part of URI points at a different library: {actual_specifier}"),
        InvalidPartReason::PartOfNameMismatch {
            expected_name,
            actual_name,
        } => format!(
            "Dart part of library name mismatch: expected {}, found {actual_name}",
            expected_name.as_deref().unwrap_or("<unnamed library>")
        ),
    }
}
