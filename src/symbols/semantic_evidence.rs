use std::collections::BTreeSet;

use crate::generated::is_generated_dart_path;
use crate::graph::normalize_against;
use crate::semantic::{
    SEMANTIC_CANDIDATE_CAP, SemanticCompleteness, SemanticDecision, SemanticEvidence,
    SemanticIdentity, SemanticOmissionReason, SemanticReport, SemanticTypeCoupling,
    declaration_identity, member_identity,
};
use crate::{DeclarationKind, ScannedProject};

use super::{
    DuplicateExport, IndexedDeclaration, PrivateTypeLeak, SymbolIndex, UnusedExport, UnusedMember,
};

pub(super) fn semantic_report(
    project: &ScannedProject,
    index: &SymbolIndex,
    unused_exports: &[UnusedExport],
    unused_members: &[UnusedMember],
    private_type_leaks: &[PrivateTypeLeak],
    duplicate_exports: &[DuplicateExport],
) -> SemanticReport {
    if project.files.is_empty() {
        return SemanticReport::default();
    }
    let mut reasons = project_omission_reasons(project);
    let mut evidence = finding_evidence(
        project,
        index,
        unused_exports,
        unused_members,
        private_type_leaks,
        duplicate_exports,
    );
    let mut type_couplings = type_couplings(project, index, &mut reasons);
    evidence.sort_by(|left, right| {
        (&left.subject, left.decision).cmp(&(&right.subject, right.decision))
    });
    evidence.dedup();
    type_couplings.sort_by(|left, right| {
        (
            &left.source,
            left.line,
            left.column,
            &left.referenced_type,
            &left.target,
        )
            .cmp(&(
                &right.source,
                right.line,
                right.column,
                &right.referenced_type,
                &right.target,
            ))
    });
    type_couplings.dedup();
    bounded_report(evidence, type_couplings, reasons)
}

fn finding_evidence(
    project: &ScannedProject,
    index: &SymbolIndex,
    unused_exports: &[UnusedExport],
    unused_members: &[UnusedMember],
    private_type_leaks: &[PrivateTypeLeak],
    duplicate_exports: &[DuplicateExport],
) -> Vec<SemanticEvidence> {
    let mut evidence = Vec::new();
    for unused in unused_exports {
        if let Some(subject) = top_level_identity(
            project,
            index,
            &unused.path,
            unused.kind,
            &unused.name,
            unused.location.line,
        ) {
            evidence.push(abstained(subject));
        }
    }
    for unused in unused_members {
        let path = normalize_against(&project.root, &unused.path);
        let library = index.library_path(&path);
        evidence.push(abstained(member_identity(
            &project.root,
            library,
            &path,
            &unused.owner,
            member_kind(unused.kind),
            &unused.name,
            unused.location,
        )));
    }
    for leak in private_type_leaks {
        if let Some(subject) = top_level_identity(
            project,
            index,
            &leak.path,
            leak.declaration_kind,
            &leak.declaration,
            0,
        ) {
            evidence.push(SemanticEvidence {
                subject,
                decision: SemanticDecision::Confirmed,
                reasons: Vec::new(),
            });
        }
    }
    for duplicate in duplicate_exports {
        for declaration in &duplicate.declarations {
            if let Some(subject) = top_level_identity(
                project,
                index,
                &declaration.path,
                declaration.kind,
                &duplicate.name,
                declaration.location.line,
            ) {
                evidence.push(SemanticEvidence {
                    subject,
                    decision: SemanticDecision::RetainedUnresolved,
                    reasons: vec![SemanticOmissionReason::AmbiguousOwner],
                });
            }
        }
    }
    evidence
}

fn type_couplings(
    project: &ScannedProject,
    index: &SymbolIndex,
    reasons: &mut BTreeSet<SemanticOmissionReason>,
) -> Vec<SemanticTypeCoupling> {
    let mut couplings = Vec::new();
    for file in &project.files {
        let path = normalize_against(&project.root, &file.path);
        let library = index.library_path(&path);
        for reference in &file.signature_references {
            let Some(source) = top_level_identity(
                project,
                index,
                &path,
                reference.declaration_kind,
                &reference.declaration,
                0,
            ) else {
                reasons.insert(SemanticOmissionReason::AmbiguousOwner);
                continue;
            };
            if !index.is_public_export(&path, &reference.declaration) {
                continue;
            }
            let targets = index
                .declarations
                .iter()
                .filter(|candidate| {
                    index.library_path(&candidate.path) == library
                        && candidate.declaration.name == reference.name
                        && is_type_declaration(candidate.declaration.kind)
                })
                .collect::<Vec<_>>();
            let [target] = targets.as_slice() else {
                if targets.len() > 1 {
                    reasons.insert(SemanticOmissionReason::AmbiguousOwner);
                }
                continue;
            };
            couplings.push(SemanticTypeCoupling {
                source,
                referenced_type: reference.name.clone(),
                target: indexed_identity(project, index, target),
                line: reference.location.line,
                column: reference.location.column,
                decision: SemanticDecision::Confirmed,
            });
        }
    }
    couplings
}

