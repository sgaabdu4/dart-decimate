use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

const REPOSITORY_LOCAL_ENVIRONMENT: &[&str] = &[
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_OBJECT_DIRECTORY",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_GRAFT_FILE",
    "GIT_INDEX_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
    "GIT_PREFIX",
    "GIT_SHALLOW_FILE",
    "GIT_COMMON_DIR",
];

pub(crate) fn for_repository(root: &Path) -> Command {
    let mut command = Command::new("git");
    command.current_dir(root);
    for variable in REPOSITORY_LOCAL_ENVIRONMENT {
        command.env_remove(variable);
    }
    for (variable, _) in std::env::vars_os() {
        if is_scoped_config_variable(&variable) {
            command.env_remove(variable);
        }
    }
    command
}

fn is_scoped_config_variable(variable: &OsStr) -> bool {
    let variable = variable.as_encoded_bytes();
    variable.starts_with(b"GIT_CONFIG_KEY_") || variable.starts_with(b"GIT_CONFIG_VALUE_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_repository_local_environment() {
        let command = for_repository(Path::new("."));
        let removed = command
            .get_envs()
            .filter_map(|(key, value)| value.is_none().then_some(key))
            .collect::<Vec<_>>();

        for variable in REPOSITORY_LOCAL_ENVIRONMENT {
            assert!(removed.contains(&OsStr::new(variable)), "kept {variable}");
        }
    }
}
