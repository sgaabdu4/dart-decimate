use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::graph::normalize_against;
use crate::{DeclarationKind, Location, ScannedProject, TopLevelDeclaration};

pub const SEMANTIC_CANDIDATE_CAP: usize = 16_384;

/// Stable identity for one syntactically owned Dart declaration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SemanticIdentity {
    /// Root-relative URI of the owning Dart library.
    pub library_uri: String,
    /// Root-relative declaration file.
    pub path: String,
    /// Stable declaration kind.
    pub kind: String,
    /// Declared name.
    pub name: String,
    /// 1-based source start line.
    pub start_line: usize,
    /// 0-based source start column.
    pub start_column: usize,
    /// Inclusive 1-based source end line.
    pub end_line: usize,
}

/// Conservative semantic reconciliation decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticDecision {
    Confirmed,
    RetainedUnresolved,
    RetainedAbstained,
}

impl SemanticDecision {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Confirmed => "confirmed",
            Self::RetainedUnresolved => "retained-unresolved",
            Self::RetainedAbstained => "retained-abstained",
        }
    }
}

/// Completeness of Rust-native semantic evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticCompleteness {
    Complete,
    Partial,
    Unavailable,
}

impl SemanticCompleteness {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Stable reason why semantic evidence is incomplete or abstained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SemanticOmissionReason {
    DynamicAccess,
    GeneratedCode,
    FrameworkRegistration,
    AmbiguousOwner,
    ParseFailure,
    Capacity,
    UnsupportedSyntax,
}

/// Reconciliation evidence for one retained symbol candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticEvidence {
    pub subject: SemanticIdentity,
    pub decision: SemanticDecision,
    pub reasons: Vec<SemanticOmissionReason>,
}

/// Advisory resolved type edge from one public signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticTypeCoupling {
    pub source: SemanticIdentity,
    pub referenced_type: String,
    pub target: SemanticIdentity,
    pub line: usize,
    pub column: usize,
    pub decision: SemanticDecision,
}

/// Project-level semantic evidence and bounded completeness accounting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticReport {
    pub completeness: SemanticCompleteness,
    pub reasons: Vec<SemanticOmissionReason>,
    pub candidate_count: usize,
    pub processed_candidates: usize,
    pub omitted_candidates: usize,
    pub capacity: usize,
    pub evidence: Vec<SemanticEvidence>,
    pub type_couplings: Vec<SemanticTypeCoupling>,
}

impl Default for SemanticReport {
    fn default() -> Self {
        Self {
            completeness: SemanticCompleteness::Unavailable,
            reasons: vec![SemanticOmissionReason::UnsupportedSyntax],
            candidate_count: 0,
            processed_candidates: 0,
            omitted_candidates: 0,
            capacity: SEMANTIC_CANDIDATE_CAP,
            evidence: Vec::new(),
            type_couplings: Vec::new(),
        }
    }
}

pub(crate) fn declaration_identity(
    root: &Path,
    library_path: &Path,
    path: &Path,
    declaration: &TopLevelDeclaration,
) -> SemanticIdentity {
    SemanticIdentity {
        library_uri: display_path(root, library_path),
        path: display_path(root, path),
        kind: declaration_kind(declaration.kind).to_owned(),
        name: declaration.name.clone(),
        start_line: declaration.location.line,
        start_column: declaration.location.column,
        end_line: declaration.range.end_line,
    }
}

pub(crate) fn member_identity(
    root: &Path,
    library_path: &Path,
    path: &Path,
    owner: &str,
    kind: &str,
    name: &str,
    location: Location,
) -> SemanticIdentity {
    SemanticIdentity {
        library_uri: display_path(root, library_path),
        path: display_path(root, path),
        kind: format!("{kind}-member"),
        name: format!("{owner}.{name}"),
        start_line: location.line,
        start_column: location.column,
        end_line: location.line,
    }
}

pub(crate) fn dependency_path(
    project: &ScannedProject,
    from: &Path,
    to: &Path,
) -> Option<Vec<String>> {
    let from = normalize_against(&project.root, from);
    let to = normalize_against(&project.root, to);
    if from == to {
        return Some(vec![display_path(&project.root, &from)]);
    }
    let mut adjacency = BTreeMap::<PathBuf, BTreeSet<PathBuf>>::new();
    for dependency in project.graph.dependencies() {
        adjacency
            .entry(dependency.from_path)
            .or_default()
            .insert(dependency.to_path);
    }
    let mut queue = VecDeque::from([from.clone()]);
    let mut previous = BTreeMap::<PathBuf, PathBuf>::new();
    let mut seen = BTreeSet::from([from.clone()]);
    while let Some(path) = queue.pop_front() {
        let Some(targets) = adjacency.get(&path) else {
            continue;
        };
        for target in targets {
            if !seen.insert(target.clone()) {
                continue;
            }
            previous.insert(target.clone(), path.clone());
            if target == &to {
                return Some(reconstruct_path(&project.root, &from, &to, &previous));
            }
            queue.push_back(target.clone());
        }
    }
    None
}

pub(crate) fn owning_library_path(project: &ScannedProject, path: &Path) -> PathBuf {
    let path = normalize_against(&project.root, path);
    let part_edges = project
        .graph
        .dependencies()
        .into_iter()
        .filter(|dependency| dependency.kind == crate::DependencyKind::Part)
        .collect::<Vec<_>>();
    let mut owner = path.clone();
    let mut seen = BTreeSet::from([path]);
    while let Some(parent) = part_edges
        .iter()
        .filter(|dependency| dependency.to_path == owner)
        .map(|dependency| dependency.from_path.clone())
        .min()
    {
        if !seen.insert(parent.clone()) {
            break;
        }
        owner = parent;
    }
    owner
}

pub(crate) fn is_test_owned_path(root: &Path, path: &Path) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some("test" | "integration_test")
        )
    })
}

pub(crate) fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn reconstruct_path(
    root: &Path,
    from: &Path,
    to: &Path,
    previous: &BTreeMap<PathBuf, PathBuf>,
) -> Vec<String> {
    let mut path = vec![to.to_path_buf()];
    let mut current = to;
    while current != from {
        let Some(parent) = previous.get(current) else {
            break;
        };
        path.push(parent.clone());
        current = parent;
    }
    path.reverse();
    path.iter().map(|path| display_path(root, path)).collect()
}

const fn declaration_kind(kind: DeclarationKind) -> &'static str {
    match kind {
        DeclarationKind::Class => "class",
        DeclarationKind::Mixin => "mixin",
        DeclarationKind::Extension => "extension",
        DeclarationKind::ExtensionType => "extension-type",
        DeclarationKind::Enum => "enum",
        DeclarationKind::TypeAlias => "type-alias",
        DeclarationKind::Variable => "variable",
        DeclarationKind::Function => "function",
    }
}
