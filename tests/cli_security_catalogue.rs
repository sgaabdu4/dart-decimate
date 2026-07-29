use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn weak_randomness_requires_dart_math_and_security_context()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: security_catalogue_fixture\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'dart:math';

final authToken = Random().nextInt(1000000).toString();
final OTPCode = Random().nextInt(1000000);
final generated = issueAuthToken(
  Random().nextInt(1000000),
);
final harmlessRoll = Random().nextInt(6);
final secureNonce = Random.secure().nextInt(1000000);
",
    )?;

    let (code, json) = security_json(&fixture, &[])?;
    let weak = candidates(&json)
        .iter()
        .find(|candidate| candidate["category"] == "weak-randomness")
        .ok_or("weak-randomness candidate")?;

    assert_eq!(code, 1);
    assert_eq!(weak["rule_id"], "dart-decimate/security-weak-randomness");
    assert_eq!(weak["cwe"][0], "CWE-338");
    assert_eq!(weak["candidate"]["source"], "dart-math-random");
    assert_eq!(weak["candidate"]["boundary"], "security-token-generation");
    assert_eq!(weak["candidate"]["effect"], "predictable-security-material");
    assert_eq!(weak["trace"][0]["role"], "source");
    assert_eq!(weak["occurrences"].as_array().map_or(0, Vec::len), 3);
    assert_eq!(json["summary"]["security_blind_spots"], 0);
    assert!(
        json["security_blind_spots"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );

    Ok(())
}

#[test]
fn weak_randomness_supports_prefixed_import_and_never_flags_secure_constructor()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: security_catalogue_fixture\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'dart:math' as math;

final sessionKey = math.Random().nextInt(1000000);
final passwordSalt = math.Random.secure().nextInt(1000000);
",
    )?;

    let (_, json) = security_json(&fixture, &[])?;
    let weak = candidates(&json)
        .iter()
        .find(|candidate| candidate["category"] == "weak-randomness")
        .ok_or("weak-randomness candidate")?;

    assert_eq!(weak["occurrences"].as_array().map_or(0, Vec::len), 1);
    assert!(
        weak["occurrences"][0]["evidence"]
            .as_str()
            .is_some_and(|evidence| evidence.contains("sessionKey"))
    );

    Ok(())
}

#[test]
fn weak_randomness_never_flags_secure_tearoffs_or_map_pins()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: security_catalogue_fixture\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'dart:math';

final createSecureRandom = Random.secure;
final resetCode = createSecureRandom().nextInt(1000000);
final mapPin = Random().nextInt(1000000);
",
    )?;

    let (_, json) = security_json(&fixture, &[])?;
    assert!(
        candidates(&json)
            .iter()
            .all(|candidate| candidate["category"] != "weak-randomness"),
        "{json:#}"
    );
    assert_eq!(json["summary"]["security_blind_spots"], 0, "{json:#}");

    Ok(())
}

#[test]
fn weak_randomness_tracks_returning_factories_without_flagging_secure_factories()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: security_catalogue_fixture\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'dart:math';

Random makeGenerator() => Random();
Random makeSecureGenerator() => Random.secure();
final makeClosureGenerator = () => Random();
final makeSecureClosureGenerator = () => Random.secure();

final resetCode = makeGenerator().nextInt(1000000);
final verificationCode = makeSecureGenerator().nextInt(1000000);
final confirmationCode = makeClosureGenerator().nextInt(1000000);
final backupCode = makeSecureClosureGenerator().nextInt(1000000);
",
    )?;

    let (_, json) = security_json(&fixture, &[])?;
    assert!(
        candidates(&json)
            .iter()
            .all(|candidate| candidate["category"] != "weak-randomness"),
        "{json:#}"
    );
    let blind_spots = json["security_blind_spots"]
        .as_array()
        .ok_or("security blind spots")?;
    assert_eq!(blind_spots.len(), 2, "{json:#}");
    assert!(blind_spots.iter().all(|spot| {
        spot["reason"] == "unflattened-random-flow"
            && spot["evidence"].as_str().is_some_and(|evidence| {
                evidence.contains("resetCode") || evidence.contains("confirmationCode")
            })
    }));

    Ok(())
}

#[test]
fn weak_randomness_recognizes_security_code_and_csrf_contexts()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: security_catalogue_fixture\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'dart:math';

final resetCode = Random().nextInt(1000000);
final verificationCode = Random().nextInt(1000000);
final csrf = Random().nextInt(1000000);
final colorCode = Random().nextInt(1000000);
final keyCode = Random().nextInt(1000000);
final gameSession = Random().nextInt(1000000);
final widgetKey = Random().nextInt(1000000);
",
    )?;

    let (_, json) = security_json(&fixture, &[])?;
    let weak = candidates(&json)
        .iter()
        .find(|candidate| candidate["category"] == "weak-randomness")
        .ok_or("weak-randomness candidate")?;

    assert_eq!(weak["occurrences"].as_array().map_or(0, Vec::len), 3);
    assert_eq!(json["summary"]["security_blind_spots"], 0);

    Ok(())
}