fn project_omission_reasons(project: &ScannedProject) -> BTreeSet<SemanticOmissionReason> {
    let mut reasons = BTreeSet::from([SemanticOmissionReason::UnsupportedSyntax]);
    for file in &project.files {
        let path = normalize_against(&project.root, &file.path);
        if is_generated_dart_path(&path) {
            reasons.insert(SemanticOmissionReason::GeneratedCode);
        }
        if file
            .references
            .iter()
            .any(|reference| matches!(reference.name.as_str(), "dynamic" | "noSuchMethod"))
        {
            reasons.insert(SemanticOmissionReason::DynamicAccess);
        }
        if file.references.iter().any(|reference| {
            matches!(
                reference.name.as_str(),
                "registerFactory"
                    | "registerLazySingleton"
                    | "registerSingleton"
                    | "GetIt"
                    | "getIt"
            )
        }) {
            reasons.insert(SemanticOmissionReason::FrameworkRegistration);
        }
    }
    reasons
}

fn top_level_identity(
    project: &ScannedProject,
    index: &SymbolIndex,
    path: &std::path::Path,
    kind: DeclarationKind,
    name: &str,
    line: usize,
) -> Option<SemanticIdentity> {
    let path = normalize_against(&project.root, path);
    let candidates = index
        .declarations
        .iter()
        .filter(|candidate| {
            candidate.path == path
                && candidate.declaration.kind == kind
                && candidate.declaration.name == name
                && (line == 0 || candidate.declaration.location.line == line)
        })
        .collect::<Vec<_>>();
    let [candidate] = candidates.as_slice() else {
        return None;
    };
    Some(indexed_identity(project, index, candidate))
}

fn indexed_identity(
    project: &ScannedProject,
    index: &SymbolIndex,
    indexed: &IndexedDeclaration,
) -> SemanticIdentity {
    declaration_identity(
        &project.root,
        index.library_path(&indexed.path),
        &indexed.path,
        &indexed.declaration,
    )
}

fn abstained(subject: SemanticIdentity) -> SemanticEvidence {
    SemanticEvidence {
        subject,
        decision: SemanticDecision::RetainedAbstained,
        reasons: vec![SemanticOmissionReason::UnsupportedSyntax],
    }
}

fn bounded_report(
    mut evidence: Vec<SemanticEvidence>,
    mut type_couplings: Vec<SemanticTypeCoupling>,
    mut reasons: BTreeSet<SemanticOmissionReason>,
) -> SemanticReport {
    let candidate_count = evidence.len().saturating_add(type_couplings.len());
    evidence.truncate(SEMANTIC_CANDIDATE_CAP);
    let remaining = SEMANTIC_CANDIDATE_CAP.saturating_sub(evidence.len());
    type_couplings.truncate(remaining);
    let processed_candidates = evidence.len().saturating_add(type_couplings.len());
    let omitted_candidates = candidate_count.saturating_sub(processed_candidates);
    if omitted_candidates > 0 {
        reasons.insert(SemanticOmissionReason::Capacity);
    }
    SemanticReport {
        completeness: SemanticCompleteness::Partial,
        reasons: reasons.into_iter().collect(),
        candidate_count,
        processed_candidates,
        omitted_candidates,
        capacity: SEMANTIC_CANDIDATE_CAP,
        evidence,
        type_couplings,
    }
}

const fn is_type_declaration(kind: DeclarationKind) -> bool {
    matches!(
        kind,
        DeclarationKind::Class
            | DeclarationKind::Mixin
            | DeclarationKind::ExtensionType
            | DeclarationKind::Enum
            | DeclarationKind::TypeAlias
    )
}

const fn member_kind(kind: crate::MemberKind) -> &'static str {
    match kind {
        crate::MemberKind::EnumConstant => "enum-constant",
        crate::MemberKind::Field => "field",
        crate::MemberKind::Getter => "getter",
        crate::MemberKind::Setter => "setter",
        crate::MemberKind::Method => "method",
        crate::MemberKind::Constructor => "constructor",
        crate::MemberKind::Operator => "operator",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capacity_retains_original_count_and_marks_omitted_tail() {
        let evidence = (0..=SEMANTIC_CANDIDATE_CAP)
            .map(|index| SemanticEvidence {
                subject: SemanticIdentity {
                    library_uri: "lib/api.dart".to_owned(),
                    path: "lib/api.dart".to_owned(),
                    kind: "class".to_owned(),
                    name: format!("Api{index}"),
                    start_line: index + 1,
                    start_column: 0,
                    end_line: index + 1,
                },
                decision: SemanticDecision::RetainedAbstained,
                reasons: vec![SemanticOmissionReason::UnsupportedSyntax],
            })
            .collect();

        let report = bounded_report(
            evidence,
            Vec::new(),
            BTreeSet::from([SemanticOmissionReason::UnsupportedSyntax]),
        );

        assert_eq!(report.candidate_count, SEMANTIC_CANDIDATE_CAP + 1);
        assert_eq!(report.processed_candidates, SEMANTIC_CANDIDATE_CAP);
        assert_eq!(report.omitted_candidates, 1);
        assert!(report.reasons.contains(&SemanticOmissionReason::Capacity));
    }
}
