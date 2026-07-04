use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn package_used_in_tooling(package_root: &Path, dependency: &str) -> bool {
    pubspec_tooling_section(package_root, dependency)
        || tooling_files(package_root).into_iter().any(|path| {
            fs::read_to_string(path)
                .ok()
                .is_some_and(|source| source_mentions_dependency(&source, dependency))
        })
}

fn pubspec_tooling_section(package_root: &Path, dependency: &str) -> bool {
    let path = package_root.join("pubspec.yaml");
    fs::read_to_string(path).ok().is_some_and(|source| {
        source.lines().any(|line| {
            let trimmed = line.trim_end();
            !trimmed.starts_with(' ')
                && !trimmed.starts_with('\t')
                && trimmed == format!("{dependency}:")
        })
    })
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