#[test]
fn weak_randomness_covers_security_code_vocabulary_and_multiline_flow()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: security_catalogue_fixture\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'dart:math';

typedef RNG = Random;

final authorizationCode = Random().nextInt(1000000);
final backupCode = Random().nextInt(1000000);
final passcode = Random().nextInt(1000000);

final createRandom = Random.new;

int tearOffResetCode() => createRandom().nextInt(1000000);

int multilineResetCode() {
  final generator =
      Random();
  return generator.nextInt(1000000);
}

int directMultilineResetCode() {
  return Random().nextInt(1000000);
}

int aliasMultilineResetCode() {
  return RNG().nextInt(1000000);
}
",
    )?;

    let (_, json) = security_json(&fixture, &[])?;
    let weak = candidates(&json)
        .iter()
        .find(|candidate| candidate["category"] == "weak-randomness")
        .ok_or("weak-randomness candidate")?;
    assert_eq!(weak["occurrences"].as_array().map_or(0, Vec::len), 6);
    let blind_spots = json["security_blind_spots"]
        .as_array()
        .ok_or("security_blind_spots array")?;
    assert_eq!(blind_spots.len(), 2, "{json:#}");
    assert!(blind_spots.iter().all(|spot| {
        spot["reason"] == "unflattened-random-flow"
            && (spot["evidence"]
                .as_str()
                .is_some_and(|evidence| evidence.contains("tearOffResetCode"))
                || spot["evidence"]
                    .as_str()
                    .is_some_and(|evidence| evidence.contains("generator.nextInt")))
    }));

    Ok(())
}

#[test]
fn weak_randomness_tracks_documented_alias_and_tearoff_forms()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: security_catalogue_fixture\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'dart:math';

typedef RNG = Random;
final resetCode = RNG().nextInt(1000000);

final createRandom = Random.new;
final generator = createRandom();
final resetCodeFromFactory = generator.nextInt(1000000);

final fakeFactory = 'Random.new';
final verificationCode = fakeFactory.length;
",
    )?;

    let (_, json) = security_json(&fixture, &[])?;
    let weak = candidates(&json)
        .iter()
        .find(|candidate| candidate["category"] == "weak-randomness")
        .ok_or("weak-randomness candidate")?;

    assert_eq!(weak["occurrences"].as_array().map_or(0, Vec::len), 1);
    assert!(
        json["security_blind_spots"]
            .as_array()
            .is_some_and(|spots| spots.iter().any(|spot| {
                spot["reason"] == "unflattened-random-flow"
                    && spot["evidence"]
                        .as_str()
                        .is_some_and(|evidence| evidence.contains("resetCodeFromFactory"))
            })),
        "{json:#}"
    );
    assert_eq!(
        json["security_blind_spots"].as_array().map_or(0, Vec::len),
        1,
        "{json:#}"
    );

    Ok(())
}

#[test]
fn ambiguous_and_unflattened_security_calls_are_reported_as_blind_spots()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: security_catalogue_fixture\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'dart:math';

final generator = Random();
final authToken = generator.nextInt(1000000);
final secretNonce = UnknownRandom().nextInt(1000000);
",
    )?;

    let (code, json) = security_json(&fixture, &[])?;
    let blind_spots = json["security_blind_spots"]
        .as_array()
        .ok_or("security_blind_spots array")?;

    assert_eq!(code, 0);
    assert!(candidates(&json).is_empty());
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["security_candidates"], 0);
    assert_eq!(json["summary"]["security_blind_spots"], 2);
    assert!(blind_spots.iter().any(|spot| {
        spot["reason"] == "unflattened-random-flow" && spot["path"] == "lib/main.dart"
    }));
    assert!(blind_spots.iter().any(|spot| {
        spot["reason"] == "ambiguous-random-provenance" && spot["path"] == "lib/main.dart"
    }));

    Ok(())
}

#[test]
fn secure_random_methods_and_flutter_keys_are_not_security_blind_spots()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: security_catalogue_fixture\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'dart:math';

final encryptionKey = Key.fromSecureRandom(32);
final widgetKey = ValueKey(Random().nextDouble());
final pageStorageKey = PageStorageKey(Random().nextDouble());
final resetCode = Random().nextInt(1000000);
",
    )?;

    let (_, json) = security_json(&fixture, &[])?;
    let weak = candidates(&json)
        .iter()
        .find(|candidate| candidate["category"] == "weak-randomness")
        .ok_or("weak-randomness candidate")?;

    assert_eq!(weak["occurrences"].as_array().map_or(0, Vec::len), 1);
    assert_eq!(json["summary"]["security_blind_spots"], 0, "{json:#}");

    Ok(())
}

#[test]
fn game_session_identifiers_do_not_become_security_blind_spots()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: security_catalogue_fixture\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'dart:math';

final random = Random();

class OfflineComputerGameState {
  factory OfflineComputerGameState.initial() {
    final sessionId = 'game_${random.nextInt(1 << 32)}';
    return OfflineComputerGameState._(sessionId);
  }

