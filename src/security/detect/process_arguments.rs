use tree_sitter::Node;

use super::{
    command_interpreter_uses_c_flag, posix_shell_flag_invokes_command, read_string,
    resolved_fixed_literal_value, resolved_list_arguments,
};

const COMMAND_WRAPPERS: [&str; 19] = [
    "ssh", "sudo", "env", "xargs", "kubectl", "docker", "podman", "adb", "find", "nice", "timeout",
    "nohup", "doas", "su", "runuser", "chroot", "script", "watch", "parallel",
];

const COMMAND_RUNNING_OPTIONS: [&str; 9] = [
    "proxycommand",
    "upload-pack",
    "receive-pack",
    "rsh",
    "to-command",
    "exec",
    "command",
    "ssh-command",
    "sshcommand",
];

fn option_runs_a_command(literal: &str) -> bool {
    let lower = literal.to_ascii_lowercase();
    let option = lower.split('=').next().unwrap_or_default();
    let name = option.trim_start_matches('-');
    COMMAND_RUNNING_OPTIONS
        .iter()
        .any(|listed| name == *listed || name.strip_prefix('o') == Some(*listed))
        || option.starts_with("core.")
        || lower.contains("core.sshcommand")
}

pub(super) fn is_command_wrapper(executable: &str) -> bool {
    let name = executable_name(executable);
    COMMAND_WRAPPERS.contains(&name.as_str())
}

fn executable_name(executable: &str) -> String {
    let name = executable
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(executable)
        .to_ascii_lowercase();
    name.strip_suffix(".exe")
        .map_or(name.clone(), str::to_owned)
}

fn is_shell_element(root: Node<'_>, source: &str, index: usize, element: &str) -> bool {
    let Some(value) = resolved_fixed_literal_value(root, source, index, element) else {
        return false;
    };
    let name = executable_name(&value);
    let flag = value.to_ascii_lowercase();
    command_interpreter_uses_c_flag(&name)
        || matches!(name.as_str(), "cmd" | "powershell" | "pwsh")
        || posix_shell_flag_invokes_command(&flag)
        || matches!(flag.as_str(), "/c" | "/k" | "-command" | "-encodedcommand")
}

pub(super) fn placed_argument_list(
    root: Node<'_>,
    source: &str,
    index: usize,
    arguments: &str,
    trusted_executable: bool,
) -> bool {
    let Some(elements) = resolved_list_arguments(root, source, index, arguments) else {
        return false;
    };
    placed_elements(root, source, index, &elements, trusted_executable)
}

pub(super) fn placed_command_list(
    root: Node<'_>,
    source: &str,
    index: usize,
    command: &str,
) -> bool {
    let Some(elements) = resolved_list_arguments(root, source, index, command) else {
        return false;
    };
    let Some((executable, arguments)) = elements.split_first() else {
        return false;
    };
    let trusted = resolved_fixed_literal_value(root, source, index, executable)
        .is_some_and(|value| !is_command_wrapper(&value));
    placed_elements(root, source, index, arguments, trusted)
}

fn flatten(
    root: Node<'_>,
    source: &str,
    index: usize,
    elements: &[String],
    depth: usize,
) -> Option<Vec<String>> {
    let mut flat = Vec::new();
    for element in elements {
        let element = element.trim();
        if element.is_empty() {
            continue;
        }
        if let Some(spread) = element.strip_prefix("...") {
            if depth >= 4 {
                return None;
            }
            let inner =
                resolved_list_arguments(root, source, index, spread.trim_start_matches('?'))?;
            flat.extend(flatten(root, source, index, &inner, depth + 1)?);
        } else {
            flat.push(element.to_owned());
        }
    }
    Some(flat)
}

