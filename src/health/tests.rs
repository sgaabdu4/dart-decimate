use std::fs;

use tempfile::TempDir;

use crate::{
    ComplexityFunctionKind, HealthOptions, HealthThresholdOverride, HealthThresholdOverrideStatus,
    analyze_health, scan_project,
};

#[test]
fn health_counts_branch_constructs() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "lib/main.dart",
        "void main() {
  if (a && b || c) {}
  for (final item in items) {}
  while (ready) {}
  do {} while (again);
  switch (value) {
    case 1:
      break;
    default:
      break;
  }
  try {} catch (error) {}
  final next = a ? b : c;
}
",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_health(
        &project,
        &HealthOptions {
            max_cyclomatic: 1,
            max_cognitive: 99,
            top: None,
            complexity_breakdown: true.into(),
            ..HealthOptions::default()
        },
    )?;

    assert_eq!(report.functions, 1);
    assert_eq!(report.max_cyclomatic_complexity, 10);
    assert_eq!(report.complexity[0].cyclomatic_complexity, 10);
    assert_eq!(report.complexity[0].contributions.len(), 9);

    Ok(())
}

#[test]
fn cognitive_complexity_penalizes_nesting() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "lib/main.dart",
        "void flat() {
  if (a) {}
  if (b) {}
}

void nested() {
  if (a) {
    if (b) {}
  }
}
",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_health(
        &project,
        &HealthOptions {
            max_cyclomatic: 1,
            max_cognitive: 1,
            top: None,
            complexity_breakdown: false.into(),
            ..HealthOptions::default()
        },
    )?;

    let Some(flat) = report
        .complexity
        .iter()
        .find(|finding| finding.symbol == "flat")
    else {
        panic!("flat finding");
    };
    let Some(nested) = report
        .complexity
        .iter()
        .find(|finding| finding.symbol == "nested")
    else {
        panic!("nested finding");
    };
    assert_eq!(flat.cyclomatic_complexity, nested.cyclomatic_complexity);
    assert!(nested.cognitive_complexity > flat.cognitive_complexity);

    Ok(())
}

#[test]
fn nested_closures_are_scored_separately() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "lib/main.dart",
        "void outer() {
  final inner = () {
    if (ready) {}
  };
  inner();
}
",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_health(
        &project,
        &HealthOptions {
            max_cyclomatic: 0,
            max_cognitive: 99,
            top: None,
            complexity_breakdown: false.into(),
            ..HealthOptions::default()
        },
    )?;

    let outer = report
        .complexity
        .iter()
        .find(|finding| finding.symbol == "outer");
    let closure = report
        .complexity
        .iter()
        .find(|finding| finding.symbol == "<closure>");
    let (Some(outer), Some(closure)) = (outer, closure) else {
        panic!("outer function and closure findings");
    };
    assert_eq!(outer.cyclomatic_complexity, 1);
    assert_eq!(closure.cyclomatic_complexity, 2);

    Ok(())
}

#[test]
fn lcov_drives_coverage_gaps_and_crap_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_coverage_source(&fixture)?;
    write(
        &fixture,
        "coverage/lcov.info",
        "SF:lib/main.dart
DA:2,0
DA:3,0
DA:4,0
DA:5,0
end_of_record
",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_health(
        &project,
        &HealthOptions {
            coverage_path: Some("coverage/lcov.info".into()),
            coverage_gaps: true.into(),
            max_crap: Some(10),
            ..HealthOptions::default()
        },
    )?;

    assert_eq!(report.coverage_files, 1);
    assert_eq!(report.coverage_gaps.len(), 1);
    assert_eq!(report.coverage_gaps[0].covered_lines, 0);
    assert_eq!(report.coverage_gaps[0].executable_lines, 4);
    assert_eq!(report.crap.len(), 1);
    assert_eq!(report.crap[0].symbol, "uncovered");
    assert_eq!(report.crap[0].cyclomatic_complexity, 4);
    assert_eq!(report.crap[0].line_coverage_percent, 0);
    assert_eq!(report.crap[0].crap_score, 20);
    assert_eq!(report.max_crap_score, 20);

    Ok(())
}

#[test]
fn covered_lcov_lines_do_not_emit_coverage_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_coverage_source(&fixture)?;
    write(
        &fixture,
        "coverage/lcov.info",
        "SF:lib/main.dart
DA:2,1
DA:3,1
DA:4,1
DA:5,1
end_of_record
",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_health(
        &project,
        &HealthOptions {
            coverage_path: Some("coverage/lcov.info".into()),
            coverage_gaps: true.into(),
            max_crap: Some(10),
            ..HealthOptions::default()
        },
    )?;

    assert!(report.coverage_gaps.is_empty());
    assert!(report.crap.is_empty());
    assert_eq!(report.max_crap_score, 0);

    Ok(())
}

