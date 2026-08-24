use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn package_used_in_tooling(package_root: &Path, dependency: &str) -> bool {
    pubspec_has_top_level_key(package_root, dependency)
        || known_tooling_convention(package_root, dependency)
        || tooling_files(package_root).into_iter().any(|path| {
            fs::read_to_string(path)
                .is_ok_and(|source| source_mentions_dependency(&source, dependency))
        })
}

fn pubspec_has_top_level_key(package_root: &Path, key: &str) -> bool {
    let path = package_root.join("pubspec.yaml");
    fs::read_to_string(path)
        .ok()
        .and_then(|source| serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&source).ok())
        .is_some_and(|pubspec| pubspec.get(key).is_some())
}

fn known_tooling_convention(package_root: &Path, dependency: &str) -> bool {
    match dependency {
        "flutter_gen_runner" => pubspec_has_top_level_key(package_root, "flutter_gen"),
        "test" => contains_runnable_test(&package_root.join("test")),
        _ => false,
    }
}

fn contains_runnable_test(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        entry.file_type().is_ok_and(|file_type| {
            file_type.is_file() && is_runnable_test_file(&path)
                || file_type.is_dir() && contains_runnable_test(&path)
        })
    })
}

fn is_runnable_test_file(path: &Path) -> bool {
    if path.extension().is_none_or(|extension| extension != "dart") {
        return false;
    }
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with("_test.dart"))
    {
        return true;
    }
    crate::extract_dart_file(path)
        .is_ok_and(|file| crate::extract::has_top_level_function(&file, "main"))
}

fn tooling_files(package_root: &Path) -> Vec<PathBuf> {
    let mut paths = [
        "analysis_options.yaml",
        "build.yaml",
        "flutter_launcher_icons.yaml",
        "flutter_native_splash.yaml",
        "melos.yaml",
        "codemagic.yaml",
        "Makefile",
    ]
    .into_iter()
    .map(|path| package_root.join(path))
    .filter(|path| path.is_file())
    .collect::<Vec<_>>();

    collect_matching_files(&package_root.join(".github/workflows"), &mut paths);
    collect_matching_files(&package_root.join("tool"), &mut paths);
    paths
}

fn collect_matching_files(dir: &Path, paths: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && is_tooling_file(&path) {
            paths.push(path);
        }
    }
}

fn is_tooling_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("yaml" | "yml" | "sh" | "bash" | "zsh")
    ) || path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "Makefile")
}

fn source_mentions_dependency(source: &str, dependency: &str) -> bool {
    source
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'))
        .any(|token| token == dependency)
        || source.contains(&format!("{dependency}|"))
}

#[cfg(test)]
mod tests {
    use super::source_mentions_dependency;

    #[test]
    fn matches_dependency_tokens_without_substrings() {
        assert!(source_mentions_dependency(
            "builders:\n  build_runner|combining_builder:\n",
            "build_runner"
        ));
        assert!(source_mentions_dependency(
            "plugins:\n  - custom_lint\n",
            "custom_lint"
        ));
        assert!(!source_mentions_dependency(
            "some_build_runner_extra",
            "build_runner"
        ));
    }
}
