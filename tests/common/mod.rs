use std::ffi::OsStr;
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

pub fn isolated_command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
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

pub fn isolated_git_command() -> Command {
    isolated_command("git")
}

fn is_scoped_config_variable(variable: &OsStr) -> bool {
    let variable = variable.as_encoded_bytes();
    variable.starts_with(b"GIT_CONFIG_KEY_") || variable.starts_with(b"GIT_CONFIG_VALUE_")
}
