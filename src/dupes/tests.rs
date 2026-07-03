use std::fs;

use tempfile::TempDir;

use super::*;
use crate::scan_project;

#[test]
fn detects_exact_duplicate_dart_blocks() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    let source = "void shared() {\n  final items = [1, 2, 3];\n  final active = items.where((item) => item > 1);\n  print(active.length);\n}\n";
    write(&fixture, "lib/a.dart", source)?;
    write(&fixture, "lib/b.dart", source)?;
    let project = scan_project(fixture.path())?;

    let report = detect_duplicates(&project, &options(DuplicateMode::Strict, 5, 10))?;

    assert_eq!(report.clone_groups.len(), 1);
    let clone = &report.clone_groups[0];
    assert!(clone.fingerprint.starts_with("dup:"));
    assert_eq!(clone.instances.len(), 2);
    assert_eq!(clone.instances[0].path, fixture.path().join("lib/a.dart"));
    assert_eq!(clone.instances[0].start_line, 1);
    assert_eq!(clone.instances[1].path, fixture.path().join("lib/b.dart"));
    assert_eq!(report.stats.analyzed_lines, 10);
    assert_eq!(report.stats.duplicated_lines, 10);
    assert_eq!(report.stats.duplication_percentage_basis_points, 10000);

    Ok(())
}

#[test]
fn semantic_mode_normalizes_identifiers_and_literals() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/a.dart",
        "int totalActive(List<int> values) {\n  var sum = 0;\n  for (final value in values) {\n    if (value > 10) {\n      print('مرحبا 👋');\n      sum += value;\n    }\n  }\n  return sum;\n}\n",
    )?;
    write(
        &fixture,
        "lib/b.dart",
        "int countReady(List<int> scores) {\n  var acc = 999;\n  for (final score in scores) {\n    if (score > 42) {\n      print('hello 😀');\n      acc += score;\n    }\n  }\n  return acc;\n}\n",
    )?;
    let project = scan_project(fixture.path())?;

    let report = detect_duplicates(&project, &options(DuplicateMode::Semantic, 9, 20))?;

    assert_eq!(report.clone_groups.len(), 1);
    assert_eq!(report.clone_groups[0].instances.len(), 2);
    assert_eq!(report.clone_groups[0].line_count, 9);

    Ok(())
}

#[test]
fn filters_short_blocks_and_generated_files() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/a.dart", "String get label => 'OK';\n")?;
    write(&fixture, "lib/b.dart", "String get title => 'OK';\n")?;
    write(
        &fixture,
        "lib/generated.g.dart",
        "String get title => 'OK';\nString get subtitle => 'OK';\n",
    )?;
    let project = scan_project(fixture.path())?;

    let report = detect_duplicates(&project, &options(DuplicateMode::Semantic, 1, 24))?;

    assert!(report.clone_groups.is_empty());

    Ok(())
}

#[test]
fn detects_sparse_duplicate_blocks_that_meet_default_tokens_as_a_whole(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    let helper = "  static String formatLabel({\n    required String value,\n    required String fallback,\n    required bool useFallback,\n    required String prefix,\n  }) {\n    if (useFallback) {\n      return '$prefix ${fallback.trim()}';\n    }\n\n    final normalized = value.trim().toLowerCase();\n    return '$prefix $normalized';\n  }\n";
    write(
        &fixture,
        "lib/a.dart",
        &format!("class AFormatter {{\n{helper}  int get aOnly => 1;\n}}\n"),
    )?;
    write(
        &fixture,
        "lib/b.dart",
        &format!("class BFormatter {{\n{helper}  int get bOnly => 2;\n}}\n"),
    )?;
    let project = scan_project(fixture.path())?;

    let report = detect_duplicates(&project, &DuplicateOptions::default())?;

    assert_eq!(report.clone_groups.len(), 1);
    let clone = &report.clone_groups[0];
    assert!(clone.line_count > DuplicateOptions::default().min_lines);
    assert!(clone.token_count >= DuplicateOptions::default().min_tokens);
    assert_eq!(clone.instances.len(), 2);
    assert_eq!(clone.instances[0].path, fixture.path().join("lib/a.dart"));
    assert_eq!(clone.instances[0].start_line, 2);
    assert_eq!(clone.instances[1].path, fixture.path().join("lib/b.dart"));
    assert_eq!(clone.instances[1].start_line, 2);

    Ok(())
}

#[test]
fn trace_clone_matches_fingerprint_and_source_line() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    let source = "void shared() {\n  final items = [1, 2, 3];\n  final active = items.where((item) => item > 1);\n  print(active.length);\n}\n";
    write(&fixture, "lib/a.dart", source)?;
    write(&fixture, "lib/b.dart", source)?;
    let project = scan_project(fixture.path())?;
    let report = detect_duplicates(&project, &options(DuplicateMode::Strict, 5, 10))?;
    let fingerprint = report.clone_groups[0].fingerprint.clone();

    let by_fingerprint = trace_clone(&project, &report, &fingerprint);
    let by_line = trace_clone(&project, &report, "lib/a.dart:3");

    assert!(by_fingerprint.found);
    assert_eq!(by_fingerprint.clone_groups[0].fingerprint, fingerprint);
    assert!(by_line.found);
    assert_eq!(by_line.clone_groups[0].instances[0].path, "lib/a.dart");

    Ok(())
}

fn options(mode: DuplicateMode, min_lines: usize, min_tokens: usize) -> DuplicateOptions {
    DuplicateOptions {
        mode,
        min_tokens,
        min_lines,
        min_occurrences: 2,
        skip_local: false,
        ignore_imports: true,
        top: None,
        threshold: None,
    }
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
