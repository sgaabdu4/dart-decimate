use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

#[derive(Clone)]
struct FlutterL10nConfig {
    output_dir: PathBuf,
    output_file: String,
}

#[derive(Clone)]
struct CachedFlutterL10nConfig {
    length: u64,
    modified: Option<SystemTime>,
    config: Option<FlutterL10nConfig>,
}

static FLUTTER_L10N_CONFIGS: LazyLock<Mutex<BTreeMap<PathBuf, CachedFlutterL10nConfig>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));

pub(crate) const GENERATED_DART_SUFFIXES: &[&str] = &[
    ".g.dart",
    ".freezed.dart",
    ".gen.dart",
    ".gr.dart",
    ".mocks.dart",
    ".pb.dart",
    ".pbenum.dart",
    ".pbjson.dart",
    ".pbgrpc.dart",
];

#[must_use]
pub(crate) fn is_generated_dart_path(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    is_generated_dart_file_name(file_name)
        || is_flutter_gen_l10n_path(path, file_name)
        || is_drift_generated_schema_path(path, file_name)
        || is_configured_flutter_gen_l10n_path(path, file_name)
        || is_flutterfire_options_path(path)
}

#[must_use]
pub(crate) fn is_generated_dart_file_name(name: &str) -> bool {
    GENERATED_DART_SUFFIXES
        .iter()
        .any(|suffix| name.ends_with(suffix))
}

fn is_flutter_gen_l10n_path(path: &Path, file_name: &str) -> bool {
    (file_name == "app_localizations.dart"
        || file_name
            .strip_prefix("app_localizations_")
            .is_some_and(is_flutter_gen_l10n_locale_file))
        && path.components().any(|component| {
            matches!(
                component.as_os_str().to_str(),
                Some("l10n" | "gen_l10n" | "generated")
            )
        })
}

fn is_drift_generated_schema_path(path: &Path, file_name: &str) -> bool {
    let Some(version) = file_name
        .strip_prefix("schema_v")
        .and_then(|name| name.strip_suffix(".dart"))
    else {
        return false;
    };
    !version.is_empty()
        && version.bytes().all(|byte| byte.is_ascii_digit())
        && path
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|parent| parent == "generated")
}

fn is_configured_flutter_gen_l10n_path(path: &Path, file_name: &str) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    let Some(config) = flutter_l10n_config(parent) else {
        return false;
    };
    if parent != config.output_dir {
        return false;
    }
    if file_name == config.output_file {
        return true;
    }
    let Some(stem) = config.output_file.strip_suffix(".dart") else {
        return false;
    };
    file_name
        .strip_prefix(&format!("{stem}_"))
        .is_some_and(is_flutter_gen_l10n_locale_file)
}

fn flutter_l10n_config(directory: &Path) -> Option<FlutterL10nConfig> {
    for project_root in directory.ancestors() {
        let config_path = project_root.join("l10n.yaml");
        let Ok(metadata) = fs::metadata(&config_path) else {
            continue;
        };
        let modified = metadata.modified().ok();
        let Ok(mut cache) = FLUTTER_L10N_CONFIGS.lock() else {
            return None;
        };
        if let Some(cached) = cache.get(&config_path)
            && cached.length == metadata.len()
            && cached.modified == modified
        {
            return cached.config.clone();
        }
        let config = read_flutter_l10n_config(project_root, &config_path);
        cache.insert(
            config_path,
            CachedFlutterL10nConfig {
                length: metadata.len(),
                modified,
                config: config.clone(),
            },
        );
        return config;
    }
    None
}

fn read_flutter_l10n_config(project_root: &Path, config_path: &Path) -> Option<FlutterL10nConfig> {
    let source = fs::read_to_string(config_path).ok()?;
    let config = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&source).ok()?;
    let output_file = config
        .get("output-localization-file")
        .and_then(serde_yaml_ng::Value::as_str)
        .unwrap_or("app_localizations.dart")
        .to_owned();
    let output_dir = config
        .get("output-dir")
        .and_then(serde_yaml_ng::Value::as_str)
        .or_else(|| config.get("arb-dir").and_then(serde_yaml_ng::Value::as_str))
        .unwrap_or("lib/l10n");
    Some(FlutterL10nConfig {
        output_dir: project_root.join(output_dir),
        output_file,
    })
}

fn is_flutter_gen_l10n_locale_file(name: &str) -> bool {
    let Some(locale) = name.strip_suffix(".dart") else {
        return false;
    };
    let parts = locale.split('_').collect::<Vec<_>>();
    match parts.as_slice() {
        [language] => is_language_subtag(language),
        [language, script_or_region] => {
            is_language_subtag(language)
                && (is_script_subtag(script_or_region) || is_region_subtag(script_or_region))
        }
        [language, script, region] => {
            is_language_subtag(language) && is_script_subtag(script) && is_region_subtag(region)
        }
        _ => false,
    }
}

