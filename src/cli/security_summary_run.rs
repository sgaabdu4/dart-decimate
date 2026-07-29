use crate::output::{AuditRiskLevel, JsonReport, ReportCommand, Verdict};

use super::{CommandRequest, ReportOutputFormat, audit_run::AuditGate};

pub(super) fn apply_security_summary(request: &CommandRequest, report: &mut JsonReport) {
    if !request.security_summary_mode.is_counts_only()
        || request.command != ReportCommand::Security
        || request.format == ReportOutputFormat::Sarif
    {
        return;
    }

    report.findings.clear();
    report.clone_groups.clear();
    report.complexity.clear();
    report.file_scores.clear();
    report.hotspots.clear();
    report.refactoring_targets.clear();
    report.threshold_overrides.clear();
    report.feature_flags.clear();
    report.security_candidates.clear();
    report.security_blind_spots.clear();
    report.attack_surface.clear();
    report.runtime_coverage = None;
    report.next_steps.clear();
}

pub(super) fn exit_code(request: &CommandRequest, report: &JsonReport, regressed: bool) -> i32 {
    if request.security_gate.is_some() {
        if report.verdict == Verdict::Pass {
            0
        } else {
            8
        }
    } else if request.security_issue_mode.fails_on_issues() {
        i32::from(report.summary.findings > 0)
    } else if request.command == ReportCommand::Audit && request.audit_gate == AuditGate::NewOnly {
        i32::from(report.summary.risk_level == Some(AuditRiskLevel::Fail))
    } else if request.fail_on_regression {
        i32::from(regressed)
    } else {
        i32::from(report.verdict != Verdict::Pass)
    }
}