fn placed_elements(
    root: Node<'_>,
    source: &str,
    index: usize,
    elements: &[String],
    trusted_executable: bool,
) -> bool {
    let Some(flat) = flatten(root, source, index, elements, 0) else {
        return false;
    };
    let separator_trusted = trusted_executable
        && !flat
            .iter()
            .any(|element| is_shell_element(root, source, index, element));
    let mut after_separator = false;
    flat.iter().all(|element| {
        if let Some(word) = resolved_fixed_literal_value(root, source, index, element) {
            after_separator |= separator_trusted && word == "--";
            return true;
        }
        after_separator || attached_option_value(element)
    })
}

fn is_option_word(word: &str) -> bool {
    let name = word.trim_start_matches('-');
    word.starts_with('-')
        && word != "--"
        && !name.is_empty()
        && name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn attached_option_value(element: &str) -> bool {
    if !matches!(element.as_bytes().first(), Some(b'\'' | b'"')) {
        return false;
    }
    let Some((inner, _, end)) = read_string(element, 0) else {
        return false;
    };
    end == element.len()
        && !option_runs_a_command(&inner)
        && inner
            .split_once('=')
            .is_some_and(|(option, _)| is_option_word(option))
}

pub(super) fn name_is_reassigned(root: Node<'_>, source: &str, index: usize, name: &str) -> bool {
    function_body_of(root, index).is_none_or(|body| scope_assigns_name(body, source, name))
}

fn function_body_of(root: Node<'_>, index: usize) -> Option<Node<'_>> {
    let mut node = root.descendant_for_byte_range(index, index.saturating_add(1))?;
    let mut body = None;
    while let Some(parent) = node.parent() {
        match parent.kind() {
            "class_body" => break,
            "function_body" | "function_expression_body" => body = Some(parent),
            _ => {}
        }
        node = parent;
    }
    body
}

pub(super) fn list_is_proven_fixed(root: Node<'_>, source: &str, index: usize, name: &str) -> bool {
    if name_is_const(root, source, name) {
        return true;
    }
    let Some(body) = function_body_of(root, index) else {
        return false;
    };
    let text = &source[body.start_byte()..body.end_byte()];
    let uses = super::segment_indices(text, name)
        .into_iter()
        .filter(|start| {
            let before = text[..*start].trim_end();
            !before.ends_with('.') || before.ends_with("...")
        })
        .count();
    uses == 2 && !scope_assigns_name(body, source, name)
}

fn name_is_const(root: Node<'_>, source: &str, name: &str) -> bool {
    let mut declarations = 0;
    let mut constant = false;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if matches!(
            node.kind(),
            "initialized_identifier"
                | "initialized_variable_definition"
                | "static_final_declaration"
        ) && node
            .child_by_field_name("name")
            .and_then(|binding| binding.utf8_text(source.as_bytes()).ok())
            == Some(name)
        {
            declarations += 1;
            constant = node.parent().is_some_and(|parent| {
                let mut cursor = parent.walk();
                let direct = parent
                    .children(&mut cursor)
                    .any(|child| child.kind() == "const");
                direct
                    || parent.parent().is_some_and(|outer| {
                        let mut cursor = outer.walk();
                        outer
                            .children(&mut cursor)
                            .any(|child| child.kind() == "const")
                    })
            });
        }
        let mut cursor = node.walk();
        stack.extend(node.named_children(&mut cursor));
    }
    declarations == 1 && constant
}

fn scope_assigns_name(scope: Node<'_>, source: &str, name: &str) -> bool {
    let mut stack = vec![scope];
    while let Some(node) = stack.pop() {
        if node.kind() == "assignment_expression"
            && node
                .child_by_field_name("left")
                .and_then(|left| left.utf8_text(source.as_bytes()).ok())
                .is_some_and(|left| assigns_name(left.trim(), name))
        {
            return true;
        }
        let mut cursor = node.walk();
        stack.extend(node.named_children(&mut cursor));
    }
    false
}

fn assigns_name(left: &str, name: &str) -> bool {
    left == name
        || left
            .strip_prefix(name)
            .is_some_and(|rest| rest.starts_with('['))
}
