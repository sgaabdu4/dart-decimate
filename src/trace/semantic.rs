use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::graph::normalize_against;
use crate::semantic::{
    SemanticCompleteness, SemanticDecision, SemanticIdentity, SemanticOmissionReason,
    declaration_identity, dependency_path, is_test_owned_path, owning_library_path,
};
use crate::{ScannedProject, TopLevelDeclaration};

use super::{TraceReference, display_path};

/// Completeness of one symbol trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceSemanticCompleteness {
    pub status: SemanticCompleteness,
    pub reasons: Vec<SemanticOmissionReason>,
}

/// One resolved dependency path from a symbol reference to its declaration file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceImpactPath {
    pub from: String,
    pub to: String,
    pub files: Vec<String>,
}

/// Existing test-owned importer suggested for targeted verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceTestSuggestion {
    pub path: String,
    pub import_path: Vec<String>,
    pub reason: String,
}

pub(super) struct SymbolSemanticEvidence {
    pub(super) identity: Option<SemanticIdentity>,
    pub(super) decision: SemanticDecision,
    pub(super) completeness: TraceSemanticCompleteness,
    pub(super) impact_paths: Vec<TraceImpactPath>,
    pub(super) suggested_tests: Vec<TraceTestSuggestion>,
}

pub(super) fn symbol_semantic_evidence(
    project: &ScannedProject,
    path: &Path,
    declarations: &[&TopLevelDeclaration],
    references: &[TraceReference],
) -> SymbolSemanticEvidence {
    let identity = (declarations.len() == 1).then(|| {
        declaration_identity(
            &project.root,
            &owning_library_path(project, path),
            path,
            declarations[0],
        )
    });
    let decision = if identity.is_some() {
        SemanticDecision::Confirmed
    } else if declarations.is_empty() {
        SemanticDecision::RetainedUnresolved
    } else {
        SemanticDecision::RetainedAbstained
    };
    let impact_paths = impact_paths(project, path, references);
    let suggested_tests = suggested_tests(project, &impact_paths);
    SymbolSemanticEvidence {
        identity,
        decision,
        completeness: trace_completeness(path, declarations.len()),
        impact_paths,
        suggested_tests,
    }
}

fn impact_paths(
    project: &ScannedProject,
    target: &Path,
    references: &[TraceReference],
) -> Vec<TraceImpactPath> {
    let mut paths = references
        .iter()
        .filter_map(|reference| {
            let from = normalize_against(&project.root, Path::new(&reference.path));
            dependency_path(project, &from, target).map(|files| TraceImpactPath {
                from: reference.path.clone(),
                to: display_path(&project.root, target),
                files,
            })
        })
        .collect::<Vec<_>>();
    paths.sort_by(|left, right| (&left.from, &left.files).cmp(&(&right.from, &right.files)));
    paths.dedup();
    paths
}

fn suggested_tests(
    project: &ScannedProject,
    impact_paths: &[TraceImpactPath],
) -> Vec<TraceTestSuggestion> {
    impact_paths
        .iter()
        .filter(|path| is_test_owned_path(&project.root, &project.root.join(&path.from)))
        .map(|path| TraceTestSuggestion {
            path: path.from.clone(),
            import_path: path.files.clone(),
            reason: "Existing test-owned importer resolves to this declaration".to_owned(),
        })
        .collect()
}

fn trace_completeness(path: &Path, declaration_count: usize) -> TraceSemanticCompleteness {
    let mut reasons = BTreeSet::from([SemanticOmissionReason::UnsupportedSyntax]);
    if declaration_count > 1 {
        reasons.insert(SemanticOmissionReason::AmbiguousOwner);
    }
    if crate::generated::is_generated_dart_path(path) {
        reasons.insert(SemanticOmissionReason::GeneratedCode);
    }
    TraceSemanticCompleteness {
        status: SemanticCompleteness::Partial,
        reasons: reasons.into_iter().collect(),
    }
}