#[test]
fn reports_functions_above_the_default_unit_size() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "lib/main.dart",
        &large_function_source("oversized", 61),
    )?;
    write(
        &fixture,
        "lib/exact.dart",
        &large_function_source("exactLimit", 60),
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_health(&project, &HealthOptions::default())?;

    assert_eq!(report.large_functions.len(), 1);
    assert_eq!(report.large_functions[0].symbol, "oversized");
    assert_eq!(report.large_functions[0].line_count, 61);
    assert_eq!(report.large_functions[0].max_unit_size, 60);
    assert_eq!(
        report.large_functions[0].kind,
        ComplexityFunctionKind::Function
    );
    assert!(report.complexity.is_empty());

    Ok(())
}

#[test]
fn unit_size_override_can_suppress_a_matching_function() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "lib/main.dart",
        &large_function_source("legacyRoute", 61),
    )?;
    let project = scan_project(fixture.path())?;
    let options = HealthOptions {
        threshold_overrides: vec![HealthThresholdOverride {
            files: vec!["lib/main.dart".to_owned()],
            functions: vec!["legacyRoute".to_owned()],
            max_cyclomatic: None,
            max_cognitive: None,
            max_crap: None,
            max_unit_size: Some(120),
            reason: Some("framework adapter".to_owned()),
        }],
        ..HealthOptions::default()
    };

    let report = analyze_health(&project, &options)?;

    assert!(report.large_functions.is_empty());
    assert_eq!(
        report.threshold_overrides[0].status,
        HealthThresholdOverrideStatus::Active
    );
    assert_eq!(
        report.threshold_overrides[0].matched_functions,
        ["lib/main.dart:legacyRoute"]
    );

    Ok(())
}

#[test]
fn classifies_only_flutter_widget_build_methods() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "lib/main.dart",
        &format!(
            "{}\n\n{}\n\n{}\n\n{}\n\n{}",
            large_build_method_source("Dashboard", "StatelessWidget", 61),
            large_build_method_source("Presenter", "Object", 61),
            large_build_method_source("DashboardState", "State<Dashboard>", 61),
            large_build_method_source("StatefulOwner", "StatefulWidget", 61),
            large_build_method_source("ConsumerStatefulOwner", "ConsumerStatefulWidget", 61)
        ),
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_health(&project, &HealthOptions::default())?;

    assert_eq!(report.large_functions.len(), 5);
    let dashboard = &report.large_functions[0];
    assert_eq!(dashboard.kind, ComplexityFunctionKind::FlutterBuildMethod);
    let presenter = &report.large_functions[1];
    assert_eq!(presenter.kind, ComplexityFunctionKind::Method);
    assert_eq!(
        report.large_functions[2].kind,
        ComplexityFunctionKind::FlutterBuildMethod
    );
    assert_eq!(
        report.large_functions[3].kind,
        ComplexityFunctionKind::Method
    );
    assert_eq!(
        report.large_functions[4].kind,
        ComplexityFunctionKind::Method
    );

    Ok(())
}

#[test]
fn classifies_build_methods_through_project_widget_inheritance()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "lib/base.dart",
        "abstract class AppWidget extends StatelessWidget {}\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        &format!(
            "import 'base.dart';\n\n{}",
            large_build_method_source("Dashboard", "AppWidget", 61)
        ),
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_health(&project, &HealthOptions::default())?;

    assert_eq!(report.large_functions.len(), 1);
    assert_eq!(
        report.large_functions[0].kind,
        ComplexityFunctionKind::FlutterBuildMethod
    );

    Ok(())
}

#[test]
fn classifies_animated_widget_build_methods() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "lib/main.dart",
        &large_build_method_source("Spinner", "AnimatedWidget", 61),
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_health(&project, &HealthOptions::default())?;

    assert_eq!(report.large_functions.len(), 1);
    assert_eq!(
        report.large_functions[0].kind,
        ComplexityFunctionKind::FlutterBuildMethod
    );

    Ok(())
}

fn write_coverage_source(fixture: &TempDir) -> Result<(), std::io::Error> {
    write(
        fixture,
        "lib/main.dart",
        "void uncovered(List<int> items) {
  if (items.isEmpty) return;
  for (final item in items) {
    if (item.isEven) return;
  }
}
",
    )
}

fn large_function_source(name: &str, lines: usize) -> String {
    let mut source = vec![format!("void {name}() {{")];
    source.extend(
        (0..lines.saturating_sub(2)).map(|index| format!("  final value{index} = {index};")),
    );
    source.push("}".to_owned());
    source.join("\n")
}

fn large_build_method_source(class_name: &str, base: &str, lines: usize) -> String {
    let mut source = vec![
        format!("class {class_name} extends {base} {{"),
        "  Widget build(BuildContext context) {".to_owned(),
    ];
    source.extend(
        (0..lines.saturating_sub(2)).map(|index| format!("    final value{index} = {index};")),
    );
    source.push("  }".to_owned());
    source.push("}".to_owned());
    source.join("\n")
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
