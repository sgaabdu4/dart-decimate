use std::collections::BTreeSet;
use std::path::Path;

use serde::ser::{SerializeStruct, Serializer};
use serde::{Deserialize, Serialize};

use super::format::display_path;
use super::scope::{scope_attack_surface, scope_security_blind_spots, scope_security_candidates};
use super::{Finding, FindingAction, FindingKind, Severity};
use crate::{
    AttackSurfaceEntry, SecurityBlindSpot, SecurityBlindSpotReason, SecurityCandidate,
    SecurityCategory, SecurityConfidence, SecurityDefaultSeverity, SecurityReport,
    SecurityTaintConfidence,
};

/// Security candidate serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct JsonSecurityCandidate {
    /// Stable rule id.
    pub rule_id: String,
    /// Stable finding id, equal to the SARIF/Dart Decimate fingerprint.
    pub finding_id: String,
    /// Stable candidate fingerprint.
    pub fingerprint: String,
    /// Candidate category.
    pub category: SecurityCategory,
    /// CWE ids associated with this candidate category.
    pub cwe: Vec<String>,
    /// Default review severity before config rule-level overrides.
    pub severity: Severity,
    /// Agent-actionable source/sink/boundary summary.
    pub candidate: JsonSecurityCandidateDetails,
    /// API surface or sink family.
    pub sink: String,
    /// Detection confidence.
    pub confidence: SecurityConfidence,
    /// Candidate occurrences.
    pub occurrences: Vec<JsonSecurityOccurrence>,
    /// Optional module-level graph reachability context.
    pub reachability: Option<JsonSecurityReachability>,
}

impl Serialize for JsonSecurityCandidate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("JsonSecurityCandidate", 13)?;
        state.serialize_field("rule_id", &self.rule_id)?;
        state.serialize_field("finding_id", &self.finding_id)?;
        state.serialize_field("fingerprint", &self.fingerprint)?;
        state.serialize_field("category", &self.category)?;
        state.serialize_field("cwe", &self.cwe)?;
        state.serialize_field("severity", &self.severity)?;
        state.serialize_field("candidate", &self.candidate)?;
        state.serialize_field("sink", &self.sink)?;
        state.serialize_field("confidence", &self.confidence)?;
        state.serialize_field("occurrences", &self.occurrences)?;
        if let Some(reachability) = &self.reachability {
            state.serialize_field("reachability", reachability)?;
        }
        state.serialize_field(
            "evidence",
            &JsonSecurityEvidence {
                occurrences: &self.occurrences,
            },
        )?;
        state.serialize_field(
            "trace",
            &trace_steps(&self.candidate.trace_role, &self.occurrences),
        )?;
        state.end()
    }
}

/// Agent-actionable candidate context serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonSecurityCandidateDetails {
    /// Evidence source family.
    pub source: String,
    /// Risky sink family.
    pub sink: String,
    /// Trust or platform boundary involved.
    pub boundary: String,
    /// Expected security impact if the candidate is confirmed.
    pub effect: String,
    /// Evidence contract used to create this candidate.
    pub evidence_template: String,
    /// Trace role from the embedded matcher catalogue.
    pub trace_role: String,
}

/// One security candidate occurrence serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonSecurityOccurrence {
    /// Dart file path, root-relative where possible.
    pub path: String,
    /// 1-based line.
    pub line: usize,
    /// 0-based byte column.
    pub column: usize,
    /// Matched expression or API surface.
    pub expression: String,
    /// Redacted source-line evidence.
    pub evidence: String,
    /// Optional module-level graph reachability context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reachability: Option<JsonSecurityReachability>,
}

/// Module-level security reachability context serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonSecurityReachability {
    /// Whether this module is reachable from a configured entry point.
    pub reachable_from_entrypoint: bool,
    /// Confidence tier for the reachability evidence.
    pub taint_confidence: SecurityTaintConfidence,
    /// Root-relative entry points that seeded graph traversal.
    pub entry_points: Vec<String>,
    /// Candidate occurrence count covered by this reachability context.
    pub reachable_occurrences: usize,
}

#[derive(Serialize)]
struct JsonSecurityEvidence<'a> {
    occurrences: &'a [JsonSecurityOccurrence],
}

#[derive(Serialize)]
struct JsonSecurityTraceStep<'a> {
    role: &'a str,
    path: &'a str,
    line: usize,
    column: usize,
    expression: &'a str,
    evidence: &'a str,
}

