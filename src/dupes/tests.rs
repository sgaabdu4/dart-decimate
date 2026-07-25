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
fn detects_sparse_duplicate_blocks_that_meet_default_tokens_as_a_whole()
-> Result<(), Box<dyn std::error::Error>> {
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
fn copied_package_filter_ignores_unrelated_malformed_pubspec()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "fixtures/bad/pubspec.yaml", "name: [\n")?;
    for package in ["copy_a", "copy_b"] {
        write(
            &fixture,
            &format!("{package}/pubspec.yaml"),
            "name: copied_package\n",
        )?;
        write(
            &fixture,
            &format!("{package}/lib/shared.dart"),
            "String sharedHttp() {\n  final headers = <String, String>{\n    'accept': 'application/json',\n    'content-type': 'application/json',\n    'x-client': 'function-client',\n  };\n  final values = ['alpha', 'beta', 'gamma', headers.keys.join('|')];\n  final normalized = values.map((value) => value.trim().toLowerCase()).where((value) => value.isNotEmpty).toList();\n  return normalized.join(',');\n}\n",
        )?;
    }
    let project = scan_project(fixture.path())?;

    let report = detect_duplicates(&project, &DuplicateOptions::default())?;

    assert!(report.clone_groups.is_empty());

    Ok(())
}

#[test]
fn top_uses_canonicalized_clone_ranking() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    let mirrored = pagination_clone_source();
    write(&fixture, "functions/shared/pubspec.yaml", "name: shared\n")?;
    write(&fixture, "functions/shared/lib/pagination.dart", mirrored)?;
    for function in ["delete_user", "squad_operations"] {
        write(
            &fixture,
            &format!("functions/{function}/shared/pubspec.yaml"),
            "name: shared\n",
        )?;
        write(
            &fixture,
            &format!("functions/{function}/shared/lib/pagination.dart"),
            mirrored,
        )?;
    }
    let real = "String visibleClone(String owner) {\n  final normalized = owner.trim().toLowerCase();\n  final words = normalized.split(' ');\n  final compact = words.where((word) => word.isNotEmpty).join('-');\n  return 'owner:$compact';\n}\n";
    for path in ["lib/alpha.dart", "lib/beta.dart", "lib/gamma.dart"] {
        write(&fixture, path, real)?;
    }
    let project = scan_project(fixture.path())?;
    let mut opts = options(DuplicateMode::Strict, 5, 10);
    opts.top = Some(1);

    let report = detect_duplicates(&project, &opts)?;

    assert_eq!(report.clone_groups.len(), 1);
    let clone = &report.clone_groups[0];
    assert_eq!(clone.instances.len(), 3);
    assert!(
        clone
            .instances
            .iter()
            .all(|instance| { instance.path.starts_with(fixture.path().join("lib")) })
    );

    Ok(())
}

#[test]
fn copied_package_canonicalization_keeps_unmatched_occurrences()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "functions/shared/pubspec.yaml", "name: shared\n")?;
    write(
        &fixture,
        "functions/delete_user/shared/pubspec.yaml",
        "name: shared\n",
    )?;
    let clone = "String copiedFormatter(String value) {\n  final normalized = value.trim().toLowerCase();\n  final compact = normalized.split(' ').where((word) => word.isNotEmpty).join('-');\n  final suffix = compact.isEmpty ? 'empty' : compact;\n  return 'owner:$suffix';\n}\n";
    write(
        &fixture,
        "functions/shared/lib/pagination.dart",
        &format!(
            "// header 1\n// header 2\n// header 3\n// header 4\n// header 5\n// header 6\n// header 7\n{clone}"
        ),
    )?;
    write(
        &fixture,
        "functions/delete_user/shared/lib/pagination.dart",
        &format!("{clone}\n{clone}"),
    )?;
    let project = scan_project(fixture.path())?;

    let report = detect_duplicates(&project, &options(DuplicateMode::Strict, 6, 20))?;

    assert_eq!(report.clone_groups.len(), 1);
    let clone_group = &report.clone_groups[0];
    assert_eq!(clone_group.instances.len(), 3);
    assert!(clone_group.instances.iter().any(|instance| {
        instance
            .path
            .ends_with("functions/delete_user/shared/lib/pagination.dart")
            && instance.start_line == 1
    }));

    Ok(())
}

#[test]
fn copied_package_canonicalization_orders_same_line_instances_by_column()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "functions/shared/pubspec.yaml", "name: shared\n")?;
    write(
        &fixture,
        "functions/delete_user/shared/pubspec.yaml",
        "name: shared\n",
    )?;
    let canonical = fixture.path().join("functions/shared/lib/pagination.dart");
    let mirror = fixture
        .path()
        .join("functions/delete_user/shared/lib/pagination.dart");
    let mut group = CodeClone {
        fingerprint: "dup:test".to_owned(),
        instances: vec![
            instance(&mirror, 10, 12, 8),
            instance(&mirror, 10, 12, 4),
            instance(&canonical, 10, 12, 8),
            instance(&canonical, 10, 12, 4),
        ],
        line_count: 3,
        token_count: 20,
    };
    let mut filter = CopiedPackageFilter::new(fixture.path());

    filter.canonicalize_copied_package_instances(&mut group);

    assert_eq!(group.instances.len(), 2);
    assert!(group.instances.iter().all(|clone| clone.path == canonical));
    assert_eq!(
        group
            .instances
            .iter()
            .map(|clone| clone.column)
            .collect::<Vec<_>>(),
        vec![4, 8]
    );

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

fn pagination_clone_source() -> &'static str {
    r"Future<List<String>> fetchAllRowIds(Pages pages) async {
  final values = <String>[];
  var cursor = '';
  do {
    final page = await pages.list(cursor: cursor);
    for (final row in page.rows) {
      values.add(row.id);
    }
    cursor = page.nextCursor;
  } while (cursor.isNotEmpty);
  return values;
}

Future<List<String>> fetchAllFieldValues(Pages pages) async {
  final values = <String>[];
  var cursor = '';
  do {
    final page = await pages.list(cursor: cursor);
    for (final row in page.rows) {
      values.add(row.id);
    }
    cursor = page.nextCursor;
  } while (cursor.isNotEmpty);
  return values;
}
"
}

fn options(mode: DuplicateMode, min_lines: usize, min_tokens: usize) -> DuplicateOptions {
    DuplicateOptions {
        mode,
        min_tokens,
        min_lines,
        min_occurrences: 2,
        skip_local: false,
        ignore_imports: true,
        ignore_mapper_pairs: true,
        top: None,
        threshold: None,
    }
}

fn instance(
    path: &std::path::Path,
    start_line: usize,
    end_line: usize,
    column: usize,
) -> CodeCloneInstance {
    CodeCloneInstance {
        path: path.to_path_buf(),
        start_line,
        end_line,
        column,
    }
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
