use std::fs;

use tempfile::TempDir;

use crate::{SecurityOptions, analyze_security, scan_project};

#[test]
fn detects_dart_and_flutter_security_candidate_patterns() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'dart:io';

const accessToken = 'dart_decimate_fixture_value_1234567890';

Future<void> main(dynamic db, dynamic prefs, dynamic controller, String command, String id, String token) async {
  final uri = Uri.parse('http://api.example.com/login');
  final client = HttpClient();
  client.badCertificateCallback = (cert, host, port) => true;
  controller.setJavaScriptMode(JavaScriptMode.unrestricted);
  await Process.run(command, ['-c', id]);
  await db.rawQuery('SELECT * FROM users WHERE id = $id');
  await prefs.setString('access_token', token);
  print(uri);
}
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(
        &project,
        &SecurityOptions {
            top: None,
            surface: true,
            ..SecurityOptions::default()
        },
        None,
    )?;
    let rules = report
        .candidates
        .iter()
        .map(|candidate| candidate.rule_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(report.analyzed_files, 1);
    assert_eq!(report.total_occurrences, 7);
    assert_eq!(report.attack_surface.len(), 7);
    assert!(rules.contains(&"dart-decimate/security-hardcoded-secret"));
    assert!(rules.contains(&"dart-decimate/security-insecure-transport"));
    assert!(rules.contains(&"dart-decimate/security-tls-bypass"));
    assert!(rules.contains(&"dart-decimate/security-webview-risk"));
    assert!(rules.contains(&"dart-decimate/security-process-execution"));
    assert!(rules.contains(&"dart-decimate/security-raw-sql"));
    assert!(rules.contains(&"dart-decimate/security-plain-secret-storage"));
    assert!(
        report
            .candidates
            .iter()
            .flat_map(|candidate| &candidate.occurrences)
            .all(|occurrence| !occurrence
                .evidence
                .contains("dart_decimate_fixture_value_1234567890"))
    );

    Ok(())
}

#[test]
fn skips_comments_generated_tests_and_placeholders() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "// const accessToken = 'dart_decimate_fixture_value_1234567890';
const apiKey = 'YOUR_API_KEY';
const firebase = FirebaseOptions(apiKey: 'REDACTED_FIREBASE_API_KEY');
final uri = Uri.parse('http://localhost:8080');
Future<void> run() => Process.run('git', ['status']);
Future<void> query(dynamic db, String id) => db.rawQuery('SELECT * FROM users WHERE id = ?', [id]);
",
    )?;
    write(
        &fixture,
        "lib/generated.g.dart",
        "const accessToken = 'dart_decimate_fixture_value_1234567890';\n",
    )?;
    write(
        &fixture,
        "test/security_test.dart",
        "const accessToken = 'dart_decimate_fixture_value_1234567890';\n",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;

    assert!(report.candidates.is_empty());
    assert_eq!(report.total_occurrences, 0);

    Ok(())
}

#[test]
fn skips_password_named_non_secret_literals() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:example/features/auth/password_form_field.dart';

class Routes {
  static const String forgotPassword = '/forgot-password';
  static const String resetPassword = '/reset-password';
  static const String passwordRecoveryRedirectUrl = 'https://example.invalid/reset-password';
}

class Copy {
  String get settingsSecurityChangePassword => 'Change Password';
  static const String invalidCredentials = 'Invalid email or password';
  static const String passwordsDoNotMatch = 'Passwords do not match';
  static const String passwordTooShort = 'Use at least 8 characters';
  static const String cloudFunctionSubject = 'Jabal Sina Cloud Function Error | token_notifications';
  static const String pingResponse = 'Pong - Token Notifications Active';
  static const String tokenTitle = '🎫 Token Issued';
  static const String requestTokenOperation = 'Pre-fetch token';
  static const String updatingPassword = 'Updating password...';
}
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;

    assert!(report.candidates.is_empty());
    assert_eq!(report.total_occurrences, 0);

    Ok(())
}

#[test]
fn skips_low_entropy_labels_with_secret_like_words() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "void log(String message) => print(message);

void main() {
  log('requestAuthorization result');
  log('synchronizeRemoteCatalog');
}
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;

    assert!(report.candidates.is_empty());
    assert_eq!(report.total_occurrences, 0);

    Ok(())
}

#[test]
fn reports_secret_bindings_and_shaped_literals() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "const clientSecret = 'ordinary dictionary words';

void authenticate({required String accessToken}) {}
void log(String message) => print(message);

void main() {
  authenticate(accessToken: 'another dictionary phrase');
  log('ghp_0123456789abcdef0123456789abcdef');
  print(clientSecret);
}
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;

    assert_eq!(report.total_occurrences, 3);
    assert_eq!(report.candidates.len(), 1);
    assert_eq!(
        report.candidates[0].rule_id,
        "dart-decimate/security-hardcoded-secret"
    );
    assert_eq!(report.candidates[0].occurrences.len(), 3);

    Ok(())
}