/// Attack-surface entry serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonAttackSurfaceEntry {
    /// Candidate category exposed on this surface.
    pub category: SecurityCategory,
    /// Dart file path, root-relative where possible.
    pub path: String,
    /// 1-based line.
    pub line: usize,
    /// 0-based byte column.
    pub column: usize,
    /// API surface or boundary.
    pub surface: String,
    /// Verification prompt for downstream agents.
    pub verification_prompt: String,
}

/// Bounded security-analysis blind spot serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonSecurityBlindSpot {
    /// Candidate category whose verification was incomplete.
    pub category: SecurityCategory,
    /// Dart file path, root-relative where possible.
    pub path: String,
    /// 1-based line.
    pub line: usize,
    /// 0-based byte column.
    pub column: usize,
    /// Sink or context family.
    pub sink: String,
    /// Stable bounded omission reason.
    pub reason: SecurityBlindSpotReason,
    /// Redacted source-line evidence.
    pub evidence: String,
}

pub(super) struct JsonSecurityInventories {
    pub(super) candidates: Vec<JsonSecurityCandidate>,
    pub(super) blind_spots: Vec<JsonSecurityBlindSpot>,
    pub(super) attack_surface: Vec<JsonAttackSurfaceEntry>,
}

pub(super) fn json_security_inventories(
    root: &Path,
    report: Option<&SecurityReport>,
    scope: Option<&BTreeSet<String>>,
) -> JsonSecurityInventories {
    JsonSecurityInventories {
        candidates: scope_security_candidates(
            report.map_or_else(Vec::new, |report| json_security_candidates(root, report)),
            scope,
        ),
        blind_spots: scope_security_blind_spots(
            report.map_or_else(Vec::new, |report| json_security_blind_spots(root, report)),
            scope,
        ),
        attack_surface: scope_attack_surface(
            report.map_or_else(Vec::new, |report| json_attack_surface(root, report)),
            scope,
        ),
    }
}

pub(super) fn add_security_findings(
    root: &Path,
    report: &SecurityReport,
    findings: &mut Vec<Finding>,
) {
    findings.extend(
        report
            .candidates
            .iter()
            .map(|candidate| security_finding(root, candidate)),
    );
}

pub(super) fn json_security_candidates(
    root: &Path,
    report: &SecurityReport,
) -> Vec<JsonSecurityCandidate> {
    report
        .candidates
        .iter()
        .map(|candidate| {
            let fingerprint = security_fingerprint(candidate);
            let occurrences = candidate
                .occurrences
                .iter()
                .map(|occurrence| JsonSecurityOccurrence {
                    path: display_path(root, &occurrence.path),
                    line: occurrence.location.line,
                    column: occurrence.location.column,
                    expression: occurrence.expression.clone(),
                    evidence: occurrence.evidence.clone(),
                    reachability: occurrence.reachability.as_ref().map(|reachability| {
                        JsonSecurityReachability {
                            reachable_from_entrypoint: reachability.reachable_from_entrypoint,
                            taint_confidence: reachability.taint_confidence,
                            entry_points: reachability
                                .entry_points
                                .iter()
                                .map(|entry| display_path(root, entry))
                                .collect(),
                            reachable_occurrences: 1,
                        }
                    }),
                })
                .collect::<Vec<_>>();
            JsonSecurityCandidate {
                rule_id: candidate.rule_id.clone(),
                finding_id: fingerprint.clone(),
                fingerprint,
                category: candidate.category,
                cwe: candidate.cwe.clone(),
                severity: security_severity(candidate.default_severity),
                candidate: JsonSecurityCandidateDetails {
                    source: candidate.source.clone(),
                    sink: candidate.sink.clone(),
                    boundary: candidate.boundary.clone(),
                    effect: candidate.effect.clone(),
                    evidence_template: candidate.evidence_template.clone(),
                    trace_role: candidate.trace_role.clone(),
                },
                sink: candidate.sink.clone(),
                confidence: candidate.confidence,
                reachability: candidate_reachability(&occurrences),
                occurrences,
            }
        })
        .collect()
}

pub(super) fn json_security_blind_spots(
    root: &Path,
    report: &SecurityReport,
) -> Vec<JsonSecurityBlindSpot> {
    report
        .blind_spots
        .iter()
        .map(|blind_spot| json_security_blind_spot(root, blind_spot))
        .collect()
}

