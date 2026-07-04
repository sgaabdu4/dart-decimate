use std::path::Path;

pub(crate) const GENERATED_DART_SUFFIXES: &[&str] = &[
    ".g.dart",
    ".freezed.dart",
    ".gen.dart",
    ".gr.dart",
    ".mocks.dart",
];

#[must_use]
pub(crate) fn is_generated_dart_path(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    is_generated_dart_file_name(file_name)
        || is_flutter_gen_l10n_path(path, file_name)
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
            Path::new("lib/src/firebase_options.dart"),
            Path::new("packages/foo/lib/config/firebase_options.dart"),
        ] {
            assert!(!is_generated_dart_path(path), "{}", path.display());
        }
    }
}
