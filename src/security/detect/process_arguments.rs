use tree_sitter::Node;

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
