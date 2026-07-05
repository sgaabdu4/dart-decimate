use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use tree_sitter::Node;

use super::format::{dependency_kind, display_path};
use super::{Finding, FindingAction, FindingEdge, FindingKind, Severity};
use crate::{
    BoundaryCallViolation, BoundaryCoverageGap, BoundaryViolation, DartFile, DeadCodeReport,
    DependencyCycle, DependencyKind, InvalidPartReason, InvalidPartRelationship, PolicySeverity,
    PolicyViolation, ReExportCycle, ResolvedDependency, UnresolvedDependency, scan::ScannedProject,
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

fn is_typed_go_router_registry_cycle(project: &ScannedProject, cycle: &DependencyCycle) -> bool {
    let cycle_files = cycle.files.iter().cloned().collect::<BTreeSet<_>>();
    let route_files = project
        .files
        .iter()
        .filter(|file| cycle_files.contains(&file.path) && is_typed_go_router_registry(file))
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    if route_files.is_empty() {
        return false;
    }

    let dependencies = project.graph.dependencies();
    let internal_dependencies = dependencies
        .iter()
        .filter(|dependency| {
            cycle_files.contains(&dependency.from_path)
                && cycle_files.contains(&dependency.to_path)
                && dependency.from_path != dependency.to_path
        })
        .collect::<Vec<_>>();
    let files_by_path = project
        .files
        .iter()
        .map(|file| (file.path.clone(), file))
        .collect::<BTreeMap<_, _>>();
    if internal_dependencies.is_empty()
        || internal_dependencies.iter().any(|dependency| {
            !is_typed_go_router_registry_edge(dependency, &route_files, &files_by_path)
        })
    {
        return false;
    }

    let mut route_helpers = BTreeMap::<PathBuf, BTreeSet<PathBuf>>::new();
    for dependency in &internal_dependencies {
        if route_files.contains(&dependency.from_path) {
            route_helpers
                .entry(dependency.from_path.clone())
                .or_default()
                .insert(dependency.to_path.clone());
        }
    }

    let helper_files = cycle_files
        .difference(&route_files)
        .cloned()
        .collect::<BTreeSet<_>>();
    !helper_files.is_empty()
        && route_files.iter().all(|route_file| {
            route_helpers
                .get(route_file)
                .is_some_and(|helpers| !helpers.is_empty())
        })
        && helper_files.iter().all(|helper_file| {
            route_files.iter().any(|route_file| {
                route_helpers
                    .get(route_file)
                    .is_some_and(|helpers| helpers.contains(helper_file))
                    && internal_dependencies.iter().any(|dependency| {
                        dependency.from_path == *helper_file && dependency.to_path == *route_file
                    })
            })
        })
}

fn is_typed_go_router_registry_edge(
    dependency: &ResolvedDependency,
    route_files: &BTreeSet<PathBuf>,
    files_by_path: &BTreeMap<PathBuf, &DartFile>,
) -> bool {
    if dependency.kind != DependencyKind::Import {
        return false;
    }
    let from_is_route = route_files.contains(&dependency.from_path);
    let to_is_route = route_files.contains(&dependency.to_path);
    if from_is_route == to_is_route {
        return false;
    }
    let route_path = if from_is_route {
        &dependency.from_path
    } else {
        &dependency.to_path
    };
    let helper_path = if from_is_route {
        &dependency.to_path
    } else {
        &dependency.from_path
    };
    let Some(route_file) = files_by_path.get(route_path) else {
        return false;
    };
    let Some(helper_file) = files_by_path.get(helper_path) else {
        return false;
    };
    is_typed_go_router_navigation_helper(helper_file, route_file)
}

fn is_typed_go_router_navigation_helper(helper_file: &DartFile, route_file: &DartFile) -> bool {
    let route_classes = route_file
        .routes
        .iter()
        .map(|route| route.route_class.clone())
        .collect::<BTreeSet<_>>();
    if route_classes.is_empty() {
        return false;
    }
    let Ok(source) = fs::read_to_string(&helper_file.path) else {
        return false;
    };
    let Ok(parsed) = crate::dart_parser::parse_dart_source_lossy(&helper_file.path, &source) else {
        return false;
    };
    helper_has_typed_route_navigation_call(
        parsed.tree().root_node(),
        parsed.source(),
        &route_classes,
    )
}

fn helper_has_typed_route_navigation_call(
    root: Node<'_>,
    source: &str,
    route_classes: &BTreeSet<String>,
) -> bool {
    let mut found = false;
    visit_named(root, &mut |node| {
        if found {
            return;
        }
        if typed_route_navigation_call(node, source, route_classes) {
            found = true;
        }
    });
    found
}

fn typed_route_navigation_call(
    node: Node<'_>,
    source: &str,
    route_classes: &BTreeSet<String>,
) -> bool {
    if !matches!(
        node.kind(),
        "call_expression" | "function_expression_invocation"
    ) {
        return false;
    }
    let Some(arguments) = argument_list(node) else {
        return false;
    };
    let Some(prefix) = source.get(node.start_byte()..arguments.start_byte()) else {
        return false;
    };
    let Some(navigation_name) = navigation_call_name(prefix) else {
        return false;
    };
    route_extension_navigation_call(prefix, &navigation_name, route_classes)
        || arguments_contain_route_location(arguments, route_classes, source)
}

fn argument_list(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("arguments")
        .or_else(|| direct_named_child(node, "arguments"))
        .or_else(|| direct_named_child(node, "argument_part"))
}

fn navigation_call_name(prefix: &str) -> Option<String> {
    let compact = strip_whitespace(prefix);
    let name = compact.rsplit('.').next().unwrap_or(compact.as_str());
    is_typed_route_navigation_reference(name).then(|| name.to_owned())
}

fn route_extension_navigation_call(
    prefix: &str,
    navigation_name: &str,
    route_classes: &BTreeSet<String>,
) -> bool {
    let compact = strip_whitespace(prefix);
    let suffix = format!(".{navigation_name}");
    let Some(receiver) = compact.strip_suffix(&suffix) else {
        return false;
    };
    let receiver = receiver
        .strip_prefix("const")
        .or_else(|| receiver.strip_prefix("new"))
        .unwrap_or(receiver);
    route_classes
        .iter()
        .any(|route_class| contains_constructor_call(receiver, route_class))
}

fn arguments_contain_route_location(
    arguments: Node<'_>,
    route_classes: &BTreeSet<String>,
    source: &str,
) -> bool {
    let mut found = false;
    visit_named(arguments, &mut |node| {
        if found {
            return;
        }
        if route_location_member(node, route_classes, source) {
            found = true;
        }
    });
    found
}

fn route_location_member(node: Node<'_>, route_classes: &BTreeSet<String>, source: &str) -> bool {
    if !matches!(
        node.kind(),
        "member_expression" | "null_aware_member_expression" | "assignable_expression"
    ) {
        return false;
    }
    let Some(property) = node.child_by_field_name("property") else {
        return false;
    };
    if property.utf8_text(source.as_bytes()).ok() != Some("location") {
        return false;
    }
    node.child_by_field_name("object")
        .is_some_and(|object| node_contains_route_constructor(object, route_classes, source))
}

fn node_contains_route_constructor(
    node: Node<'_>,
    route_classes: &BTreeSet<String>,
    source: &str,
) -> bool {
    if matches!(
        node.kind(),
        "constructor_invocation" | "const_object_expression" | "new_expression"
    ) && node
        .child_by_field_name("type")
        .and_then(|type_node| type_node.utf8_text(source.as_bytes()).ok())
        .map(simple_type_name)
        .is_some_and(|type_name| route_classes.contains(&type_name))
    {
        return true;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| node_contains_route_constructor(child, route_classes, source))
}

fn contains_constructor_call(text: &str, route_class: &str) -> bool {
    let mut cursor = 0usize;
    while let Some(relative) = text[cursor..].find(route_class) {
        let start = cursor + relative;
        let end = start + route_class.len();
        if !identifier_continues_before(text, start)
            && text
                .get(end..)
                .is_some_and(|after| after.starts_with('(') || after.starts_with('<'))
        {
            return true;
        }
        cursor = end;
    }
    false
}

fn identifier_continues_before(text: &str, index: usize) -> bool {
    text[..index]
        .chars()
        .next_back()
        .is_some_and(is_identifier_character)
}

fn is_identifier_character(character: char) -> bool {
    character == '_' || character == '$' || character.is_ascii_alphanumeric()
}

fn simple_type_name(text: &str) -> String {
    text.trim_end_matches('?')
        .rsplit('.')
        .next()
        .unwrap_or(text)
        .split('<')
        .next()
        .unwrap_or(text)
        .to_owned()
}

fn is_typed_route_navigation_reference(name: &str) -> bool {
    matches!(
        name,
        "go" | "goNamed"
            | "push"
            | "pushNamed"
            | "pushReplacement"
            | "pushReplacementNamed"
            | "replace"
            | "replaceNamed"
    )
}

fn is_typed_go_router_registry(file: &DartFile) -> bool {
    if file.routes.is_empty()
        && !file.references.iter().any(|reference| {
            matches!(
                reference.name.as_str(),
                "TypedGoRoute"
                    | "TypedRelativeGoRoute"
                    | "TypedShellRoute"
                    | "TypedStatefulShellRoute"
                    | "TypedStatefulShellBranch"
            )
        })
    {
        return false;
    }

    file.parts.iter().any(|part| part.uri.ends_with(".g.dart"))
}

fn direct_named_child<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == kind)
}

fn strip_whitespace(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn visit_named(node: Node<'_>, visitor: &mut impl FnMut(Node<'_>)) {
    visitor(node);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visit_named(child, visitor);
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
