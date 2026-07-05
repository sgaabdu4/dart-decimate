use std::collections::BTreeSet;

use tree_sitter::Node;

use super::simple_type_name;

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
    let mut formal_parameters = Vec::new();
    collect_nodes(parameters, "formal_parameter", &mut formal_parameters);
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

fn object_pattern_field_reads_in_body(
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
        let roots = root_aliases_at(body, node, root_names, source);
        if roots_shadowed_by_enclosing_callable(body, node, &roots, source) {
            return;
        }
        if !object_pattern_matches_roots(node, &roots, source) {
            return;
        }
        fields.extend(object_pattern_field_names(node, source));
    });
    fields
}

fn roots_shadowed_by_enclosing_callable(
    body: Node<'_>,
    site: Node<'_>,
    roots: &BTreeSet<String>,
    source: &str,
) -> bool {
    let owner = (body.kind() == "class_body")
        .then(|| outermost_callable_before_body(body, site))
        .flatten();
    let mut parent = site.parent();
    while let Some(ancestor) = parent {
        if same_node(ancestor, body) {
            return false;
        }
        if owner.is_none_or(|owner| !same_node(owner, ancestor))
            && callable_direct_parameters_bind_any(ancestor, roots, source)
        {
            return true;
        }
        parent = ancestor.parent();
    }
    false
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

fn root_aliases_at(
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
    for (scope, child) in steps.into_iter().rev() {
        collect_prior_root_aliases(scope, child, &mut roots, source);
    }
    roots
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
    if !matches!(node.kind(), "local_variable_declaration") {
        return;
    }
    collect_root_aliases_from_declaration_shape(node, roots, source);
}

fn collect_root_aliases_from_declaration_shape(
    node: Node<'_>,
    roots: &mut BTreeSet<String>,
    source: &str,
) {
    if let Some(name) = binding_name(node, source)
        && roots.contains(&name)
    {
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
                return switch_expression_matches(ancestor, roots, source);
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
    let Some(after_pattern) = source.get(pattern.end_byte()..owner.end_byte()) else {
        return false;
    };
    let Some((_, right)) = after_pattern.split_once('=') else {
        return false;
    };
    expression_matches_roots(right, roots)
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
    let Some(case_index) = before_pattern.rfind("case") else {
        return false;
    };
    let before_case = &before_pattern[..case_index];
    let expression = before_case
        .rfind('(')
        .map_or(before_case, |start| &before_case[start + 1..]);
    expression_matches_roots(expression, roots)
}

fn switch_expression_matches(node: Node<'_>, roots: &BTreeSet<String>, source: &str) -> bool {
    let Some(text) = node.utf8_text(source.as_bytes()).ok() else {
        return false;
    };
    let Some(expression) = parenthesized_after_keyword(text, "switch") else {
        return false;
    };
    expression_matches_roots(expression, roots)
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
        if helper_name_matches(helper_name, &invocation_name, node, body, source)
            && invocation_arguments_pass_roots(
                node,
                parameter,
                &root_aliases_at(body, node, root_names, source),
                source,
            )
        {
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
    if enclosing_callable_parameter_binds_name(invocation, body, &helper_name.name, source) {
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

fn callable_direct_parameters_bind_any(
    node: Node<'_>,
    names: &BTreeSet<String>,
    source: &str,
) -> bool {
    if !is_callable_node(node.kind()) {
        return false;
    }
    direct_parameter_lists(node)
        .into_iter()
        .any(|parameters| parameter_list_directly_binds_any(parameters, names, source))
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

fn parameter_list_directly_binds_any(
    parameters: Node<'_>,
    names: &BTreeSet<String>,
    source: &str,
) -> bool {
    let mut cursor = parameters.walk();
    parameters.named_children(&mut cursor).any(|candidate| {
        if formal_parameter_name(candidate, source).is_some_and(|name| names.contains(&name)) {
            return true;
        }
        candidate.kind() == "optional_formal_parameters"
            && optional_parameters_directly_bind_any(candidate, names, source)
    })
}

fn optional_parameters_directly_bind_any(
    parameters: Node<'_>,
    names: &BTreeSet<String>,
    source: &str,
) -> bool {
    let mut cursor = parameters.walk();
    parameters
        .named_children(&mut cursor)
        .filter(|candidate| candidate.kind() == "formal_parameter")
        .any(|candidate| {
            formal_parameter_name(candidate, source).is_some_and(|name| names.contains(&name))
        })
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
    let mut parent = node.parent();
    while let Some(ancestor) = parent {
        if ancestor.kind() == "class_declaration" {
            return field_text(ancestor, "name", source);
        }
        parent = ancestor.parent();
    }
    None
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
        .map(strip_whitespace)?;
    node.child_by_field_name("constructor")
        .and_then(|constructor| constructor.utf8_text(source.as_bytes()).ok())
        .map_or(Some(type_name.clone()), |constructor| {
            Some(format!("{type_name}.{constructor}"))
        })
}

fn normalized_invocation_prefix(prefix: &str) -> String {
    strip_whitespace(
        prefix
            .trim()
            .strip_prefix("const ")
            .or_else(|| prefix.trim().strip_prefix("new "))
            .unwrap_or(prefix.trim()),
    )
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
    let mut saw_named_child = false;
    let mut cursor = arguments.walk();
    for argument in arguments.named_children(&mut cursor) {
        saw_named_child = true;
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
    if !saw_named_child {
        return argument_texts_pass_roots(arguments, parameter, roots, source);
    }
    false
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

fn collect_nodes<'tree>(node: Node<'tree>, kind: &str, nodes: &mut Vec<Node<'tree>>) {
    if node.kind() == kind {
        nodes.push(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_nodes(child, kind, nodes);
    }
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