fn json_security_blind_spot(root: &Path, blind_spot: &SecurityBlindSpot) -> JsonSecurityBlindSpot {
    JsonSecurityBlindSpot {
        category: blind_spot.category,
        path: display_path(root, &blind_spot.path),
        line: blind_spot.location.line,
        column: blind_spot.location.column,
        sink: blind_spot.sink.clone(),
        reason: blind_spot.reason,
        evidence: blind_spot.evidence.clone(),
    }
}

fn candidate_reachability(
    occurrences: &[JsonSecurityOccurrence],
) -> Option<JsonSecurityReachability> {
    let reachable = occurrences
        .iter()
        .filter_map(|occurrence| occurrence.reachability.as_ref())
        .collect::<Vec<_>>();
    let first = reachable.first()?;
    let entry_points = reachable
        .iter()
        .flat_map(|reachability| reachability.entry_points.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    Some(JsonSecurityReachability {
        reachable_from_entrypoint: true,
        taint_confidence: first.taint_confidence,
        entry_points,
        reachable_occurrences: reachable.len(),
    })
}

pub(super) fn json_attack_surface(
    root: &Path,
    report: &SecurityReport,
) -> Vec<JsonAttackSurfaceEntry> {
    report
        .attack_surface
        .iter()
        .map(|entry| attack_surface_entry(root, entry))
        .collect()
}

fn security_finding(root: &Path, candidate: &SecurityCandidate) -> Finding {
    let first = &candidate.occurrences[0];
    let path = display_path(root, &first.path);
    let files = candidate
        .occurrences
        .iter()
        .map(|occurrence| display_path(root, &occurrence.path))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    Finding {
        rule_id: candidate.rule_id.clone(),
        fingerprint: Some(security_fingerprint(candidate)),
        kind: FindingKind::SecurityCandidate,
        severity: security_severity(candidate.default_severity),
        message: format!(
            "Security review candidate for {} via {}",
            category_name(candidate.category),
            candidate.sink
        ),
        path: path.clone(),
        line: first.location.line,
        column: first.location.column,
        safe_to_delete: false,
        files,
        edge: None,
        actions: vec![
            FindingAction::new(
                "review-security-candidate",
                "Verify source control, reachability, and defensive controls before editing",
                false,
            )
            .with_target_path(path.clone())
            .with_target_symbol(candidate.sink.clone())
            .with_dart_decimate_args(["inspect", "--format", "json", "--file", path.as_str()])
            .with_suppression_comment("// dart-decimate-ignore-next-line security-sink"),
        ],
    }
}

fn attack_surface_entry(root: &Path, entry: &AttackSurfaceEntry) -> JsonAttackSurfaceEntry {
    JsonAttackSurfaceEntry {
        category: entry.category,
        path: display_path(root, &entry.path),
        line: entry.location.line,
        column: entry.location.column,
        surface: entry.surface.clone(),
        verification_prompt: entry.verification_prompt.clone(),
    }
}

fn security_fingerprint(candidate: &SecurityCandidate) -> String {
    let text = format!(
        "{}:{}:{}",
        candidate.rule_id,
        category_name(candidate.category),
        candidate.sink
    );
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("sec:{:08x}", hash & 0xffff_ffff)
}

fn trace_steps<'a>(
    role: &'a str,
    occurrences: &'a [JsonSecurityOccurrence],
) -> Vec<JsonSecurityTraceStep<'a>> {
    occurrences
        .iter()
        .map(|occurrence| JsonSecurityTraceStep {
            role,
            path: &occurrence.path,
            line: occurrence.line,
            column: occurrence.column,
            expression: &occurrence.expression,
            evidence: &occurrence.evidence,
        })
        .collect()
}

const fn security_severity(severity: SecurityDefaultSeverity) -> Severity {
    match severity {
        SecurityDefaultSeverity::Warning => Severity::Warning,
        SecurityDefaultSeverity::Error => Severity::Error,
    }
}

const fn category_name(category: SecurityCategory) -> &'static str {
    match category {
        SecurityCategory::HardcodedSecret => "hardcoded secret",
        SecurityCategory::FirebaseApiKey => "Firebase API key",
        SecurityCategory::InsecureTransport => "insecure transport",
        SecurityCategory::TlsBypass => "TLS bypass",
        SecurityCategory::WebViewRisk => "WebView risk",
        SecurityCategory::ProcessExecution => "process execution",
        SecurityCategory::RawSql => "raw SQL",
        SecurityCategory::PlainSecretStorage => "plain secret storage",
        SecurityCategory::WeakRandomness => "weak randomness",
    }
}
