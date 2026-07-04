use std::fs;

use tempfile::TempDir;

use super::*;
use crate::{
    FeatureFlagOptions, analyze_symbols, detect_feature_flags, find_dead_code, scan::scan_project,
};

#[test]
fn emits_agent_actionable_dead_file_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(&fixture, "lib/dead.dart", "class Dead {}\n")?;
    let project = scan_project(fixture.path())?;
    let dead_code = find_dead_code(&project.graph, ["lib/main.dart"]);

    let report = build_json_report(
        &project,
        &AnalysisResults {
            command: ReportCommand::DeadCode,
            dead_code: Some(dead_code),
            symbols: None,
            cycles: Vec::new(),
            re_export_cycles: Vec::new(),
            boundary_violations: Vec::new(),
            boundary_coverage: Vec::new(),
            boundary_call_violations: Vec::new(),
            policy_violations: Vec::new(),
            dependency_hygiene: None,
            duplicates: None,
            health: None,
            feature_flags: None,
            security: None,
            routes: None,
            widgets: None,
            file_scope: None,
            require_suppression_reasons: false,
        },
    );

    assert_eq!(report.schema_version, SCHEMA_VERSION);
    assert_eq!(report.verdict, Verdict::Fail);
    assert_eq!(report.summary.dead_files, 1);
    assert_eq!(report.findings[0].rule_id, "dart-decimate/dead-file");
    assert_eq!(report.findings[0].path, "lib/dead.dart");
    assert!(report.findings[0].safe_to_delete);
    assert!(report.findings[0].actions[0].auto_fixable);

    Ok(())
}

#[test]
fn emits_agent_actionable_duplicate_export_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "export 'src/a.dart';\nexport 'src/b.dart';\n",
    )?;
    write(&fixture, "lib/src/a.dart", "class Api {}\n")?;
    write(&fixture, "lib/src/b.dart", "class Api {}\n")?;
    let project = scan_project(fixture.path())?;
    let symbols = analyze_symbols(&project, None);

    let report = build_json_report(
        &project,
        &AnalysisResults {
            command: ReportCommand::Check,
            dead_code: None,
            symbols: Some(symbols),
            cycles: Vec::new(),
            re_export_cycles: Vec::new(),
            boundary_violations: Vec::new(),
            boundary_coverage: Vec::new(),
            boundary_call_violations: Vec::new(),
            policy_violations: Vec::new(),
            dependency_hygiene: None,
            duplicates: None,
            health: None,
            feature_flags: None,
            security: None,
            routes: None,
            widgets: None,
            file_scope: None,
            require_suppression_reasons: false,
        },
    );

    assert_eq!(report.verdict, Verdict::Fail);
    assert_eq!(report.summary.duplicate_exports, 1);
    assert_eq!(report.findings[0].rule_id, "dart-decimate/duplicate-export");
    assert_eq!(report.findings[0].kind, FindingKind::DuplicateExport);
    assert_eq!(report.findings[0].path, "lib/package.dart");
    assert_eq!(
        report.findings[0].files,
        vec!["lib/src/a.dart", "lib/src/b.dart"]
    );
    assert!(!report.findings[0].safe_to_delete);
    assert_eq!(
        report.findings[0].actions[0].action,
        "inspect-export-surface"
    );

    Ok(())
}

#[test]
fn compile_time_feature_flag_severity_depends_on_path_not_name()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "const variant = bool.fromEnvironment('AB_TEST_VARIANT');\n",
    )?;
    write(
        &fixture,
        "lib/dev_flags.dart",
        "const debugFlag = bool.fromEnvironment('DEBUG_PROD_ONLY');\n",
    )?;
    write(
        &fixture,
        "lib/debug_flags.dart",
        "const e2eFlag = bool.fromEnvironment('E2E_PROD_ONLY');\n",
    )?;
    write(
        &fixture,
        "test/flag_test.dart",
        "const debug = bool.fromEnvironment('DEBUG_TEST_ONLY');\n",
    )?;
    let project = scan_project(fixture.path())?;
    let feature_flags = detect_feature_flags(&project, &FeatureFlagOptions::default())?;

    let report = build_json_report(
        &project,
        &AnalysisResults {
            command: ReportCommand::Check,
            dead_code: None,
            symbols: None,
            cycles: Vec::new(),
            re_export_cycles: Vec::new(),
            boundary_violations: Vec::new(),
            boundary_coverage: Vec::new(),
            boundary_call_violations: Vec::new(),
            policy_violations: Vec::new(),
            dependency_hygiene: None,
            duplicates: None,
            health: None,
            feature_flags: Some(feature_flags),
            security: None,
            routes: None,
            widgets: None,
            file_scope: None,
            require_suppression_reasons: false,
        },
    );

    assert_eq!(
        report
            .findings
            .iter()
            .find(|finding| finding.path == "lib/main.dart")
            .map(|finding| finding.severity),
        Some(Severity::Error)
    );
    assert_eq!(
        report
            .findings
            .iter()
            .find(|finding| finding.path == "lib/dev_flags.dart")
            .map(|finding| finding.severity),
        Some(Severity::Error)
    );
    assert_eq!(
        report
            .findings
            .iter()
            .find(|finding| finding.path == "lib/debug_flags.dart")
            .map(|finding| finding.severity),
        Some(Severity::Error)
    );
    assert_eq!(
        report
            .findings
            .iter()
            .find(|finding| finding.path == "test/flag_test.dart")
            .map(|finding| finding.severity),
        Some(Severity::Warning)
    );

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
