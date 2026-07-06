use std::collections::BTreeSet;

use tree_sitter::Node;

use super::{reads::scoped_header_binding_exists, simple_type_name};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObjectPatternHelper {
    name: HelperName,
    parameter: HelperParameter,
    fields: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HelperName {
    name: String,
    qualified_names: BTreeSet<String>,
    method_owner: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HelperParameter {
    name: String,
    positional_index: Option<usize>,
}

pub(super) fn object_pattern_field_reads_for_widget(
    root: Node<'_>,
    widget_class: &str,
    widget_body: Node<'_>,
    state_bodies: Option<&Vec<Node<'_>>>,
    source: &str,
) -> BTreeSet<String> {
    let mut reads =
        object_pattern_field_reads_in_body(widget_body, widget_class, &["this"], source);
    if let Some(state_bodies) = state_bodies {
        for state_body in state_bodies {
            reads.extend(object_pattern_field_reads_in_body(
                *state_body,
                widget_class,
                &["widget", "oldWidget"],
                source,
            ));
        }
    }

    let helpers = object_pattern_helpers(root, widget_class, source);
    for helper in helpers {
        let widget_call = body_calls_helper_with_roots(
            widget_body,
            &helper.name,
            &helper.parameter,
            &["this"],
            source,
        );
        if widget_call
            || state_bodies.is_some_and(|state_bodies| {
                state_bodies.iter().any(|state_body| {
                    body_calls_helper_with_roots(
                        *state_body,
                        &helper.name,
                        &helper.parameter,
                        &["widget", "oldWidget"],
                        source,
                    )
                })
            })
        {
            reads.extend(helper.fields);
        }
    }
    reads
}

fn object_pattern_helpers(
    root: Node<'_>,
    widget_class: &str,
    source: &str,
) -> Vec<ObjectPatternHelper> {
    let mut declarations = Vec::new();
    collect_nodes_in(
        root,
        &["function_declaration", "method_declaration"],
        &mut declarations,
    );

    let mut helpers = Vec::new();
    for declaration in declarations {
        let Some(signature) = helper_signature(declaration) else {
            continue;
        };
        let Some(name) = helper_name(declaration, signature, source) else {
            continue;
        };
        let body = helper_body(declaration).unwrap_or(declaration);
        for parameter in helper_widget_parameters(signature, widget_class, source) {
            let fields =
                object_pattern_field_reads_in_body(body, widget_class, &[&parameter.name], source);
            if fields.is_empty() {
                continue;
            }
            helpers.push(ObjectPatternHelper {
                name: name.clone(),
                parameter,
                fields,
            });
        }
    }
    helpers
}

fn helper_signature(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("signature")
        .or_else(|| direct_named_child(node, "function_signature"))
}

fn helper_body(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("body")
        .or_else(|| direct_named_child(node, "function_body"))
}

fn helper_name(declaration: Node<'_>, signature: Node<'_>, source: &str) -> Option<HelperName> {
    let name = field_text(signature, "name", source)
        .or_else(|| identifier_before_parameters(signature, source))?;
    let method_owner = (declaration.kind() == "method_declaration")
        .then(|| owner_class_name(declaration, source))
        .flatten();
    let mut qualified_names = BTreeSet::new();
    if let Some(owner) = &method_owner {
        qualified_names.insert(format!("{owner}.{name}"));
    }
    Some(HelperName {
        name,
        qualified_names,
        method_owner,
    })
}

fn owner_class_name(node: Node<'_>, source: &str) -> Option<String> {
    let mut parent = node.parent();
    while let Some(ancestor) = parent {
        if ancestor.kind() == "class_declaration" {
            return field_text(ancestor, "name", source);
        }
        parent = ancestor.parent();
    }
    None
}

fn helper_widget_parameters(
    signature: Node<'_>,
    widget_class: &str,
    source: &str,
) -> Vec<HelperParameter> {
    let parameters = parameter_list(signature).unwrap_or(signature);
    let formal_parameters = direct_formal_parameters(parameters);
    let mut positional_index = 0usize;
    let mut widget_parameters = Vec::new();
    for param in formal_parameters {
        let is_named = is_named_parameter(param, source);
        let current_positional_index = (!is_named).then_some(positional_index);
        if !is_named {
            positional_index += 1;
        }
        let Some(name) = formal_parameter_name(param, source) else {
            continue;
        };
        if formal_parameter_type(param, &name, source).as_deref() != Some(widget_class) {
            continue;
        }
        widget_parameters.push(HelperParameter {
            name,
            positional_index: current_positional_index,
        });
    }
    widget_parameters
}

fn direct_formal_parameters(parameters: Node<'_>) -> Vec<Node<'_>> {
    let mut formal_parameters = Vec::new();
    let mut cursor = parameters.walk();
    for candidate in parameters.named_children(&mut cursor) {
        if candidate.kind() == "optional_formal_parameters" {
            let mut optional_cursor = candidate.walk();
            formal_parameters.extend(candidate.named_children(&mut optional_cursor));
        } else {
            formal_parameters.push(candidate);
        }
    }
    formal_parameters
}

fn parameter_list(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("parameters")
        .and_then(formal_parameter_list)
        .or_else(|| own_parameter_list(node))
}

fn is_named_parameter(param: Node<'_>, source: &str) -> bool {
    let Some(parent) = param.parent() else {
        return false;
    };
    parent.kind() == "optional_formal_parameters"
        && parent
            .utf8_text(source.as_bytes())
            .ok()
            .is_some_and(|text| text.trim_start().starts_with('{'))
}

fn formal_parameter_name(param: Node<'_>, source: &str) -> Option<String> {
    param
        .child_by_field_name("name")
        .or_else(|| last_identifier_child(param, source))
        .and_then(|name| name.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn formal_parameter_type(param: Node<'_>, name: &str, source: &str) -> Option<String> {
    let text = param.utf8_text(source.as_bytes()).ok()?;
    let name_index = text.rfind(name)?;
    let before_name = text[..name_index].trim();
    let type_name = before_name
        .split_whitespace()
        .rfind(|part| !matches!(*part, "required" | "covariant" | "final" | "var"))?;
    Some(simple_type_name(type_name.trim_end_matches('?')))
}

pub(super) fn object_pattern_field_reads_in_body(
    body: Node<'_>,
    widget_class: &str,
    root_names: &[&str],
    source: &str,
) -> BTreeSet<String> {
    let mut fields = BTreeSet::new();
    visit_named(body, &mut |node| {
        if node.kind() != "object_pattern" {
            return;
        }
        if object_pattern_type_name(node, source).as_deref() != Some(widget_class) {
            return;
        }
        let mut roots = root_aliases_at(body, node, root_names, source);
        remove_roots_shadowed_by_enclosing_callable(body, node, &mut roots, source);
        if roots.is_empty() {
            return;
        }
        if !object_pattern_matches_roots(node, &roots, source) {
            return;
        }
        fields.extend(object_pattern_field_names(node, source));
    });
    fields
}

pub(super) fn remove_roots_shadowed_by_enclosing_callable(
    body: Node<'_>,
    site: Node<'_>,
    roots: &mut BTreeSet<String>,
    source: &str,
) {
    let owner = (body.kind() == "class_body")
        .then(|| outermost_callable_before_body(body, site))
        .flatten();
    let mut parent = site.parent();
    while let Some(ancestor) = parent {
        if same_node(ancestor, body) {
            return;
        }
        for name in callable_direct_parameter_bound_names(ancestor, roots, source) {
            if owner.is_some_and(|owner| same_node(owner, ancestor))
                && owner_parameter_preserves_root(ancestor, &name, source)
            {
                continue;
            }
            roots.remove(&name);
        }
        parent = ancestor.parent();
    }
}

fn owner_parameter_preserves_root(owner: Node<'_>, name: &str, source: &str) -> bool {
    name == "oldWidget"
        && owner.kind() == "method_declaration"
        && callable_name(owner, source).as_deref() == Some("didUpdateWidget")
}

fn callable_name(node: Node<'_>, source: &str) -> Option<String> {
    let signature = helper_signature(node).unwrap_or(node);
    field_text(signature, "name", source)
        .or_else(|| identifier_before_parameters(signature, source))
}

fn outermost_callable_before_body<'tree>(
    body: Node<'tree>,
    site: Node<'tree>,
) -> Option<Node<'tree>> {
    let mut owner = None;
    let mut parent = site.parent();
    while let Some(ancestor) = parent {
        if same_node(ancestor, body) {
            return owner;
        }
        if is_callable_node(ancestor.kind()) {
            owner = Some(ancestor);
        }
        parent = ancestor.parent();
    }
    owner
}

pub(super) fn root_aliases_at(
    body: Node<'_>,
    site: Node<'_>,
    root_names: &[&str],
    source: &str,
) -> BTreeSet<String> {
    let mut roots = root_names
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<BTreeSet<_>>();
    let mut steps = Vec::new();
    let mut path_child = site;
    let mut parent = site.parent();
    while let Some(scope) = parent {
        steps.push((scope, path_child));
        if same_node(scope, body) {
            break;
        }
        path_child = scope;
        parent = scope.parent();
    }
    let owner = (body.kind() == "class_body")
        .then(|| outermost_callable_before_body(body, site))
        .flatten();
    for (scope, child) in steps.into_iter().rev() {
        remove_roots_shadowed_by_callable_scope(scope, owner, &mut roots, source);
        remove_roots_shadowed_by_header_scope(scope, child, site.start_byte(), &mut roots, source);
        collect_prior_root_aliases(scope, child, &mut roots, source);
    }
    roots
}

fn remove_roots_shadowed_by_callable_scope(
    scope: Node<'_>,
    owner: Option<Node<'_>>,
    roots: &mut BTreeSet<String>,
    source: &str,
) {
    for name in callable_direct_parameter_bound_names(scope, roots, source) {
        if owner.is_some_and(|owner| same_node(owner, scope))
            && owner_parameter_preserves_root(scope, &name, source)
        {
            continue;
        }
        roots.remove(&name);
    }
}

fn remove_roots_shadowed_by_header_scope(
    scope: Node<'_>,
    path_child: Node<'_>,
    usage_start: usize,
    roots: &mut BTreeSet<String>,
    source: &str,
) {
    let shadowed = roots
        .iter()
        .filter(|root| scoped_header_binding_exists(scope, path_child, usage_start, root, source))
        .cloned()
        .collect::<Vec<_>>();
    for name in shadowed {
        roots.remove(&name);
    }
}

fn collect_prior_root_aliases(
    scope: Node<'_>,
    path_child: Node<'_>,
    roots: &mut BTreeSet<String>,
    source: &str,
) {
    let mut cursor = scope.walk();
    for sibling in scope.named_children(&mut cursor) {
        if same_node(sibling, path_child) || sibling.start_byte() >= path_child.start_byte() {
            break;
        }
        collect_root_aliases_from_lexical_declaration(sibling, roots, source);
    }
}

fn collect_root_aliases_from_lexical_declaration(
    node: Node<'_>,
    roots: &mut BTreeSet<String>,
    source: &str,
) {
    remove_roots_shadowed_by_lexical_sibling(node, roots, source);
    match node.kind() {
        "local_variable_declaration" | "pattern_variable_declaration" => {
            collect_root_aliases_from_declaration_shape(node, roots, source);
        }
        "expression_statement" | "statement" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if matches!(
                    child.kind(),
                    "local_variable_declaration" | "pattern_variable_declaration"
                ) {
                    collect_root_aliases_from_declaration_shape(child, roots, source);
                }
            }
        }
        _ => {}
    }
}

fn remove_roots_shadowed_by_lexical_sibling(
    node: Node<'_>,
    roots: &mut BTreeSet<String>,
    source: &str,
) {
    if node.kind() == "local_function_declaration"
        && let Some(name) = local_function_name(node, source)
    {
        roots.remove(&name);
    }
}

fn collect_root_aliases_from_declaration_shape(
    node: Node<'_>,
    roots: &mut BTreeSet<String>,
    source: &str,
) {
    let shadowed_roots = roots
        .iter()
        .filter(|root| {
            binding_name(node, source).as_deref() == Some(root.as_str())
                || declaration_binds_name(node, root, source)
        })
        .cloned()
        .collect::<Vec<_>>();
    for name in shadowed_roots {
        roots.remove(&name);
    }
    if let Some(alias) = local_alias_for_root(node, roots, source) {
        roots.insert(alias);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if matches!(
            child.kind(),
            "initialized_identifier"
                | "initialized_identifier_list"
                | "initialized_variable_definition"
                | "variable_declaration_list"
        ) {
            collect_root_aliases_from_declaration_shape(child, roots, source);
        }
    }
}

fn declaration_binds_name(node: Node<'_>, name: &str, source: &str) -> bool {
    if binding_name(node, source).as_deref() == Some(name) {
        return true;
    }
    if node.kind() == "pattern_variable_declaration" {
        return pattern_variable_declaration_binds_name(node, name, source);
    }
    if !matches!(
        node.kind(),
        "local_variable_declaration"
            | "initialized_identifier_list"
            | "initialized_variable_definition"
            | "initialized_identifier"
            | "variable_declaration_list"
    ) {
        return false;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| declaration_binds_name(child, name, source))
}

fn pattern_variable_declaration_binds_name(node: Node<'_>, name: &str, source: &str) -> bool {
    let equals_byte = source
        .get(node.start_byte()..node.end_byte())
        .and_then(|text| text.find('='))
        .map_or(node.end_byte(), |offset| node.start_byte() + offset);
    let mut cursor = node.walk();
    node.named_children(&mut cursor).any(|child| {
        child.end_byte() <= equals_byte
            && is_pattern_node(child)
            && pattern_binds_name(child, name, source)
    })
}

fn local_alias_for_root(node: Node<'_>, roots: &BTreeSet<String>, source: &str) -> Option<String> {
    if !matches!(
        node.kind(),
        "initialized_identifier" | "initialized_variable_definition"
    ) {
        return None;
    }
    let text = node.utf8_text(source.as_bytes()).ok()?;
    let (_, right) = text.split_once('=')?;
    if !expression_matches_roots(right, roots) {
        return None;
    }
    field_text(node, "name", source).or_else(|| identifier_before_equals(text))
}

fn object_pattern_matches_roots(pattern: Node<'_>, roots: &BTreeSet<String>, source: &str) -> bool {
    let mut parent = pattern.parent();
    while let Some(ancestor) = parent {
        match ancestor.kind() {
            "pattern_assignment" | "pattern_variable_declaration" => {
                return pattern_variable_expression_matches(pattern, ancestor, roots, source);
            }
            "if_element" | "if_statement" => {
                return if_case_expression_matches(pattern, ancestor, roots, source);
            }
            "switch_expression" | "switch_statement" => {
                return switch_expression_matches(pattern, ancestor, roots, source);
            }
            "class_body" | "class_declaration" => return false,
            _ => {}
        }
        parent = ancestor.parent();
    }
    false
}

fn pattern_variable_expression_matches(
    pattern: Node<'_>,
    owner: Node<'_>,
    roots: &BTreeSet<String>,
    source: &str,
) -> bool {
    pattern_variable_expression_for_pattern(pattern, owner, source)
        .is_some_and(|expression| expression_matches_roots(&expression, roots))
}

fn pattern_variable_expression_for_pattern(
    pattern: Node<'_>,
    owner: Node<'_>,
    source: &str,
) -> Option<String> {
    let owner_text = source.get(owner.start_byte()..owner.end_byte())?;
    let assignment_byte = owner.start_byte() + find_top_level_assignment(owner_text)?;
    if pattern.end_byte() > assignment_byte {
        return None;
    }
    let right = source.get(assignment_byte + 1..owner.end_byte())?.trim();
    nested_expression_for_pattern(pattern, top_pattern_in_owner(pattern, owner), right, source)
}

fn top_pattern_in_owner<'tree>(pattern: Node<'tree>, owner: Node<'tree>) -> Node<'tree> {
    let mut top = pattern;
    let mut parent = pattern.parent();
    while let Some(ancestor) = parent {
        if same_node(ancestor, owner) {
            break;
        }
        if is_pattern_node(ancestor) {
            top = ancestor;
        }
        parent = ancestor.parent();
    }
    top
}

fn find_top_level_assignment(text: &str) -> Option<usize> {
    let mut angle_depth = 0usize;
    let mut grouping_depth = 0usize;
    for (index, character) in text.char_indices() {
        match character {
            '<' => angle_depth += 1,
            '>' => angle_depth = angle_depth.saturating_sub(1),
            '(' | '[' | '{' => grouping_depth += 1,
            ')' | ']' | '}' => grouping_depth = grouping_depth.saturating_sub(1),
            '=' if angle_depth == 0
                && grouping_depth == 0
                && !text[..index]
                    .chars()
                    .next_back()
                    .is_some_and(|previous| matches!(previous, '!' | '<' | '>' | '='))
                && !text[index + character.len_utf8()..]
                    .chars()
                    .next()
                    .is_some_and(|next| matches!(next, '=' | '>')) =>
            {
                return Some(index);
            }
            _ => {}
        }
    }
    None
}

fn nested_expression_for_pattern(
    pattern: Node<'_>,
    top_pattern: Node<'_>,
    expression: &str,
    source: &str,
) -> Option<String> {
    if same_node(pattern, top_pattern) {
        return Some(expression.to_owned());
    }
    let mut path = Vec::new();
    let mut child = pattern;
    let mut parent = pattern.parent();
    while let Some(container) = parent {
        if is_pattern_node(container) {
            path.push((container, child));
            child = container;
            if same_node(container, top_pattern) {
                break;
            }
        } else {
            child = container;
        }
        parent = container.parent();
    }
    if path
        .last()
        .is_none_or(|(container, _)| !same_node(*container, top_pattern))
    {
        return None;
    }

    let mut current = expression.to_owned();
    for (container, child) in path.into_iter().rev() {
        if same_node(container, child) || shorthand_pattern_wrapper(container.kind()) {
            continue;
        }
        current = child_expression_for_nested_pattern(container, child, &current, source)?;
    }
    Some(current)
}

fn child_expression_for_nested_pattern(
    container: Node<'_>,
    child: Node<'_>,
    expression: &str,
    source: &str,
) -> Option<String> {
    match container.kind() {
        "record_pattern" => {
            delimited_child_expression(container, child, expression, source, '(', ')')
        }
        "list_pattern" => {
            delimited_child_expression(container, child, expression, source, '[', ']')
        }
        _ => None,
    }
}

fn delimited_child_expression(
    container: Node<'_>,
    child: Node<'_>,
    expression: &str,
    source: &str,
    open: char,
    close: char,
) -> Option<String> {
    let container_text = source.get(container.start_byte()..container.end_byte())?;
    let (pattern_inner_offset, pattern_inner) =
        enclosed_inner_with_offset(container_text, open, close)?;
    let pattern_parts = split_top_level_ranges(pattern_inner);
    let child_start = child
        .start_byte()
        .checked_sub(container.start_byte() + pattern_inner_offset)?;
    let child_end = child
        .end_byte()
        .checked_sub(container.start_byte() + pattern_inner_offset)?;
    let pattern_index = pattern_parts
        .iter()
        .position(|(start, end)| child_start >= *start && child_end <= *end)?;
    let pattern_part = pattern_inner
        .get(pattern_parts[pattern_index].0..pattern_parts[pattern_index].1)?
        .trim();

    let (_, expression_inner) = enclosed_inner_with_offset(expression, open, close)?;
    let expression_parts = split_top_level_ranges(expression_inner);
    let expression_part = expression_part_matching_pattern(
        pattern_part,
        expression_inner,
        &expression_parts,
        pattern_index,
    )?;
    Some(
        expression_part
            .trim()
            .trim_end_matches(';')
            .trim()
            .to_owned(),
    )
}

fn expression_part_matching_pattern(
    pattern_part: &str,
    expression_inner: &str,
    expression_parts: &[(usize, usize)],
    pattern_index: usize,
) -> Option<String> {
    if let Some((pattern_label, _)) = top_level_label_value(pattern_part) {
        return expression_parts.iter().find_map(|(start, end)| {
            let part = expression_inner.get(*start..*end)?.trim();
            let (label, value) = top_level_label_value(part)?;
            (label == pattern_label).then(|| value.to_owned())
        });
    }
    let (start, end) = *expression_parts.get(pattern_index)?;
    let part = expression_inner.get(start..end)?.trim();
    top_level_label_value(part)
        .is_none()
        .then(|| part.to_owned())
}

fn enclosed_inner_with_offset(text: &str, open: char, close: char) -> Option<(usize, &str)> {
    let leading = text.len() - text.trim_start().len();
    let trimmed = text.trim();
    if !trimmed.starts_with(open) {
        return None;
    }
    let end = matching_enclosed_end(trimmed, open, close)?;
    if end + close.len_utf8() != trimmed.len() {
        return None;
    }
    Some((
        leading + open.len_utf8(),
        trimmed.get(open.len_utf8()..end)?,
    ))
}

fn split_top_level_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = 0usize;
    let mut angle_depth = 0usize;
    let mut grouping_depth = 0usize;
    for (index, character) in text.char_indices() {
        match character {
            '<' => angle_depth += 1,
            '>' => angle_depth = angle_depth.saturating_sub(1),
            '(' | '[' | '{' => grouping_depth += 1,
            ')' | ']' | '}' => grouping_depth = grouping_depth.saturating_sub(1),
            ',' if angle_depth == 0 && grouping_depth == 0 => {
                ranges.push((start, index));
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    if start < text.len() {
        ranges.push((start, text.len()));
    }
    ranges
}

fn top_level_label_value(text: &str) -> Option<(&str, &str)> {
    let mut angle_depth = 0usize;
    let mut grouping_depth = 0usize;
    for (index, character) in text.char_indices() {
        match character {
            '<' => angle_depth += 1,
            '>' => angle_depth = angle_depth.saturating_sub(1),
            '(' | '[' | '{' => grouping_depth += 1,
            ')' | ']' | '}' => grouping_depth = grouping_depth.saturating_sub(1),
            ':' if angle_depth == 0 && grouping_depth == 0 => {
                let label = text[..index].trim();
                if is_identifier_text(label) {
                    return Some((label, text[index + character.len_utf8()..].trim()));
                }
            }
            _ => {}
        }
    }
    None
}

fn matching_enclosed_end(text: &str, open: char, close: char) -> Option<usize> {
    let mut chars = text.char_indices();
    if chars.next().is_none_or(|(_, character)| character != open) {
        return None;
    }
    let mut depth = 0usize;
    for (index, character) in text.char_indices() {
        if character == open {
            depth += 1;
        } else if character == close {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn if_case_expression_matches(
    pattern: Node<'_>,
    owner: Node<'_>,
    roots: &BTreeSet<String>,
    source: &str,
) -> bool {
    let Some(before_pattern) = source.get(owner.start_byte()..pattern.start_byte()) else {
        return false;
    };
    let Some(case_index) = find_keyword(before_pattern, "case") else {
        return false;
    };
    let before_case = &before_pattern[..case_index];
    let Some(expression) = expression_after_if_keyword(before_case) else {
        return false;
    };
    case_pattern_expression_matches(pattern, owner, expression, roots, source)
}

fn expression_after_if_keyword<'source>(text: &'source str) -> Option<&'source str> {
    let keyword = find_keyword(text, "if")?;
    let after_keyword = text.get(keyword + "if".len()..)?.trim_start();
    Some(
        after_keyword
            .strip_prefix('(')
            .unwrap_or(after_keyword)
            .trim(),
    )
}

fn switch_expression_matches(
    pattern: Node<'_>,
    node: Node<'_>,
    roots: &BTreeSet<String>,
    source: &str,
) -> bool {
    let Some(text) = node.utf8_text(source.as_bytes()).ok() else {
        return false;
    };
    let Some(expression) = parenthesized_after_keyword(text, "switch") else {
        return false;
    };
    case_pattern_expression_matches(pattern, node, expression, roots, source)
}

fn case_pattern_expression_matches(
    pattern: Node<'_>,
    owner: Node<'_>,
    expression: &str,
    roots: &BTreeSet<String>,
    source: &str,
) -> bool {
    nested_expression_for_pattern(
        pattern,
        top_pattern_in_owner(pattern, owner),
        expression,
        source,
    )
    .is_some_and(|expression| expression_matches_roots(&expression, roots))
}

fn expression_matches_roots(expression: &str, roots: &BTreeSet<String>) -> bool {
    let expression = normalized_expression(expression);
    roots.contains(&expression)
}

fn normalized_expression(expression: &str) -> String {
    let mut expression = expression
        .split([';', ',', '}'])
        .next()
        .unwrap_or(expression)
        .trim()
        .trim_end_matches(';')
        .trim()
        .to_owned();
    while expression.starts_with('(') && expression.ends_with(')') {
        expression = expression[1..expression.len() - 1].trim().to_owned();
    }
    strip_whitespace(&expression)
}

fn parenthesized_after_keyword<'source>(text: &'source str, keyword: &str) -> Option<&'source str> {
    let keyword_index = text.find(keyword)?;
    let open = text[keyword_index + keyword.len()..].find('(')? + keyword_index + keyword.len();
    let mut depth = 0usize;
    for (offset, character) in text[open..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return text.get(open + 1..open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn body_calls_helper_with_roots(
    body: Node<'_>,
    helper_name: &HelperName,
    parameter: &HelperParameter,
    root_names: &[&str],
    source: &str,
) -> bool {
    let mut found = false;
    visit_named(body, &mut |node| {
        if found {
            return;
        }
        let Some(invocation_name) = invocation_name(node, source) else {
            return;
        };
        if !helper_name_matches(helper_name, &invocation_name, node, body, source) {
            return;
        }
        let mut roots = root_aliases_at(body, node, root_names, source);
        remove_roots_shadowed_by_enclosing_callable(body, node, &mut roots, source);
        if !roots.is_empty() && invocation_arguments_pass_roots(node, parameter, &roots, source) {
            found = true;
        }
    });
    found
}

fn helper_name_matches(
    helper_name: &HelperName,
    invocation_name: &str,
    invocation: Node<'_>,
    body: Node<'_>,
    source: &str,
) -> bool {
    if let Some(member) = this_or_super_member_name(invocation_name) {
        return member == helper_name.name
            && helper_name.method_owner.as_ref().is_some_and(|owner| {
                current_lexical_owner(invocation, source)
                    .as_deref()
                    .is_some_and(|current| current == owner)
            });
    }
    if invocation_name.contains('.') {
        return helper_name.qualified_names.contains(invocation_name);
    }
    if invocation_name != helper_name.name {
        return false;
    }
    if lexical_local_binding_before(body, invocation, &helper_name.name, source) {
        return false;
    }
    if lexical_header_binding_before(body, invocation, &helper_name.name, source) {
        return false;
    }
    if enclosing_callable_parameter_binds_name(invocation, body, &helper_name.name, source) {
        return false;
    }
    if helper_name.method_owner.is_none()
        && current_class_member_shadows_top_level(invocation, &helper_name.name, source)
    {
        return false;
    }
    helper_name.method_owner.as_ref().is_none_or(|owner| {
        current_lexical_owner(invocation, source)
            .as_deref()
            .is_some_and(|current| current == owner)
    })
}

fn lexical_local_binding_before(body: Node<'_>, site: Node<'_>, name: &str, source: &str) -> bool {
    let mut steps = Vec::new();
    let mut path_child = site;
    let mut parent = site.parent();
    while let Some(scope) = parent {
        steps.push((scope, path_child));
        if same_node(scope, body) {
            break;
        }
        path_child = scope;
        parent = scope.parent();
    }
    steps
        .into_iter()
        .rev()
        .any(|(scope, child)| prior_lexical_sibling_binds_name(scope, child, name, source))
}

fn lexical_header_binding_before(body: Node<'_>, site: Node<'_>, name: &str, source: &str) -> bool {
    let mut steps = Vec::new();
    let mut path_child = site;
    let mut parent = site.parent();
    while let Some(scope) = parent {
        steps.push((scope, path_child));
        if same_node(scope, body) {
            break;
        }
        path_child = scope;
        parent = scope.parent();
    }
    steps.into_iter().rev().any(|(scope, child)| {
        scoped_header_binding_exists(scope, child, site.start_byte(), name, source)
    })
}

fn enclosing_callable_parameter_binds_name(
    site: Node<'_>,
    body: Node<'_>,
    name: &str,
    source: &str,
) -> bool {
    let mut parent = site.parent();
    while let Some(ancestor) = parent {
        if same_node(ancestor, body) {
            return false;
        }
        if callable_direct_parameters_bind_name(ancestor, name, source) {
            return true;
        }
        parent = ancestor.parent();
    }
    false
}

fn prior_lexical_sibling_binds_name(
    scope: Node<'_>,
    path_child: Node<'_>,
    name: &str,
    source: &str,
) -> bool {
    let mut cursor = scope.walk();
    for sibling in scope.named_children(&mut cursor) {
        if same_node(sibling, path_child) || sibling.start_byte() >= path_child.start_byte() {
            break;
        }
        if lexical_sibling_binds_name(sibling, name, source) {
            return true;
        }
    }
    false
}

fn lexical_sibling_binds_name(node: Node<'_>, name: &str, source: &str) -> bool {
    if node.kind() == "local_function_declaration" {
        return local_function_name(node, source).as_deref() == Some(name);
    }
    matches!(
        node.kind(),
        "local_variable_declaration" | "pattern_variable_declaration"
    ) && node_contains_binding_name(node, name, source)
}

fn local_function_name(node: Node<'_>, source: &str) -> Option<String> {
    direct_named_child(node, "function_signature")
        .and_then(|signature| signature.child_by_field_name("name"))
        .or_else(|| node.child_by_field_name("name"))
        .and_then(|name| name.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn this_or_super_member_name(invocation_name: &str) -> Option<&str> {
    let member = invocation_name
        .strip_prefix("this.")
        .or_else(|| invocation_name.strip_prefix("super."))?;
    (!member.contains('.')).then_some(member)
}

fn callable_direct_parameter_bound_names(
    node: Node<'_>,
    names: &BTreeSet<String>,
    source: &str,
) -> BTreeSet<String> {
    if !is_callable_node(node.kind()) {
        return BTreeSet::new();
    }
    direct_parameter_lists(node)
        .into_iter()
        .flat_map(|parameters| parameter_list_directly_bound_names(parameters, names, source))
        .collect()
}

fn callable_direct_parameters_bind_name(node: Node<'_>, name: &str, source: &str) -> bool {
    if !is_callable_node(node.kind()) {
        return false;
    }
    direct_parameter_lists(node)
        .into_iter()
        .any(|parameters| parameter_list_directly_binds_name(parameters, name, source))
}

fn is_callable_node(kind: &str) -> bool {
    matches!(
        kind,
        "function_declaration"
            | "function_expression"
            | "local_function_declaration"
            | "method_declaration"
    )
}

fn direct_parameter_lists(node: Node<'_>) -> Vec<Node<'_>> {
    match node.kind() {
        "function_expression" => parameter_field_lists(node),
        "function_declaration" | "local_function_declaration" | "method_declaration" => {
            helper_signature(node)
                .and_then(parameter_list)
                .into_iter()
                .collect()
        }
        _ => Vec::new(),
    }
}

fn parameter_field_lists(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.children_by_field_name("parameters", &mut cursor)
        .filter_map(formal_parameter_list)
        .collect()
}

fn formal_parameter_list(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "formal_parameter_list" {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == "formal_parameter_list")
}

fn own_parameter_list(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "formal_parameter_list" {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(own_parameter_list)
}

fn parameter_list_directly_bound_names(
    parameters: Node<'_>,
    names: &BTreeSet<String>,
    source: &str,
) -> BTreeSet<String> {
    let mut bound = BTreeSet::new();
    let mut cursor = parameters.walk();
    for candidate in parameters.named_children(&mut cursor) {
        if let Some(name) = formal_parameter_name(candidate, source)
            && names.contains(&name)
        {
            bound.insert(name);
        }
        if candidate.kind() == "optional_formal_parameters" {
            bound.extend(optional_parameters_directly_bound_names(
                candidate, names, source,
            ));
        }
    }
    bound
}

fn optional_parameters_directly_bound_names(
    parameters: Node<'_>,
    names: &BTreeSet<String>,
    source: &str,
) -> BTreeSet<String> {
    let mut bound = BTreeSet::new();
    let mut cursor = parameters.walk();
    for candidate in parameters
        .named_children(&mut cursor)
        .filter(|candidate| candidate.kind() == "formal_parameter")
    {
        if let Some(name) = formal_parameter_name(candidate, source)
            && names.contains(&name)
        {
            bound.insert(name);
        }
    }
    bound
}

fn parameter_list_directly_binds_name(parameters: Node<'_>, name: &str, source: &str) -> bool {
    let mut cursor = parameters.walk();
    parameters.named_children(&mut cursor).any(|candidate| {
        if formal_parameter_name(candidate, source).as_deref() == Some(name) {
            return true;
        }
        candidate.kind() == "optional_formal_parameters"
            && optional_parameters_directly_bind_name(candidate, name, source)
    })
}

fn optional_parameters_directly_bind_name(parameters: Node<'_>, name: &str, source: &str) -> bool {
    let mut cursor = parameters.walk();
    parameters
        .named_children(&mut cursor)
        .filter(|candidate| candidate.kind() == "formal_parameter")
        .any(|candidate| formal_parameter_name(candidate, source).as_deref() == Some(name))
}

fn current_lexical_owner(node: Node<'_>, source: &str) -> Option<String> {
    current_lexical_class(node).and_then(|class| field_text(class, "name", source))
}

fn current_lexical_class(node: Node<'_>) -> Option<Node<'_>> {
    let mut parent = node.parent();
    while let Some(ancestor) = parent {
        if ancestor.kind() == "class_declaration" {
            return Some(ancestor);
        }
        parent = ancestor.parent();
    }
    None
}

fn current_class_member_shadows_top_level(invocation: Node<'_>, name: &str, source: &str) -> bool {
    current_lexical_class(invocation)
        .is_some_and(|class| class_or_inherited_member_shadows_top_level(class, name, source))
}

fn class_or_inherited_member_shadows_top_level(class: Node<'_>, name: &str, source: &str) -> bool {
    let root = root_node(class);
    let mut visited = BTreeSet::new();
    class_or_inherited_member_shadows_top_level_in(root, class, name, source, &mut visited)
}

fn class_or_inherited_member_shadows_top_level_in(
    root: Node<'_>,
    class: Node<'_>,
    name: &str,
    source: &str,
    visited: &mut BTreeSet<String>,
) -> bool {
    if class_declares_member(class, name, source) {
        return true;
    }
    for inherited in class_inherited_types(class, source) {
        if !visited.insert(inherited.name.clone()) {
            continue;
        }
        let Some(inherited_class) = find_class_like_declaration(root, &inherited.name, source)
        else {
            if unresolved_inherited_type_may_shadow(&inherited) {
                return true;
            }
            continue;
        };
        if class_or_inherited_member_shadows_top_level_in(
            root,
            inherited_class,
            name,
            source,
            visited,
        ) {
            return true;
        }
    }
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InheritedType {
    name: String,
    kind: InheritedTypeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InheritedTypeKind {
    Superclass,
    Mixin,
    Interface,
}

fn unresolved_inherited_type_may_shadow(inherited: &InheritedType) -> bool {
    inherited.kind != InheritedTypeKind::Superclass
        || !matches!(
            inherited.name.as_str(),
            "StatelessWidget"
                | "StatefulWidget"
                | "ConsumerWidget"
                | "ConsumerStatefulWidget"
                | "HookWidget"
                | "HookConsumerWidget"
                | "State"
                | "ConsumerState"
                | "Object"
        )
}

fn class_inherited_types(class: Node<'_>, source: &str) -> Vec<InheritedType> {
    let Some(body) = class.child_by_field_name("body") else {
        return Vec::new();
    };
    let Some(header) = source.get(class.start_byte()..body.start_byte()) else {
        return Vec::new();
    };
    let mut inherited = Vec::new();
    if let Some(superclass) = clause_after_keyword(header, "extends", &["with", "implements"])
        && let Some(name) = inherited_type_name(superclass)
    {
        inherited.push(InheritedType {
            name,
            kind: InheritedTypeKind::Superclass,
        });
    }
    if let Some(mixins) = clause_after_keyword(header, "with", &["implements"]) {
        inherited.extend(
            split_top_level_commas(mixins)
                .into_iter()
                .filter_map(inherited_type_name)
                .map(|name| InheritedType {
                    name,
                    kind: InheritedTypeKind::Mixin,
                }),
        );
    }
    if let Some(interfaces) = clause_after_keyword(header, "implements", &[]) {
        inherited.extend(
            split_top_level_commas(interfaces)
                .into_iter()
                .filter_map(inherited_type_name)
                .map(|name| InheritedType {
                    name,
                    kind: InheritedTypeKind::Interface,
                }),
        );
    }
    inherited
}

fn clause_after_keyword<'source>(
    text: &'source str,
    keyword: &str,
    stop_keywords: &[&str],
) -> Option<&'source str> {
    let start = find_keyword(text, keyword)? + keyword.len();
    let rest = text.get(start..)?;
    let end = stop_keywords
        .iter()
        .filter_map(|stop| find_keyword(rest, stop))
        .min()
        .unwrap_or(rest.len());
    Some(rest[..end].trim())
}

fn find_keyword(text: &str, keyword: &str) -> Option<usize> {
    let mut cursor = 0usize;
    while let Some(relative) = text[cursor..].find(keyword) {
        let start = cursor + relative;
        let end = start + keyword.len();
        let continues_before = text[..start]
            .chars()
            .next_back()
            .is_some_and(is_identifier_character);
        let continues_after = text[end..]
            .chars()
            .next()
            .is_some_and(is_identifier_character);
        if !continues_before && !continues_after {
            return Some(start);
        }
        cursor = end;
    }
    None
}

fn split_top_level_commas(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut angle_depth = 0usize;
    let mut grouping_depth = 0usize;
    for (index, character) in text.char_indices() {
        match character {
            '<' => angle_depth += 1,
            '>' => angle_depth = angle_depth.saturating_sub(1),
            '(' | '[' | '{' => grouping_depth += 1,
            ')' | ']' | '}' => grouping_depth = grouping_depth.saturating_sub(1),
            ',' if angle_depth == 0 && grouping_depth == 0 => {
                parts.push(text[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    let last = text[start..].trim();
    if !last.is_empty() {
        parts.push(last);
    }
    parts
}

fn inherited_type_name(text: &str) -> Option<String> {
    let mut name = String::new();
    for character in text.trim().chars() {
        if character == '<' {
            break;
        }
        if is_identifier_character(character) || character == '.' {
            name.push(character);
            continue;
        }
        if !name.is_empty() {
            break;
        }
    }
    let name = name.rsplit('.').next().unwrap_or(&name);
    is_identifier_text(name).then(|| name.to_owned())
}

fn find_class_like_declaration<'tree>(
    node: Node<'tree>,
    name: &str,
    source: &str,
) -> Option<Node<'tree>> {
    if matches!(node.kind(), "class_declaration" | "mixin_declaration")
        && field_text(node, "name", source).as_deref() == Some(name)
    {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(|child| find_class_like_declaration(child, name, source))
}

fn root_node(mut node: Node<'_>) -> Node<'_> {
    while let Some(parent) = node.parent() {
        node = parent;
    }
    node
}

fn class_declares_member(class: Node<'_>, name: &str, source: &str) -> bool {
    let Some(body) = class.child_by_field_name("body") else {
        return false;
    };
    let mut cursor = body.walk();
    body.named_children(&mut cursor)
        .any(|child| match child.kind() {
            "class_member" => class_member_declares_name(child, name, source),
            "method_declaration" => member_signature_name(child, source).as_deref() == Some(name),
            "declaration" => declaration_declares_member(child, name, source),
            _ => false,
        })
}

fn class_member_declares_name(member: Node<'_>, name: &str, source: &str) -> bool {
    let mut cursor = member.walk();
    member
        .named_children(&mut cursor)
        .any(|child| match child.kind() {
            "method_declaration" => member_signature_name(child, source).as_deref() == Some(name),
            "declaration" => declaration_declares_member(child, name, source),
            _ => false,
        })
}

fn declaration_declares_member(declaration: Node<'_>, name: &str, source: &str) -> bool {
    if member_signature_name(declaration, source).as_deref() == Some(name) {
        return true;
    }
    let mut names = Vec::new();
    collect_member_field_names(declaration, source, &mut names);
    names.iter().any(|candidate| candidate == name)
}

fn member_signature_name(node: Node<'_>, source: &str) -> Option<String> {
    if matches!(
        node.kind(),
        "function_signature" | "getter_signature" | "setter_signature"
    ) {
        return field_text(node, "name", source);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(|child| member_signature_name(child, source))
}

fn collect_member_field_names(node: Node<'_>, source: &str, names: &mut Vec<String>) {
    collect_named_fields(
        node,
        source,
        &["static_final_declaration", "initialized_identifier"],
        names,
    );
    if names.is_empty()
        && let Some(identifier_list) = find_first_named_descendant(node, "identifier_list")
    {
        collect_direct_identifier_children(identifier_list, source, names);
    }
}

fn collect_named_fields(
    node: Node<'_>,
    source: &str,
    owner_kinds: &[&str],
    names: &mut Vec<String>,
) {
    if owner_kinds.contains(&node.kind())
        && let Some(name) = field_text(node, "name", source)
    {
        names.push(name);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_named_fields(child, source, owner_kinds, names);
    }
}

fn find_first_named_descendant<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    if node.kind() == kind {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(|child| find_first_named_descendant(child, kind))
}

fn collect_direct_identifier_children(node: Node<'_>, source: &str, names: &mut Vec<String>) {
    let mut cursor = node.walk();
    names.extend(
        node.named_children(&mut cursor)
            .filter(|child| child.kind() == "identifier")
            .filter_map(|child| child.utf8_text(source.as_bytes()).ok())
            .map(str::to_owned),
    );
}

fn invocation_name(node: Node<'_>, source: &str) -> Option<String> {
    if !matches!(
        node.kind(),
        "call_expression"
            | "constructor_invocation"
            | "const_object_expression"
            | "new_expression"
            | "function_expression_invocation"
    ) {
        return None;
    }
    if let Some(name) = constructor_invocation_name(node, source) {
        return Some(name);
    }
    let arguments = argument_list(node)?;
    source
        .get(node.start_byte()..arguments.start_byte())
        .map(normalized_invocation_prefix)
}

fn argument_list(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("arguments")
        .or_else(|| direct_named_child(node, "arguments"))
        .or_else(|| direct_named_child(node, "argument_part"))
}

fn constructor_invocation_name(node: Node<'_>, source: &str) -> Option<String> {
    if !matches!(
        node.kind(),
        "constructor_invocation" | "const_object_expression" | "new_expression"
    ) {
        return None;
    }
    let type_name = node
        .child_by_field_name("type")?
        .utf8_text(source.as_bytes())
        .ok()
        .map(strip_whitespace)
        .map(|name| strip_type_arguments(&name))?;
    node.child_by_field_name("constructor")
        .and_then(|constructor| constructor.utf8_text(source.as_bytes()).ok())
        .map_or(Some(type_name.clone()), |constructor| {
            Some(format!("{type_name}.{constructor}"))
        })
}

fn normalized_invocation_prefix(prefix: &str) -> String {
    let prefix = strip_whitespace(
        prefix
            .trim()
            .strip_prefix("const ")
            .or_else(|| prefix.trim().strip_prefix("new "))
            .unwrap_or(prefix.trim()),
    );
    strip_type_arguments(&prefix)
}

fn invocation_arguments_pass_roots(
    node: Node<'_>,
    parameter: &HelperParameter,
    roots: &BTreeSet<String>,
    source: &str,
) -> bool {
    let Some(arguments) = argument_list(node) else {
        return false;
    };
    let mut positional_index = 0usize;
    let mut cursor = arguments.walk();
    for argument in arguments.named_children(&mut cursor) {
        if argument.kind() == "named_argument" {
            if named_argument_label(argument, source).as_deref() == Some(parameter.name.as_str())
                && named_argument_value(argument)
                    .is_some_and(|value| argument_matches_roots(value, roots, source))
            {
                return true;
            }
            continue;
        }
        if parameter.positional_index == Some(positional_index)
            && argument_matches_roots(argument, roots, source)
        {
            return true;
        }
        positional_index += 1;
    }
    argument_texts_pass_roots(arguments, parameter, roots, source)
}

fn argument_texts_pass_roots(
    arguments: Node<'_>,
    parameter: &HelperParameter,
    roots: &BTreeSet<String>,
    source: &str,
) -> bool {
    let Some(text) = arguments.utf8_text(source.as_bytes()).ok() else {
        return false;
    };
    let mut positional_index = 0usize;
    for argument in split_argument_texts(text) {
        if let Some((label, value)) = argument.split_once(':') {
            if label.trim() == parameter.name && expression_matches_roots(value, roots) {
                return true;
            }
            continue;
        }
        if parameter.positional_index == Some(positional_index)
            && expression_matches_roots(argument, roots)
        {
            return true;
        }
        positional_index += 1;
    }
    false
}

fn split_argument_texts(text: &str) -> Vec<&str> {
    let inner = text
        .trim()
        .strip_prefix('(')
        .and_then(|text| text.strip_suffix(')'))
        .unwrap_or(text)
        .trim();
    if inner.is_empty() {
        return Vec::new();
    }
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut depth = 0usize;
    for (index, character) in inner.char_indices() {
        match character {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(inner[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(inner[start..].trim());
    parts
}

fn named_argument_label(node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == "label")
        .and_then(|label| label.utf8_text(source.as_bytes()).ok())
        .map(str::trim)
        .and_then(|label| label.strip_suffix(':'))
        .map(str::to_owned)
}

fn named_argument_value(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() != "label")
}

fn argument_matches_roots(argument: Node<'_>, roots: &BTreeSet<String>, source: &str) -> bool {
    argument
        .utf8_text(source.as_bytes())
        .ok()
        .is_some_and(|text| expression_matches_roots(text, roots))
}

pub(super) fn pattern_binds_name(node: Node<'_>, name: &str, source: &str) -> bool {
    if node.kind() == "variable_pattern"
        && field_text(node, "name", source).as_deref() == Some(name)
    {
        return true;
    }
    if shorthand_pattern_name(node, source).as_deref() == Some(name) {
        return true;
    }
    if node.kind() == "object_pattern" {
        return object_pattern_binds_name(node, name, source);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| pattern_binds_name(child, name, source))
}

fn node_contains_binding_name(node: Node<'_>, name: &str, source: &str) -> bool {
    let mut found = false;
    visit_named(node, &mut |candidate| {
        if found {
            return;
        }
        if binding_name(candidate, source).as_deref() == Some(name)
            || pattern_binds_name(candidate, name, source)
        {
            found = true;
        }
    });
    found
}

fn binding_name(node: Node<'_>, source: &str) -> Option<String> {
    if !matches!(
        node.kind(),
        "formal_parameter"
            | "initialized_identifier"
            | "initialized_variable_definition"
            | "typed_identifier"
            | "variable_pattern"
    ) {
        return None;
    }
    node.child_by_field_name("name")
        .and_then(|name| name.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn object_pattern_binds_name(node: Node<'_>, name: &str, source: &str) -> bool {
    let mut saw_type = false;
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if !saw_type && is_type_child(child) {
            saw_type = true;
            continue;
        }
        if child.kind() == "type_arguments" {
            continue;
        }
        if child.kind() == "label" {
            continue;
        }
        if !is_pattern_node(child) {
            continue;
        }
        if shorthand_pattern_name(child, source).as_deref() == Some(name)
            || pattern_binds_name(child, name, source)
        {
            return true;
        }
    }
    false
}

fn object_pattern_type_name(node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| is_type_child(*child))
        .and_then(|child| child.utf8_text(source.as_bytes()).ok())
        .map(|text| simple_type_name(text.split('<').next().unwrap_or(text)))
}

fn object_pattern_field_names(node: Node<'_>, source: &str) -> BTreeSet<String> {
    let mut fields = BTreeSet::new();
    let mut saw_type = false;
    let mut pending_label = None;
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if !saw_type && is_type_child(child) {
            saw_type = true;
            continue;
        }
        if child.kind() == "type_arguments" {
            continue;
        }
        if child.kind() == "label" {
            pending_label = label_name(child, source);
            continue;
        }
        if !is_pattern_node(child) {
            continue;
        }
        if let Some(field) = pending_label.take() {
            fields.insert(field);
        } else if let Some(field) = shorthand_pattern_name(child, source) {
            fields.insert(field);
        }
    }
    fields
}

fn label_name(node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| is_identifier(*child))
        .and_then(|child| child.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn shorthand_pattern_name(node: Node<'_>, source: &str) -> Option<String> {
    if node.kind() == "variable_pattern" {
        return field_text(node, "name", source).filter(|name| name != "_");
    }
    let text = node.utf8_text(source.as_bytes()).ok()?.trim();
    if is_identifier_text(text) && text != "_" {
        return Some(text.to_owned());
    }
    if shorthand_pattern_wrapper(node.kind()) {
        let mut cursor = node.walk();
        return node
            .named_children(&mut cursor)
            .find_map(|child| shorthand_pattern_name(child, source));
    }
    None
}

fn shorthand_pattern_wrapper(kind: &str) -> bool {
    matches!(
        kind,
        "cast_pattern"
            | "null_assert_pattern"
            | "null_check_pattern"
            | "parenthesized_pattern"
            | "pattern"
    )
}

fn field_text(node: Node<'_>, field_name: &str, source: &str) -> Option<String> {
    node.child_by_field_name(field_name)
        .and_then(|field| field.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn is_type_child(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "type_identifier" | "identifier" | "qualified" | "type"
    )
}

fn is_pattern_node(node: Node<'_>) -> bool {
    node.kind().ends_with("_pattern") || node.kind() == "pattern"
}

fn is_identifier(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "identifier" | "identifier_dollar_escaped" | "type_identifier"
    )
}

fn is_identifier_text(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|character| {
            character == '_' || character == '$' || character.is_ascii_alphanumeric()
        })
}

fn is_identifier_character(character: char) -> bool {
    character == '_' || character == '$' || character.is_ascii_alphanumeric()
}

fn identifier_before_equals(text: &str) -> Option<String> {
    let before_equals = text.split_once('=')?.0;
    before_equals
        .split(|character: char| {
            !(character == '_' || character == '$' || character.is_ascii_alphanumeric())
        })
        .rfind(|part| !part.is_empty())
        .map(str::to_owned)
}

fn identifier_before_parameters(node: Node<'_>, source: &str) -> Option<String> {
    let text = node.utf8_text(source.as_bytes()).ok()?;
    let before_parameters = text.split_once('(')?.0;
    before_parameters
        .split(|character: char| {
            !(character == '_' || character == '$' || character.is_ascii_alphanumeric())
        })
        .rfind(|part| !part.is_empty())
        .map(str::to_owned)
}

fn last_identifier_child<'tree>(node: Node<'tree>, source: &str) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| {
            matches!(
                child.kind(),
                "identifier" | "identifier_dollar_escaped" | "type_identifier"
            ) && child.utf8_text(source.as_bytes()).ok() != Some("key")
        })
        .last()
}

fn collect_nodes_in<'tree>(node: Node<'tree>, kinds: &[&str], nodes: &mut Vec<Node<'tree>>) {
    if kinds.contains(&node.kind()) {
        nodes.push(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_nodes_in(child, kinds, nodes);
    }
}

fn direct_named_child<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == kind)
}

fn strip_whitespace(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn strip_type_arguments(text: &str) -> String {
    let mut stripped = String::new();
    let mut depth = 0usize;
    for character in text.chars() {
        match character {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => stripped.push(character),
            _ => {}
        }
    }
    stripped
}

fn visit_named(node: Node<'_>, visitor: &mut impl FnMut(Node<'_>)) {
    visitor(node);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visit_named(child, visitor);
    }
}

fn same_node(left: Node<'_>, right: Node<'_>) -> bool {
    left.kind() == right.kind()
        && left.start_byte() == right.start_byte()
        && left.end_byte() == right.end_byte()
}
