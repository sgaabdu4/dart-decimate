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
            .is_some_and(|rest| {
                Path::new(rest)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("dart"))
            }))
        && path.components().any(|component| {
            matches!(
                component.as_os_str().to_str(),
                Some("l10n" | "gen_l10n" | "generated")
            )
        })
}

#[must_use]
pub(crate) fn is_flutterfire_options_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|file_name| file_name == "firebase_options.dart")
        && path
            .components()
            .any(|component| component.as_os_str() == "lib")
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
            Path::new("lib/gen_l10n/app_localizations_es.dart"),
            Path::new("lib/firebase_options.dart"),
        ] {
            assert!(is_generated_dart_path(path), "{}", path.display());
        }
    }
}
