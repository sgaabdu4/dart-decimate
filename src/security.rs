use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::graph::normalize_against;
use crate::{
    DartCombinatorKind, DartImport, DeadCodeReport, DependencyKind, Location, ScannedProject,
};

mod catalogue;
mod detect;
use catalogue::MatcherCatalogue;
pub use catalogue::SecurityDefaultSeverity;
use detect::{detect_in_source, is_ignored_path};

/// Security candidate detector options.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityOptions {
    /// Limit output to the N most frequently reported candidate groups.
    pub top: Option<usize>,
    /// Include attack-surface inventory entries.
    pub surface: bool,
    /// Enabled candidate categories. Empty means all categories.
    pub categories: BTreeSet<SecurityCategory>,
}

/// Security candidate report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityReport {
    /// Options used to compute this report.
    pub options: SecurityOptions,
    /// Dart files included in security detection.
    pub analyzed_files: usize,
    /// Grouped unverified security candidates.
    pub candidates: Vec<SecurityCandidate>,
    /// Raw security candidate occurrence count before `--top` truncation.
    pub total_occurrences: usize,
    /// Bounded cases where a sink-shaped expression could not be verified.
    pub blind_spots: Vec<SecurityBlindSpot>,
    /// Optional attack-surface inventory.
    pub attack_surface: Vec<AttackSurfaceEntry>,
}

/// One grouped unverified security candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityCandidate {
    /// Stable rule id.
    pub rule_id: String,
    /// Candidate category.
    pub category: SecurityCategory,
    /// API surface or sink family.
    pub sink: String,
    /// Detection confidence.
    pub confidence: SecurityConfidence,
    /// CWE ids owned by the embedded matcher catalogue.
    pub cwe: Vec<String>,
    /// Risk effect owned by the embedded matcher catalogue.
    pub effect: String,
    /// Evidence expectation owned by the embedded matcher catalogue.
    pub evidence_template: String,
    /// Evidence source family.
    pub source: String,
    /// Trust or platform boundary involved.
    pub boundary: String,
    /// Trace role for serialized evidence.
    pub trace_role: String,
    /// Default severity before rule overrides.
    pub default_severity: SecurityDefaultSeverity,
    /// Candidate occurrences.
    pub occurrences: Vec<SecurityOccurrence>,
}

/// Security candidate category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecurityCategory {
    /// Secret-shaped literal or secret-named assignment.
    HardcodedSecret,
    /// Firebase client API key in `FirebaseOptions`.
    FirebaseApiKey,
    /// Remote cleartext HTTP transport.
    InsecureTransport,
    /// TLS validation bypass.
    TlsBypass,
    /// `WebView` JavaScript or file access exposure.
    WebViewRisk,
    /// Process execution with shell or dynamic command material.
    ProcessExecution,
    /// Raw SQL with interpolation or dynamic query text.
    RawSql,
    /// Secret-like material written to plain local storage.
    PlainSecretStorage,
    /// Predictable `dart:math Random()` output used as security material.
    WeakRandomness,
}

impl SecurityCategory {
    const ALL: [Self; 9] = [
        Self::HardcodedSecret,
        Self::FirebaseApiKey,
        Self::InsecureTransport,
        Self::TlsBypass,
        Self::WebViewRisk,
        Self::ProcessExecution,
        Self::RawSql,
        Self::PlainSecretStorage,
        Self::WeakRandomness,
    ];
}

impl SecurityOptions {
    fn includes_category(&self, category: SecurityCategory) -> bool {
        self.categories.is_empty() || self.categories.contains(&category)
    }
}

/// Detection confidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecurityConfidence {
    /// Low-confidence heuristic.
    Low,
    /// Medium-confidence heuristic.
    Medium,
    /// High-confidence known risky surface.
    High,
}

/// One security candidate occurrence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityOccurrence {
    /// Dart file path.
    pub path: PathBuf,
    /// Location of the candidate.
    pub location: Location,
    /// Matched expression or API surface.
    pub expression: String,
    /// Redacted source-line evidence.
    pub evidence: String,
    /// Optional module-level graph reachability context.
    pub reachability: Option<SecurityReachability>,
}