fn is_language_subtag(part: &str) -> bool {
    matches!(part.len(), 2 | 3) && part.bytes().all(|byte| byte.is_ascii_lowercase())
}

fn is_script_subtag(part: &str) -> bool {
    part.len() == 4
        && part.bytes().enumerate().all(|(index, byte)| match index {
            0 => byte.is_ascii_uppercase(),
            _ => byte.is_ascii_lowercase(),
        })
}

fn is_region_subtag(part: &str) -> bool {
    (part.len() == 2 && part.bytes().all(|byte| byte.is_ascii_uppercase()))
        || (part.len() == 3 && part.bytes().all(|byte| byte.is_ascii_digit()))
}

#[must_use]
pub(crate) fn is_flutterfire_options_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|file_name| file_name == "firebase_options.dart")
        && path
            .parent()
            .and_then(|parent| parent.file_name())
            .is_some_and(|parent| parent == "lib")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;

    #[test]
    fn recognizes_common_dart_generated_companions() {
        for file_name in [
            "model.g.dart",
            "model.freezed.dart",
            "l10n.gen.dart",
            "routes.gr.dart",
            "service.mocks.dart",
            "messages.pb.dart",
            "messages.pbenum.dart",
            "messages.pbjson.dart",
            "messages.pbgrpc.dart",
        ] {
            assert!(is_generated_dart_file_name(file_name), "{file_name}");
            assert!(is_generated_dart_path(
                Path::new("lib").join(file_name).as_path()
            ));
        }
    }

    #[test]
    fn rejects_regular_dart_sources() {
        for file_name in ["main.dart", "mock_service.dart", "generated.dart"] {
            assert!(!is_generated_dart_file_name(file_name), "{file_name}");
        }
    }

    #[test]
    fn recognizes_flutter_generated_entry_files() {
        for path in [
            Path::new("lib/l10n/app_localizations.dart"),
            Path::new("lib/l10n/app_localizations_en.dart"),
            Path::new("lib/l10n/app_localizations_en_US.dart"),
            Path::new("lib/l10n/app_localizations_zh_Hant.dart"),
            Path::new("lib/l10n/app_localizations_sr_Latn_RS.dart"),
            Path::new("lib/l10n/app_localizations_es_419.dart"),
            Path::new("lib/gen_l10n/app_localizations_es.dart"),
            Path::new("test/drift/generated/schema_v31.dart"),
            Path::new("lib/firebase_options.dart"),
            Path::new("packages/foo/lib/firebase_options.dart"),
        ] {
            assert!(is_generated_dart_path(path), "{}", path.display());
        }
    }

    #[test]
    fn rejects_handwritten_app_localizations_sources() {
        for path in [
            Path::new("lib/l10n/app_localizations_repository.dart"),
            Path::new("lib/l10n/app_localizations_en_US_helper.dart"),
            Path::new("lib/l10n/app_localizations_.dart"),
            Path::new("lib/l10n/app_localizations_en-us.dart"),
            Path::new("test/drift/schema_v31.dart"),
            Path::new("test/drift/generated/schema_version.dart"),
            Path::new("lib/src/firebase_options.dart"),
            Path::new("packages/foo/lib/config/firebase_options.dart"),
        ] {
            assert!(!is_generated_dart_path(path), "{}", path.display());
        }
    }

    #[test]
    fn recognizes_configured_flutter_generated_localizations()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = tempfile::tempdir()?;
        fs::create_dir_all(fixture.path().join("lib/l10n"))?;
        fs::write(
            fixture.path().join("l10n.yaml"),
            "arb-dir: lib/l10n\noutput-localization-file: l10n.dart\n",
        )?;

        for file_name in ["l10n.dart", "l10n_en.dart", "l10n_zh_Hant.dart"] {
            assert!(is_generated_dart_path(
                &fixture.path().join("lib/l10n").join(file_name)
            ));
        }
        assert!(!is_generated_dart_path(
            &fixture.path().join("lib/l10n/l10n_repository.dart")
        ));

        fs::write(
            fixture.path().join("l10n.yaml"),
            "arb-dir: lib/l10n\noutput-localization-file: generated_localizations.dart\n",
        )?;
        assert!(is_generated_dart_path(
            &fixture
                .path()
                .join("lib/l10n/generated_localizations_en.dart")
        ));
        assert!(!is_generated_dart_path(
            &fixture.path().join("lib/l10n/l10n_en.dart")
        ));
        Ok(())
    }
}
