mod common;

use std::ffi::OsStr;

#[test]
fn git_commands_remove_repository_local_environment() {
    let command = common::isolated_git_command();
    let removed = command
        .get_envs()
        .filter_map(|(key, value)| value.is_none().then_some(key))
        .collect::<Vec<_>>();

    for variable in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_COMMON_DIR",
        "GIT_CONFIG_COUNT",
    ] {
        assert!(removed.contains(&OsStr::new(variable)), "kept {variable}");
    }
}
