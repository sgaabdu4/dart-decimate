use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;

const SOURCE: &str =
    include_str!("fixtures/interface-signatures/lib/interface_signature_repro.dart");

#[test]
fn interface_signatures_do_not_fail_check_or_dupes() -> Result<(), Box<dyn std::error::Error>> {
    for header in [
        "abstract class",
        "abstract interface class",
        "abstract base class",
        "mixin",
    ] {
        for parameter in [
            "Object Function(int)",
            "Object Function({required int count})",
            "Object Function(int, [String?])",
            "Object Function<T>(T)",
        ] {
            let source = SOURCE
                .replace("abstract class", header)
                .replace("Object Function(int)", parameter);
            for command in ["check", "dupes"] {
                let (code, report) = run(&source, command, &[])?;
                assert_eq!(code, 0, "{header} / {parameter}: {report}");
                assert_eq!(report["verdict"], "pass");
                assert_eq!(report["summary"]["code_duplications"], 0);
                assert_eq!(report["summary"]["duplicated_lines"], 0);
                assert_eq!(report["summary"]["duplication_threshold_exceeded"], false);
                assert_eq!(report["clone_groups"], serde_json::json!([]));
                assert_eq!(report["findings"], serde_json::json!([]));
            }
        }
    }
    Ok(())
}

#[test]
fn signature_windows_starting_inside_parameters_are_excluded()
-> Result<(), Box<dyn std::error::Error>> {
    let source = SOURCE.replacen("writeStream", "declareStream", 1);
    let (code, report) = run(
        &source,
        "dupes",
        &["--min-lines", "3", "--min-tokens", "15"],
    )?;
    assert_eq!(code, 0, "{report}");
    assert_eq!(report["clone_groups"], serde_json::json!([]));
    Ok(())
}

#[test]
fn concrete_duplicates_remain_after_declaration_occurrence_is_removed()
-> Result<(), Box<dyn std::error::Error>> {
    let implementation = SOURCE
        .split("class StorageAdapter")
        .nth(1)
        .ok_or("missing implementation")?;
    let source = format!("{SOURCE}\nclass SecondAdapter{implementation}");
    let (code, report) = run(&source, "dupes", &[])?;
    assert_eq!(code, 1);
    assert_eq!(report["summary"]["duplication_threshold_exceeded"], true);
    let groups = report["clone_groups"].as_array().ok_or("missing groups")?;
    assert!(!groups.is_empty());
    for group in groups {
        let instances = group["instances"].as_array().ok_or("missing instances")?;
        assert!(instances.len() >= 2);
        assert!(
            instances
                .iter()
                .all(|instance| instance["start_line"].as_u64().unwrap_or(0) > 11)
        );
    }
    Ok(())
}

#[test]
fn concrete_logic_inside_abstract_classes_remains_reported()
-> Result<(), Box<dyn std::error::Error>> {
    let body = "  void work() {\n    final values = [1, 2, 3, 4, 5];\n    final active = values.where((value) => value > 2).toList();\n    final total = active.fold(0, (sum, value) => sum + value);\n    print(total);\n  }\n";
    let source = format!("abstract class First {{\n{body}}}\nabstract class Second {{\n{body}}}\n");
    let (code, report) = run(&source, "dupes", &[])?;
    assert_eq!(code, 1, "{report}");
    assert_eq!(report["summary"]["duplication_threshold_exceeded"], true);
    Ok(())
}

#[test]
fn suppressed_clone_has_consistent_findings_inventory_and_threshold()
-> Result<(), Box<dyn std::error::Error>> {
    let body = "void shared() {\n  final items = [1, 2, 3];\n  final active = items.where((item) => item > 1);\n  print(active.length);\n}\n";
    let source = format!("// dart-decimate-ignore-next-line code-duplication\n{body}\n{body}");
    for extra in [vec![], vec!["--top", "1"]] {
        let mut args = vec!["--mode", "strict", "--min-lines", "5", "--min-tokens", "10"];
        args.extend(extra);
        let (code, report) = run(&source, "dupes", &args)?;
        assert_eq!(code, 0, "{report}");
        assert_eq!(report["verdict"], "pass");
        assert_eq!(report["findings"], serde_json::json!([]));
        assert_eq!(report["clone_groups"], serde_json::json!([]));
        assert_eq!(report["summary"]["duplicated_lines"], 0);
        assert_eq!(report["summary"]["duplication_threshold_exceeded"], false);
    }
    Ok(())
}

#[test]
fn suppression_with_top_keeps_other_groups_in_threshold() -> Result<(), Box<dyn std::error::Error>>
{
    let body = "void shared() {\n  final items = [1, 2, 3];\n  final active = items.where((item) => item > 1);\n  print(active.length);\n}\n";
    let second = body.replace("shared", "second").replace("items", "values");
    let third = body.replace("shared", "third").replace("items", "numbers");
    let source = format!(
        "// dart-decimate-ignore-next-line code-duplication\n{body}\n{body}\n{second}\n{second}\n{third}\n{third}"
    );
    let (code, report) = run(
        &source,
        "dupes",
        &[
            "--mode",
            "strict",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
            "--top",
            "1",
        ],
    )?;
    assert_eq!(code, 1, "{report}");
    assert_eq!(report["clone_groups"].as_array().map(Vec::len), Some(1));
    assert_eq!(report["findings"].as_array().map(Vec::len), Some(1));
    assert_eq!(report["summary"]["findings"], 1);
    // Whole-file windows can include adjoining braces; all surviving groups
    // must still count even though only one group is displayed.
    let (_, unlimited) = run(
        &source,
        "dupes",
        &["--mode", "strict", "--min-lines", "5", "--min-tokens", "10"],
    )?;
    assert_eq!(
        report["summary"]["duplicated_lines"],
        unlimited["summary"]["duplicated_lines"]
    );
    assert!(report["summary"]["duplicated_lines"].as_u64().unwrap_or(0) >= 20);
    assert_eq!(report["summary"]["duplication_threshold_exceeded"], true);
    Ok(())
}

fn run(
    source: &str,
    command: &str,
    extra: &[&str],
) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    fs::create_dir(fixture.path().join("lib"))?;
    fs::write(
        fixture.path().join("pubspec.yaml"),
        "name: interface_signatures\n",
    )?;
    fs::write(fixture.path().join("lib/contracts.dart"), source)?;
    let root = fixture.path().to_str().ok_or("invalid path")?;
    let mut args = vec![
        "dart-decimate",
        command,
        root,
        "--threshold",
        "0",
        "--strict",
        "--format",
        "json",
    ];
    args.extend(extra);
    let mut output = Vec::new();
    let code = run_from(args, &mut output)?;
    Ok((code, serde_json::from_slice(&output)?))
}