  const OfflineComputerGameState._(this.sessionId);
  final String sessionId;
}
",
    )?;

    let (_, json) = security_json(&fixture, &[])?;

    assert!(
        candidates(&json)
            .iter()
            .all(|candidate| candidate["category"] != "weak-randomness"),
        "{json:#}"
    );
    assert_eq!(json["summary"]["security_blind_spots"], 0, "{json:#}");

    Ok(())
}

#[test]
fn weak_randomness_preserves_fail_gate_redaction_sarif_and_schema_contracts()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: security_catalogue_fixture\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'dart:math';

final passwordToken = Random().nextInt(1000000);
const apiSecret = 'dart_decimate_fixture_secret_1234567890';
",
    )?;

    let (code, json) = security_json(&fixture, &["--fail-on-issues"])?;
    let serialized = serde_json::to_string(&json)?;
    assert_eq!(code, 1);
    assert!(!serialized.contains("dart_decimate_fixture_secret_1234567890"));

    let mut sarif_output = Vec::new();
    let sarif_code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--ci",
        ],
        &mut sarif_output,
    )?;
    let sarif = serde_json::from_slice::<Value>(&sarif_output)?;
    assert_eq!(sarif_code, 1);
    assert!(
        sarif["runs"][0]["results"]
            .as_array()
            .is_some_and(|results| {
                results.iter().any(|result| {
                    result["ruleId"] == "dart-decimate/security-weak-randomness"
                        && result["properties"]["cwe"][0] == "CWE-338"
                })
            })
    );

    let mut schema_output = Vec::new();
    let schema_code = run_from(
        ["dart-decimate", "report-schema", "--format", "json"],
        &mut schema_output,
    )?;
    let schema = serde_json::from_slice::<Value>(&schema_output)?;
    assert_eq!(schema_code, 0);
    assert_eq!(
        schema["properties"]["security_blind_spots"]["items"]["$ref"],
        "#/$defs/security_blind_spot"
    );
    assert_eq!(
        schema["$defs"]["summary"]["properties"]["security_blind_spots"]["type"],
        "integer"
    );

    Ok(())
}

#[test]
fn weak_randomness_rule_alias_can_disable_the_candidate() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: security_catalogue_fixture\n",
    )?;
    write(
        &fixture,
        ".dart-decimaterc",
        "[rules]\nweak-randomness = \"off\"\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'dart:math';\nfinal authToken = Random().nextInt(1000000);\n",
    )?;

    let (code, json) = security_json(&fixture, &[])?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["security_candidates"], 0);
    assert!(candidates(&json).is_empty());

    Ok(())
}

#[test]
fn human_output_never_calls_nonzero_blind_spots_clean_security_proof()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: security_catalogue_fixture\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "final authToken = UnknownRandom().nextInt(1000000);\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "security",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "human",
        ],
        &mut output,
    )?;

    let text = String::from_utf8(output)?;
    assert_eq!(code, 0);
    assert!(text.contains("1 security blind spot"));
    assert!(text.contains("not clean security proof"));
    assert!(!text.contains("No findings. The selected Dart graph checks passed."));

    Ok(())
}

#[test]
fn hidden_or_shadowed_random_symbols_abstain_to_blind_spots()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: security_catalogue_fixture\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'dart:math' hide Random;
import 'dart:math' as math;

class Random {
  int nextInt(int max) => 4;
}

final math = Object();
final authToken = Random().nextInt(1000000);
final sessionKey = math.Random().nextInt(1000000);
",
    )?;

    let (code, json) = security_json(&fixture, &[])?;
    assert_eq!(code, 0);
    assert!(candidates(&json).is_empty());
    assert_eq!(json["summary"]["security_blind_spots"], 2);
    assert!(
        json["security_blind_spots"]
            .as_array()
            .is_some_and(|spots| {
                spots
                    .iter()
                    .all(|spot| spot["reason"] == "ambiguous-random-provenance")
            })
    );

    Ok(())
}

#[test]
fn security_catalogue_output_is_byte_stable() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: security_catalogue_fixture\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'dart:math';\nfinal authToken = Random().nextInt(1000000);\nfinal sessionKey = UnknownRandom().nextInt(1000000);\n",
    )?;
    let args = [
        "dart-decimate",
        "security",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ];
    let mut first = Vec::new();
    let mut second = Vec::new();

    let first_code = run_from(args, &mut first)?;
    let second_code = run_from(args, &mut second)?;

    assert_eq!(first_code, second_code);
    assert_eq!(first, second);

    Ok(())
}

fn security_json(
    fixture: &TempDir,
    extra: &[&str],
) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let mut args = vec![
        "dart-decimate",
        "security",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ];
    args.extend_from_slice(extra);
    let mut output = Vec::new();
    let code = run_from(args, &mut output)?;
    Ok((code, serde_json::from_slice(&output)?))
}

fn candidates(json: &Value) -> &[Value] {
    json["security_candidates"]
        .as_array()
        .map_or(&[], Vec::as_slice)
}

fn write(fixture: &TempDir, relative: &str, contents: &str) -> std::io::Result<()> {
    let path = fixture.path().join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}