#[test]
fn reports_operational_copy_with_concrete_token_segments() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "class Session {
  static const String accessToken = 'Token issued: abc123abc123';
  static const String operationMessage = 'Pre-fetch token abc123abc123';
}
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;

    assert_eq!(report.total_occurrences, 2);
    assert_eq!(
        report.candidates[0].rule_id,
        "dart-decimate/security-hardcoded-secret"
    );
    assert_eq!(report.candidates[0].occurrences.len(), 2);

    Ok(())
}

#[test]
fn reports_user_copy_with_secret_bindings_or_token_segments()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "class Session {
  static const String accessToken = 'Invalid token abc123abc123';
  static const String resetToken = 'Reset link invalid or expired abc123abc123';
}
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;

    assert_eq!(report.total_occurrences, 2);
    assert_eq!(
        report.candidates[0].rule_id,
        "dart-decimate/security-hardcoded-secret"
    );
    assert_eq!(report.candidates[0].occurrences.len(), 2);

    Ok(())
}

#[test]
fn skips_dot_qualified_diagnostic_identifiers_but_reports_jwts()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "class Crash {
  static void error(Object error, StackTrace stackTrace, {required String reason}) {}
}

void report(Object error, StackTrace stackTrace) {
  Crash.error(error, stackTrace, reason: 'ActiveWorkoutNotifier.saveActiveWorkout.squadCheckIn');
}

const jwt = 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c';
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;

    assert_eq!(report.total_occurrences, 1);
    assert_eq!(
        report.candidates[0].rule_id,
        "dart-decimate/security-hardcoded-secret"
    );
    assert_eq!(report.candidates[0].occurrences[0].location.line, 9);

    Ok(())
}

#[test]
fn reports_secret_named_urls_with_concrete_secret_parameters()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "class Routes {
  static const String resetPassword = '/reset-password?next=/settings';
  static const String resetPasswordSuccess = '/reset-password/success-page';
  static const String resetPasswordToken = '/reset-password?token=dartdecimate12345';
  static const String recoveryAccessToken = 'https://auth.invalid/reset-password#access_token=dartdecimate67890';
  static const String resetPasswordPathToken = 'https://auth.invalid/reset-password/dartdecimate12345';
}
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;

    assert_eq!(report.total_occurrences, 3);
    assert_eq!(
        report.candidates[0].rule_id,
        "dart-decimate/security-hardcoded-secret"
    );
    assert_eq!(report.candidates[0].occurrences.len(), 3);

    Ok(())
}

#[test]
fn classifies_firebase_options_api_key_separately() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/firebase_config.dart",
        "const options = FirebaseOptions(
  apiKey: 'DartDecimateFirebaseKeyValue123456789',
  appId: '1:123:web:abc',
  messagingSenderId: '123',
  projectId: 'example-project',
);
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;

    assert_eq!(report.total_occurrences, 1);
    assert_eq!(
        report.candidates[0].rule_id,
        "dart-decimate/security-firebase-api-key"
    );
    assert_eq!(report.candidates[0].sink, "firebase-api-key");

    Ok(())
}

#[test]
fn skips_dynamic_firebase_options_api_key() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/firebase_options.dart",
        "const options = FirebaseOptions(
  apiKey: '$firebaseApiKey',
  appId: '1:123:web:abc',
);
const emptyOptions = FirebaseOptions(apiKey: '', appId: '1:123:web:def');
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;

    assert!(report.candidates.is_empty());
    assert_eq!(report.total_occurrences, 0);

    Ok(())
}

#[test]
fn classifies_compact_firebase_options_literals_by_argument()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/firebase_options.dart",
        "const options = FirebaseOptions(apiKey: 'DartDecimateFirebaseKeyValue123456789', appId: '1:123:web:abc', clientSecret: 'dart_decimate_fixture_value_1234567890');\n",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;
    let mut rules = report
        .candidates
        .iter()
        .map(|candidate| candidate.rule_id.as_str())
        .collect::<Vec<_>>();
    rules.sort_unstable();

    assert_eq!(report.total_occurrences, 1);
    assert_eq!(rules, vec!["dart-decimate/security-hardcoded-secret"]);

    Ok(())
}

#[test]
fn classifies_newline_firebase_options_api_key_by_argument()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/firebase_options.dart",
        "const options = FirebaseOptions(apiKey:
  'DartDecimateFirebaseKeyValue123456789', appId: '1:123:web:abc', clientSecret: 'dart_decimate_fixture_value_1234567890');\n",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;
    let mut rules = report
        .candidates
        .iter()
        .map(|candidate| candidate.rule_id.as_str())
        .collect::<Vec<_>>();
    rules.sort_unstable();

    assert_eq!(report.total_occurrences, 1);
    assert_eq!(rules, vec!["dart-decimate/security-hardcoded-secret"]);

    Ok(())
}