/// Why a sink-shaped expression could not be verified by the bounded analyzer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecurityBlindSpotReason {
    /// A value was assigned through an intermediate identifier.
    UnflattenedRandomFlow,
    /// A Random-shaped constructor could not be attributed to `dart:math`.
    AmbiguousRandomProvenance,
    /// A sink-shaped call could not be flattened into a complete argument list.
    UnflattenedCall,
    /// A sink-shaped call could not be attributed to its declaring library.
    AmbiguousCallProvenance,
    /// A TLS validation callback assignment could not be resolved locally.
    AmbiguousTlsCallback,
}

/// One bounded security-analysis blind spot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityBlindSpot {
    /// Candidate category whose verification was incomplete.
    pub category: SecurityCategory,
    /// Dart file path.
    pub path: PathBuf,
    /// Location of the ambiguous expression.
    pub location: Location,
    /// Sink or context family.
    pub sink: String,
    /// Stable bounded omission reason.
    pub reason: SecurityBlindSpotReason,
    /// Redacted source-line evidence.
    pub evidence: String,
}

/// Module-level security reachability context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityReachability {
    /// Whether this occurrence's module is reachable from a configured entry point.
    pub reachable_from_entrypoint: bool,
    /// Confidence tier for the reachability evidence.
    pub taint_confidence: SecurityTaintConfidence,
    /// Entry points that seeded the module graph traversal.
    pub entry_points: Vec<PathBuf>,
}

/// Security taint confidence tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecurityTaintConfidence {
    /// Module is import-reachable from an entry point; value flow is not proven.
    ModuleLevel,
}

/// Attack-surface inventory entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttackSurfaceEntry {
    /// Candidate category exposed on this surface.
    pub category: SecurityCategory,
    /// Dart file path.
    pub path: PathBuf,
    /// Location of the surface.
    pub location: Location,
    /// API surface or boundary.
    pub surface: String,
    /// Verification prompt for downstream agents.
    pub verification_prompt: String,
}

