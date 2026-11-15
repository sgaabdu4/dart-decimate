use std::path::Path;

use super::format::{declaration_kind, display_path};
use super::{Finding, FindingAction, FindingKind, Severity, SymbolReport};
use crate::{
    DeclarationKind, DuplicateExport, MemberKind, PrivateTypeLeak, UnusedExport, UnusedMember,
};

pub(super) fn add_symbol_findings(root: &Path, report: &SymbolReport, findings: &mut Vec<Finding>) {
    findings.extend(
        report
            .unused_exports
            .iter()
            .map(|unused| unused_export_finding(root, unused)),
    );
    findings.extend(
        report
            .duplicate_exports
            .iter()
            .map(|duplicate| duplicate_export_finding(root, duplicate)),
    );
    findings.extend(
        report
            .private_type_leaks
            .iter()
            .map(|leak| private_type_leak_finding(root, leak)),
    );
    findings.extend(
        report
            .unused_members
            .iter()
            .map(|unused| unused_member_finding(root, unused)),
    );
}

fn private_type_leak_finding(root: &Path, leak: &PrivateTypeLeak) -> Finding {
    let path = display_path(root, &leak.path);
    let symbol_target = format!("{path}:{}", leak.declaration);
    Finding {
        rule_id: "dart-decimate/private-type-leak".to_owned(),
        fingerprint: None,
        kind: FindingKind::PrivateTypeLeak,
        severity: Severity::Error,
        message: format!(
            "Public {} signature exposes private Dart library type {}: {}",
            declaration_kind(leak.declaration_kind),
            leak.private_type,
            leak.declaration
        ),
        path: path.clone(),
        line: leak.location.line,
        column: leak.location.column,
        safe_to_delete: leak.safe_to_delete,
        files: Vec::new(),
        edge: None,
        actions: vec![
            FindingAction::new(
                "review-public-api",
                "Rename the private type, hide this declaration, or expose a public signature type",
                false,
            )
            .with_target_path(path.clone())
            .with_target_symbol(leak.declaration.clone())
            .with_dart_decimate_args([
                "inspect",
                "--format",
                "json",
                "--symbol",
                symbol_target.as_str(),
            ])
            .with_suppression_comment("// dart-decimate-ignore-next-line private-type-leak"),
        ],
    }
}

fn unused_export_finding(root: &Path, unused: &UnusedExport) -> Finding {
    let path = display_path(root, &unused.path);
    let symbol_target = format!("{path}:{}", unused.name);
    let kind = unused_top_level_kind(unused.kind);
    let mut actions = vec![
        FindingAction::new(
            "trace-symbol",
            "Trace this symbol before removing it",
            false,
        )
        .with_target_path(path.clone())
        .with_target_symbol(unused.name.clone())
        .with_dart_decimate_args([
            "inspect",
            "--format",
            "json",
            "--symbol",
            symbol_target.as_str(),
        ])
        .with_suppression_comment(format!(
            "// dart-decimate-ignore-next-line {}",
            kind_key(kind)
        )),
    ];
    if unused.safe_to_delete {
        actions.insert(
            0,
            FindingAction::new(
                "remove-declaration",
                "Remove this unused one-line top-level Dart declaration",
                true,
            )
            .with_target_path(path.clone())
            .with_target_symbol(unused.name.clone())
            .with_target_end_line(unused.location.line),
        );
    }
    Finding {
        rule_id: unused_top_level_rule_id(unused.kind).to_owned(),
        fingerprint: None,
        kind,
        severity: Severity::Error,
        message: unused_top_level_message(unused),
        path: path.clone(),
        line: unused.location.line,
        column: unused.location.column,
        safe_to_delete: unused.safe_to_delete,
        files: Vec::new(),
        edge: None,
        actions,
    }
}

const fn unused_top_level_rule_id(kind: DeclarationKind) -> &'static str {
    match kind {
        DeclarationKind::TypeAlias => "dart-decimate/unused-type",
        _ => "dart-decimate/unused-export",
    }
}

