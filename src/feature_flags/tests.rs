use std::fs;

use tempfile::TempDir;

use crate::{FeatureFlagOptions, detect_feature_flags, scan_project};

#[test]
fn detects_dart_and_flutter_feature_flag_patterns() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'dart:io';
import 'package:firebase_remote_config/firebase_remote_config.dart';

const newUi = bool.fromEnvironment('FEATURE_NEW_UI');
const cohort = String.fromEnvironment('EXPERIMENT_COHORT');

void main() {
  if (Platform.environment['ENABLE_PAYWALL'] == 'true') {}
  final remoteConfig = FirebaseRemoteConfig.instance;
  if (remoteConfig.getBool('new_checkout')) {}
  final enabled = client.boolVariation('checkout-redesign', false);
  print(enabled);
}
",
    )?;
    let project = scan_project(fixture.path())?;

    let report = detect_feature_flags(&project, &FeatureFlagOptions::default())?;

    let names = report
        .flags
        .iter()
        .map(|flag| flag.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(report.total_occurrences, 5);
    assert!(names.contains(&"FEATURE_NEW_UI"));
    assert!(names.contains(&"EXPERIMENT_COHORT"));
    assert!(names.contains(&"ENABLE_PAYWALL"));
    assert!(names.contains(&"new_checkout"));
    assert!(names.contains(&"checkout-redesign"));

    Ok(())
}

#[test]
fn skips_comments_and_generated_files() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "lib/main.dart",
        "// bool.fromEnvironment('FEATURE_COMMENTED')\nvoid main() {}\n",
    )?;
    write(
        &fixture,
        "lib/app.g.dart",
        "const generated = bool.fromEnvironment('FEATURE_GENERATED');\n",
    )?;
    let project = scan_project(fixture.path())?;

    let report = detect_feature_flags(&project, &FeatureFlagOptions::default())?;

    assert_eq!(report.flags, []);
    assert_eq!(report.total_occurrences, 0);

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