#[test]
fn classifies_newline_firebase_options_secret_argument() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/firebase_options.dart",
        "const options = FirebaseOptions(
  apiKey: 'DartDecimateFirebaseKeyValue123456789',
  appId: '1:123:web:abc',
  clientSecret:
    'dart_decimate_fixture_value_1234567890',
  projectId: 'example-project',
);
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;
    let mut rules = report
        .candidates
        .iter()
        .map(|candidate| candidate.rule_id.as_str())
        .collect::<Vec<_>>();
    rules.sort_unstable();

    assert_eq!(report.total_occurrences, 1);
    assert_eq!(rules, vec!["dart-decimate/security-hardcoded-secret"]);

    Ok(())
}

#[test]
fn locates_javascript_password_autofill_at_assignment_literal()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "const loginJs = '''
  var inputs = document.querySelectorAll('input');
  for (var i = 0; i < inputs.length; i++) {
    if (inputs[i].type === 'password')
      inputs[i].value = 'dart_decimate_fixture_password_value_12345';
  }
''';
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;

    assert_eq!(report.total_occurrences, 1);
    assert_eq!(
        report.candidates[0].rule_id,
        "dart-decimate/security-hardcoded-secret"
    );
    assert_eq!(report.candidates[0].occurrences[0].location.line, 5);

    Ok(())
}

#[test]
fn skips_javascript_password_autofill_when_assignment_is_not_literal()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "const loginJs = '''
  if (input.type === 'password') input.value = token || 'dart_decimate_fixture_password_value_12345';
''';
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;

    assert!(report.candidates.is_empty());
    assert_eq!(report.total_occurrences, 0);

    Ok(())
}

#[test]
fn skips_javascript_password_autofill_when_password_hint_is_unrelated()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "const loginJs = '''
  if (input.type === 'password' && passwordResetForm.email) passwordResetForm.email.value = 'alice@company.invalid';
''';
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;

    assert!(report.candidates.is_empty());
    assert_eq!(report.total_occurrences, 0);

    Ok(())
}

#[test]
fn skips_javascript_password_autofill_when_value_target_is_unrelated()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "const loginJs = '''
  if (input.type === 'password') email.value = 'alice@company.invalid';
''';
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;

    assert!(report.candidates.is_empty());
    assert_eq!(report.total_occurrences, 0);

    Ok(())
}

#[test]
fn skips_javascript_password_autofill_when_password_hint_is_negative()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "const loginJs = '''
  if (input.type !== 'password') input.value = 'alice@company.invalid';
  if (input.matches(':not([type=password])')) input.value = 'alice@company.invalid';
''';
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;

    assert!(report.candidates.is_empty());
    assert_eq!(report.total_occurrences, 0);

    Ok(())
}

#[test]
fn skips_javascript_password_autofill_when_target_selector_is_negative()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "const loginJs = '''
  document.querySelector('input:not([type=password])').value = 'alice@company.invalid';
''';
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;

    assert!(report.candidates.is_empty());
    assert_eq!(report.total_occurrences, 0);

    Ok(())
}

#[test]
fn skips_javascript_password_autofill_when_only_parent_target_is_password_named()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "const loginJs = '''
  passwordResetForm.email.value = 'alice@company.invalid';
  if (passwordResetForm.email) passwordResetForm.email.value = 'alice@company.invalid';
''';
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;

    assert!(report.candidates.is_empty());
    assert_eq!(report.total_occurrences, 0);

    Ok(())
}

#[test]
fn skips_flutter_commands_logs_and_interpolated_bearer_headers()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "/// This shouldn't shift later string positions.
Future<void> main(dynamic viewModel, dynamic prefs, String token) async {
  viewModel.load.execute();
  viewModel.login.execute((email: 'user@example.com', password: 'not-a-secret'));
  _log.severe(
    'Failed to fetch Token from SharedPreferences',
  );
  _log.warning('Failed to set token');
  if (request.headers['Authorization'] != 'Bearer $token') {}
  final header = 'Bearer $token';
  await prefs.setString('access_token', token);
  print(header);
}
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;

    assert_eq!(report.total_occurrences, 1);
    assert_eq!(
        report.candidates[0].rule_id,
        "dart-decimate/security-plain-secret-storage"
    );

    Ok(())
}

#[test]
fn top_limits_grouped_candidates_but_preserves_total_occurrences()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "const accessToken = 'dart_decimate_fixture_value_1234567890';
final uri = Uri.parse('http://api.example.com/login');
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(
        &project,
        &SecurityOptions {
            top: Some(1),
            surface: false,
            ..SecurityOptions::default()
        },
        None,
    )?;

    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.total_occurrences, 2);

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