const fn unused_top_level_kind(kind: DeclarationKind) -> FindingKind {
    match kind {
        DeclarationKind::TypeAlias => FindingKind::UnusedType,
        _ => FindingKind::UnusedExport,
    }
}

fn unused_top_level_message(unused: &UnusedExport) -> String {
    if unused.kind == DeclarationKind::TypeAlias {
        return format!(
            "Public type alias is not referenced from reachable Dart files: {}",
            unused.name
        );
    }
    format!(
        "Public top-level {} is not referenced from reachable Dart files: {}",
        declaration_kind(unused.kind),
        unused.name
    )
}

fn duplicate_export_finding(root: &Path, duplicate: &DuplicateExport) -> Finding {
    let path = display_path(root, &duplicate.entry_path);
    let symbol_target = format!("{path}:{}", duplicate.name);
    let files = duplicate
        .declarations
        .iter()
        .map(|declaration| display_path(root, &declaration.path))
        .collect::<Vec<_>>();
    Finding {
        rule_id: "dart-decimate/duplicate-export".to_owned(),
        fingerprint: None,
        kind: FindingKind::DuplicateExport,
        severity: Severity::Error,
        message: format!(
            "Multiple library files declare or export a public name {}",
            duplicate.name
        ),
        path: path.clone(),
        line: 1,
        column: 0,
        safe_to_delete: duplicate.safe_to_delete,
        files,
        edge: None,
        actions: vec![
            FindingAction::new(
                "inspect-export-surface",
                "Rename, hide, or redirect one export so the public API exposes one declaration",
                false,
            )
            .with_target_path(path.clone())
            .with_target_symbol(duplicate.name.clone())
            .with_dart_decimate_args([
                "inspect",
                "--format",
                "json",
                "--symbol",
                symbol_target.as_str(),
            ]),
        ],
    }
}

fn unused_member_finding(root: &Path, unused: &UnusedMember) -> Finding {
    let path = display_path(root, &unused.path);
    let symbol_target = format!("{path}:{}", unused.owner);
    let (rule_id, kind, noun) = match unused.kind {
        MemberKind::EnumConstant => (
            "dart-decimate/unused-enum-member",
            FindingKind::UnusedEnumMember,
            "enum constant",
        ),
        _ => (
            "dart-decimate/unused-class-member",
            FindingKind::UnusedClassMember,
            if unused.name.starts_with('_') {
                "private class-like member"
            } else {
                "public class-like member"
            },
        ),
    };
    Finding {
        rule_id: rule_id.to_owned(),
        fingerprint: None,
        kind,
        severity: Severity::Error,
        message: format!(
            "Unused {} {}.{} is not referenced from reachable Dart files",
            noun, unused.owner, unused.name
        ),
        path: path.clone(),
        line: unused.location.line,
        column: unused.location.column,
        safe_to_delete: unused.safe_to_delete,
        files: Vec::new(),
        edge: None,
        actions: vec![
            FindingAction::new(
                "review-member",
                "Review same-library references before removing this member; Dart Decimate has no fix preview yet",
                false,
            )
            .with_target_path(path.clone())
            .with_target_symbol(format!("{}.{}", unused.owner, unused.name))
            .with_dart_decimate_args([
                "inspect",
                "--format",
                "json",
                "--symbol",
                symbol_target.as_str(),
            ])
            .with_suppression_comment(format!("// dart-decimate-ignore-next-line {}", kind_key(kind))),
        ],
    }
}

const fn kind_key(kind: FindingKind) -> &'static str {
    match kind {
        FindingKind::UnusedEnumMember => "unused-enum-member",
        FindingKind::UnusedClassMember => "unused-class-member",
        FindingKind::UnusedType => "unused-type",
        FindingKind::PrivateTypeLeak => "private-type-leak",
        _ => "unused-export",
    }
}