/// Errors returned while detecting security candidates.
#[derive(Debug, Error)]
pub enum SecurityError {
    /// A Dart file could not be read.
    #[error("failed to read Dart file {path}: {source}")]
    ReadFile {
        /// File path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// The embedded matcher catalogue was not valid TOML.
    #[error("invalid embedded security matcher catalogue: {0}")]
    MatcherCatalogue(toml::de::Error),
    /// The embedded matcher catalogue violated its internal contract.
    #[error("invalid embedded security matcher catalogue: {0}")]
    InvalidMatcherCatalogue(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CandidateGroup {
    rule_id: String,
    category: SecurityCategory,
    sink: String,
    confidence: SecurityConfidence,
    cwe: Vec<String>,
    effect: String,
    evidence_template: String,
    source: String,
    boundary: String,
    trace_role: String,
    default_severity: SecurityDefaultSeverity,
    occurrences: Vec<SecurityOccurrence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DetectedSecurityCandidate {
    category: SecurityCategory,
    sink: String,
    confidence: SecurityConfidence,
    occurrence: SecurityOccurrence,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct DetectionResult {
    candidates: Vec<DetectedSecurityCandidate>,
    blind_spots: Vec<SecurityBlindSpot>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct LibraryImportContext {
    visible_types: BTreeSet<(String, String, Option<String>)>,
    declared_names: BTreeSet<String>,
    import_prefixes: BTreeSet<String>,
}

impl LibraryImportContext {
    fn exposes(&self, uri: &str, type_name: &str, prefix: Option<&str>) -> bool {
        self.visible_types.contains(&(
            uri.to_owned(),
            type_name.to_owned(),
            prefix.map(str::to_owned),
        ))
    }
}

/// Detect unverified local security review candidates in Dart and Flutter code.
///
/// # Errors
///
/// Returns [`SecurityError`] if a scanned Dart file cannot be read.
pub fn analyze_security(
    project: &ScannedProject,
    options: &SecurityOptions,
    dead_code: Option<&DeadCodeReport>,
) -> Result<SecurityReport, SecurityError> {
    let catalogue = MatcherCatalogue::parse()?;
    let mut groups = BTreeMap::<(SecurityCategory, String), CandidateGroup>::new();
    let mut blind_spots = Vec::new();
    let reachability = dead_code.map(SecurityReachabilityContext::from);
    let library_imports = library_import_contexts(project);

    let mut detected_files = project
        .files
        .par_iter()
        .filter_map(|file| {
            let path = normalize_against(&project.root, &file.path);
            if !path.starts_with(&project.root) || is_ignored_path(&path) {
                return None;
            }
            let inherited_imports = library_imports.get(&path);
            let detected = fs::read_to_string(&path)
                .map(|source| detect_in_source(&path, &source, &catalogue, inherited_imports))
                .map_err(|source| SecurityError::ReadFile {
                    path: path.clone(),
                    source,
                });
            Some((path, detected))
        })
        .collect::<Vec<_>>();
    detected_files.sort_by(|left, right| left.0.cmp(&right.0));
    let analyzed_files = detected_files.len();

    for (_, detected) in detected_files {
        let detected = detected?;
        blind_spots.extend(
            detected
                .blind_spots
                .into_iter()
                .filter(|blind_spot| options.includes_category(blind_spot.category)),
        );
        for detected in detected
            .candidates
            .into_iter()
            .filter(|candidate| options.includes_category(candidate.category))
        {
            let mut detected = detected;
            detected.occurrence.reachability = reachability
                .as_ref()
                .and_then(|context| context.reachability_for(&detected.occurrence.path));
            let key = (detected.category, detected.sink.clone());
            let matcher = catalogue.matcher(detected.category);
            let group = groups.entry(key).or_insert_with(|| CandidateGroup {
                rule_id: matcher.rule_id.clone(),
                category: detected.category,
                sink: detected.sink.clone(),
                confidence: detected.confidence,
                cwe: matcher.cwe.clone(),
                effect: matcher.effect.clone(),
                evidence_template: matcher.evidence_template.clone(),
                source: matcher.source.clone(),
                boundary: matcher.boundary.clone(),
                trace_role: matcher.trace_role.clone(),
                default_severity: matcher.default_severity,
                occurrences: Vec::new(),
            });
            group.confidence = group.confidence.max(detected.confidence);
            group.occurrences.push(detected.occurrence);
        }
    }

    let total_occurrences = groups
        .values()
        .map(|group| group.occurrences.len())
        .sum::<usize>();
    let mut candidates = groups
        .into_values()
        .map(SecurityCandidate::from)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        (
            std::cmp::Reverse(left.occurrences.len()),
            left.category,
            &left.sink,
        )
            .cmp(&(
                std::cmp::Reverse(right.occurrences.len()),
                right.category,
                &right.sink,
            ))
    });
    if let Some(top) = options.top {
        candidates.truncate(top);
    }
    deduplicate_blind_spots(&mut blind_spots);
    let attack_surface = if options.surface {
        attack_surface_for(&candidates, &catalogue)
    } else {
        Vec::new()
    };

    Ok(SecurityReport {
        options: options.clone(),
        analyzed_files,
        candidates,
        total_occurrences,
        blind_spots,
        attack_surface,
    })
}

fn library_import_contexts(project: &ScannedProject) -> BTreeMap<PathBuf, LibraryImportContext> {
    let files = project
        .files
        .iter()
        .map(|file| (normalize_against(&project.root, &file.path), file))
        .collect::<BTreeMap<_, _>>();
    let mut parts_by_library = BTreeMap::<PathBuf, Vec<PathBuf>>::new();
    let mut libraries_by_part = BTreeMap::<PathBuf, Vec<PathBuf>>::new();
    for dependency in project
        .graph
        .dependencies()
        .into_iter()
        .filter(|dependency| dependency.kind == DependencyKind::Part)
    {
        parts_by_library
            .entry(dependency.from_path.clone())
            .or_default()
            .push(dependency.to_path.clone());
        libraries_by_part
            .entry(dependency.to_path)
            .or_default()
            .push(dependency.from_path);
    }

    let mut contexts = BTreeMap::new();
    for (library, mut parts) in parts_by_library {
        parts.sort();
        parts.dedup();
        let Some(owner) = files.get(&library) else {
            continue;
        };
        if owner.parts.len() != parts.len()
            || parts.iter().any(|part| {
                let mut libraries = libraries_by_part.get(part).cloned().unwrap_or_default();
                libraries.sort();
                libraries.dedup();
                libraries.as_slice() != [library.clone()]
            })
        {
            continue;
        }
        let mut context = LibraryImportContext::default();
        for import in owner
            .imports
            .iter()
            .filter(|import| import.condition.is_none())
        {
            record_security_import(&mut context, import);
        }
        for path in std::iter::once(&library).chain(parts.iter()) {
            let Some(file) = files.get(path) else {
                continue;
            };
            context.declared_names.extend(
                file.declarations
                    .iter()
                    .map(|declaration| declaration.name.clone()),
            );
        }
        contexts.insert(library, context.clone());
        for part in parts {
            contexts.insert(part, context.clone());
        }
    }
    contexts
}

fn record_security_import(context: &mut LibraryImportContext, import: &DartImport) {
    if let Some(prefix) = &import.prefix {
        context.import_prefixes.insert(prefix.clone());
    }
    let type_names: &[&str] = match import.uri.as_str() {
        "dart:io" => &["Platform", "Process"],
        "package:process/process.dart" => &["LocalProcessManager", "ProcessManager"],
        _ => return,
    };
    for type_name in type_names {
        if extracted_import_exposes_name(import, type_name) {
            context.visible_types.insert((
                import.uri.clone(),
                (*type_name).to_owned(),
                import.prefix.clone(),
            ));
        }
    }
}

fn extracted_import_exposes_name(import: &DartImport, name: &str) -> bool {
    let mut visible = true;
    for combinator in &import.combinators {
        let contains_name = combinator.names.iter().any(|candidate| candidate == name);
        match combinator.kind {
            DartCombinatorKind::Show => visible = contains_name,
            DartCombinatorKind::Hide if contains_name => visible = false,
            DartCombinatorKind::Hide => {}
        }
    }
    visible
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SecurityReachabilityContext {
    reachable_files: BTreeSet<PathBuf>,
    entry_points: Vec<PathBuf>,
}

impl SecurityReachabilityContext {
    fn reachability_for(&self, path: &PathBuf) -> Option<SecurityReachability> {
        self.reachable_files
            .contains(path)
            .then(|| SecurityReachability {
                reachable_from_entrypoint: true,
                taint_confidence: SecurityTaintConfidence::ModuleLevel,
                entry_points: self.entry_points.clone(),
            })
    }
}

impl From<&DeadCodeReport> for SecurityReachabilityContext {
    fn from(report: &DeadCodeReport) -> Self {
        Self {
            reachable_files: report.reachable_files.iter().cloned().collect(),
            entry_points: report.entry_points.clone(),
        }
    }
}

impl From<CandidateGroup> for SecurityCandidate {
    fn from(group: CandidateGroup) -> Self {
        let mut seen = BTreeSet::new();
        let mut occurrences = group
            .occurrences
            .into_iter()
            .filter(|occurrence| {
                seen.insert((
                    occurrence.path.clone(),
                    occurrence.location.line,
                    occurrence.location.column,
                    occurrence.expression.clone(),
                ))
            })
            .collect::<Vec<_>>();
        occurrences.sort_by(|left, right| {
            (&left.path, left.location.line, left.location.column).cmp(&(
                &right.path,
                right.location.line,
                right.location.column,
            ))
        });
        Self {
            rule_id: group.rule_id,
            category: group.category,
            sink: group.sink,
            confidence: group.confidence,
            cwe: group.cwe,
            effect: group.effect,
            evidence_template: group.evidence_template,
            source: group.source,
            boundary: group.boundary,
            trace_role: group.trace_role,
            default_severity: group.default_severity,
            occurrences,
        }
    }
}

fn attack_surface_for(
    candidates: &[SecurityCandidate],
    catalogue: &MatcherCatalogue,
) -> Vec<AttackSurfaceEntry> {
    candidates
        .iter()
        .flat_map(|candidate| {
            candidate
                .occurrences
                .iter()
                .map(|occurrence| AttackSurfaceEntry {
                    category: candidate.category,
                    path: occurrence.path.clone(),
                    location: occurrence.location,
                    surface: candidate.sink.clone(),
                    verification_prompt: catalogue
                        .matcher(candidate.category)
                        .verification_prompt
                        .clone(),
                })
        })
        .collect()
}

fn deduplicate_blind_spots(blind_spots: &mut Vec<SecurityBlindSpot>) {
    blind_spots.sort_by(|left, right| {
        (
            &left.path,
            left.location.line,
            left.location.column,
            left.category,
            left.reason,
            &left.sink,
        )
            .cmp(&(
                &right.path,
                right.location.line,
                right.location.column,
                right.category,
                right.reason,
                &right.sink,
            ))
    });
    blind_spots.dedup_by(|left, right| {
        left.path == right.path
            && left.location == right.location
            && left.category == right.category
            && left.reason == right.reason
            && left.sink == right.sink
    });
}

#[cfg(test)]
mod tests;
