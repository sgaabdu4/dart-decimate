use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn default_suppresses_explicit_to_entity_and_to_domain_pairs()
-> Result<(), Box<dyn std::error::Error>> {
    for (source_class, target_class, mapper) in [
        (
            "AccountDto",
            "AccountEntity",
            "AccountEntity toEntity() => AccountEntity(id: id, name: name, email: email, phone: phone);",
        ),
        (
            "RemoteUser",
            "User",
            "User toDomain() => User(id: id, name: name, email: email, phone: phone);",
        ),
    ] {
        let fixture = tempfile::tempdir()?;
        write(&fixture, "pubspec.yaml", "name: app\n")?;
        write_pair(
            &fixture,
            "lib/data/source.dart",
            source_class,
            "lib/domain/target.dart",
            target_class,
            mapper,
        )?;
        write_real_clone(&fixture)?;

        let (_, json) = run_dupes(&fixture)?;
        assert_eq!(
            clone_path_sets(&json),
            vec![vec!["lib/a.dart", "lib/b.dart"]],
            "{source_class} -> {target_class}"
        );
    }
    Ok(())
}

#[test]
fn mapper_pair_suppression_can_be_disabled() -> Result<(), Box<dyn std::error::Error>> {
    for (config_path, config) in [
        (".dart-decimaterc", "[dupes]\nignore_mapper_pairs = false\n"),
        (
            ".dart-decimaterc.jsonc",
            "{ \"dupes\": { \"ignoreMapperPairs\": false } }\n",
        ),
    ] {
        let fixture = tempfile::tempdir()?;
        write(&fixture, "pubspec.yaml", "name: app\n")?;
        write(&fixture, config_path, config)?;
        write_pair(
            &fixture,
            "lib/data/remote_user.dart",
            "RemoteUser",
            "lib/domain/user.dart",
            "User",
            "User toDomain() => User(id: id, name: name, email: email, phone: phone);",
        )?;

        let (_, json) = run_dupes(&fixture)?;
        assert!(
            clone_path_sets(&json)
                .iter()
                .any(|paths| { paths == &["lib/data/remote_user.dart", "lib/domain/user.dart",] }),
            "{config_path}"
        );
    }
    Ok(())
}

#[test]
fn arbitrary_conversion_method_does_not_suppress_clone() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_pair(
        &fixture,
        "lib/data/remote_user.dart",
        "RemoteUser",
        "lib/domain/user.dart",
        "User",
        "User toRecord() => User(id: id, name: name, email: email, phone: phone);",
    )?;

    let (_, json) = run_dupes(&fixture)?;
    assert!(
        clone_path_sets(&json)
            .iter()
            .any(|paths| { paths == &["lib/data/remote_user.dart", "lib/domain/user.dart",] })
    );
    Ok(())
}

#[test]
fn qualified_mapper_return_type_is_recognized() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/data/remote_user.dart",
        &format!(
            "import '../domain/user.dart' as domain;\n\n{}",
            class_source(
                "RemoteUser",
                Some(
                    "domain.User toDomain() => domain.User(id: id, name: name, email: email, phone: phone);",
                ),
            )
        ),
    )?;
    write(
        &fixture,
        "lib/domain/user.dart",
        &class_source("User", None),
    )?;
    write_real_clone(&fixture)?;

    let (_, json) = run_dupes(&fixture)?;
    assert_eq!(
        clone_path_sets(&json),
        vec![vec!["lib/a.dart", "lib/b.dart"]]
    );
    Ok(())
}

#[test]
fn valid_mapper_pair_does_not_hide_clone_shared_with_third_class()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_pair(
        &fixture,
        "lib/data/remote_user.dart",
        "RemoteUser",
        "lib/domain/user.dart",
        "User",
        "User toDomain() => User(id: id, name: name, email: email, phone: phone);",
    )?;
    write(
        &fixture,
        "lib/other/user_snapshot.dart",
        &class_source("UserSnapshot", None),
    )?;

    let (_, json) = run_dupes(&fixture)?;
    assert!(clone_path_sets(&json).iter().any(|paths| {
        paths.contains(&"lib/data/remote_user.dart")
            && paths.contains(&"lib/domain/user.dart")
            && paths.contains(&"lib/other/user_snapshot.dart")
    }));
    Ok(())
}

fn write_pair(
    fixture: &TempDir,
    source_path: &str,
    source_class: &str,
    target_path: &str,
    target_class: &str,
    mapper: &str,
) -> Result<(), std::io::Error> {
    let target_file = target_path.rsplit('/').next().unwrap_or(target_path);
    write(
        fixture,
        source_path,
        &format!(
            "import '../domain/{target_file}';\n\n{}",
            class_source(source_class, Some(mapper))
        ),
    )?;
    write(fixture, target_path, &class_source(target_class, None))
}

fn class_source(name: &str, mapper: Option<&str>) -> String {
    format!(
        "class {name} {{\n\
  const {name}({{\n\
    required this.id,\n\
    required this.name,\n\
    required this.email,\n\
    required this.phone,\n\
  }});\n\
  final String id;\n\
  final String name;\n\
  final String email;\n\
  final String phone;\n\
{}\n\
}}\n",
        mapper.map_or_else(String::new, |mapper| format!("\n  {mapper}\n"))
    )
}

fn write_real_clone(fixture: &TempDir) -> Result<(), std::io::Error> {
    let clone = "void shared() {\n\
  final items = [1, 2, 3];\n\
  final active = items.where((item) => item > 1);\n\
  print(active.length);\n\
}\n";
    write(fixture, "lib/a.dart", clone)?;
    write(fixture, "lib/b.dart", clone)
}

fn run_dupes(fixture: &TempDir) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(
        [
            "dart-decimate",
            "dupes",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
        ],
        &mut output,
    )?;
    Ok((code, serde_json::from_slice(&output)?))
}

fn clone_path_sets(json: &Value) -> Vec<Vec<&str>> {
    json["clone_groups"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|group| {
            let mut paths = group["instances"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|instance| instance["path"].as_str())
                .collect::<Vec<_>>();
            paths.sort_unstable();
            paths.dedup();
            paths
        })
        .collect()
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
