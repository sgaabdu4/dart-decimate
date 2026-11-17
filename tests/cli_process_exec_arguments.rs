use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;

const ROWS: &str = include_str!("fixtures/process-exec-rows.json");

#[test]
fn process_calls_are_safe_only_with_fixed_executables_and_placed_variable_arguments()
-> Result<(), Box<dyn std::error::Error>> {
    let rows = serde_json::from_str::<Vec<Value>>(ROWS)?;
    let mut failures: Vec<String> = Vec::new();
    for row in &rows {
        let case = row["case"].as_str().ok_or("case")?;
        let body = row["source"].as_str().ok_or("source")?;
        let flagged = row["flagged"].as_bool().ok_or("flagged")?;
        let category = row["category"].as_str().ok_or("category")?;
        let floor = row["main"].as_u64().ok_or("main")?;
        if std::env::var("ROW").is_ok_and(|only| only != case) {
            continue;
        }
        let fixture = tempfile::tempdir()?;
        fs::write(fixture.path().join("pubspec.yaml"), "name: app\n")?;
        fs::create_dir_all(fixture.path().join("lib"))?;
        fs::write(
            fixture.path().join("lib/main.dart"),
            format!("import 'dart:io';\n\n{body}"),
        )?;
        let mut output = Vec::new();
        run_from(
            [
                "dart-decimate",
                "security",
                fixture.path().to_str().unwrap_or("."),
                "--format",
                "json",
            ],
            &mut output,
        )?;
        let json = serde_json::from_slice::<Value>(&output)?;
        let found = json["security_candidates"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|candidate| candidate["category"] == category)
            .map(|candidate| candidate["occurrences"].as_array().map_or(0, Vec::len))
            .sum::<usize>() as u64;

        if flagged {
            if found < floor.max(1) {
                failures.push(format!("{case}: {found} under floor {floor}"));
            }
        } else if found != 0 {
            failures.push(format!("{case}: a safe row has {found} findings"));
        }
    }

    assert!(
        failures.is_empty(),
        "{} rows fail:\n{}",
        failures.len(),
        failures.join("\n")
    );

    Ok(())
}
