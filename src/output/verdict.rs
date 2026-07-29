use super::{Finding, Severity, Verdict};

pub(super) fn report_verdict(findings: &[Finding]) -> Verdict {
    if findings
        .iter()
        .any(|finding| finding.severity == Severity::Error)
    {
        Verdict::Fail
    } else {
        Verdict::Pass
    }
}
