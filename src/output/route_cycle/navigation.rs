use tree_sitter::Node;

use super::{
    class_extends_state, direct_constructor_call_text, direct_named_child,
    direct_static_member_call_text, is_identifier_character, simple_type_name,
    strip_constructor_keyword_prefix, strip_whitespace, unwrap_parenthesized_text,
};

const BUILD_CONTEXT: &str = "BuildContext";
const GO_ROUTER: &str = "GoRouter";
const GO_ROUTER_FACTORY_METHODS: &[&str] = &["of", "maybeOf", "routingConfig"];

enum NameResolution {
    Type(String),
    Shadowed,
}

impl NameResolution {
    fn matches(&self, accepted: &[&str]) -> bool {
        let Self::Type(type_name) = self else {
            return false;
        };
        accepted.contains(&type_name.as_str())
    }
}

struct NavigationTargetName {
    name: String,
    member_only: bool,
}

pub(super) fn route_extension_navigation_has_context_argument(
    root: Node<'_>,
    site: Node<'_>,
    arguments: Node<'_>,
    source: &str,
) -> bool {
    first_positional_argument(arguments)
        .and_then(|argument| argument.utf8_text(source.as_bytes()).ok())
        .and_then(navigation_target_name)
        .is_some_and(|target| {
            target_resolves_to_navigation_type(root, site, &target, &[BUILD_CONTEXT], source)
        })
}

pub(super) fn navigation_receiver_accepts_route_location(
    root: Node<'_>,
    site: Node<'_>,
    receiver: &str,
    source: &str,
) -> bool {
    let receiver = receiver.trim_end_matches(['?', '!']);
    if go_router_receiver_expression(receiver) {
        return true;
    }
    navigation_target_name(receiver).is_some_and(|target| {
        target_resolves_to_navigation_type(root, site, &target, &[BUILD_CONTEXT, GO_ROUTER], source)
    })
}

fn go_router_receiver_expression(receiver: &str) -> bool {
    let compact = strip_whitespace(receiver.trim());
    let unwrapped = unwrap_parenthesized_text(&compact).unwrap_or(&compact);
    let receiver = unwrapped.trim_end_matches(['?', '!']);
    if direct_constructor_call_text(receiver, GO_ROUTER) {
        return true;
    }
    GO_ROUTER_FACTORY_METHODS
        .iter()
        .any(|method| direct_static_member_call_text(receiver, GO_ROUTER, method))
}

fn first_positional_argument(arguments: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = arguments.walk();
    arguments
        .named_children(&mut cursor)
        .find(|argument| argument.kind() != "named_argument")
}

fn navigation_target_name(text: &str) -> Option<NavigationTargetName> {
    let compact = strip_whitespace(text.trim());
    let unwrapped = unwrap_parenthesized_text(&compact).unwrap_or(&compact);
    let target = unwrapped.trim_end_matches(['?', '!']);
    let (member_only, name) = target
        .strip_prefix("this.")
        .map_or((false, target), |member| (true, member));
    is_simple_identifier(name).then(|| NavigationTargetName {
        name: name.to_owned(),
        member_only,
    })
}

fn target_resolves_to_navigation_type(
    root: Node<'_>,
    site: Node<'_>,
    target: &NavigationTargetName,
    accepted: &[&str],
    source: &str,
) -> bool {
    let resolution = if target.member_only {
        class_member_resolution_at(site, &target.name, source)
    } else {
        visible_name_resolution_at(root, site, &target.name, source)
    };
    resolution.is_some_and(|resolution| resolution.matches(accepted))
}

fn visible_name_resolution_at(
    root: Node<'_>,
    site: Node<'_>,
    name: &str,
    source: &str,
) -> Option<NameResolution> {
    let mut path_child = site;
    let mut parent = site.parent();
    while let Some(scope) = parent {
        if let Some(resolution) = callable_parameter_resolution(scope, name, source) {
            return Some(resolution);
        }
        if let Some(resolution) = prior_binding_resolution(scope, path_child, name, source) {
            return Some(resolution);
        }
        if scoped_header_binding_exists(scope, path_child, site.start_byte(), name, source) {
            return Some(NameResolution::Shadowed);
        }
        if scope.kind() == "class_body" {
            if let Some(resolution) = class_member_resolution(scope, name, source) {
                return Some(resolution);
            }
            if name == "context" && class_extends_state(scope, source) {
                return Some(NameResolution::Type(BUILD_CONTEXT.to_owned()));
            }
        }
        if same_node(scope, root) {
            break;
        }
        path_child = scope;
        parent = scope.parent();
    }
    None
}

fn class_member_resolution_at(site: Node<'_>, name: &str, source: &str) -> Option<NameResolution> {
    let mut current = site.parent();
    while let Some(scope) = current {
        if scope.kind() == "class_body" {
            if let Some(resolution) = class_member_resolution(scope, name, source) {
                return Some(resolution);
            }
            if name == "context" && class_extends_state(scope, source) {
                return Some(NameResolution::Type(BUILD_CONTEXT.to_owned()));
            }
            return None;
        }
        current = scope.parent();
    }
    None
}

fn prior_binding_resolution(
    scope: Node<'_>,
    path_child: Node<'_>,
    name: &str,
    source: &str,
) -> Option<NameResolution> {
    let mut cursor = scope.walk();
    let mut siblings = Vec::new();
    for sibling in scope.named_children(&mut cursor) {
        if same_node(sibling, path_child) || sibling.start_byte() >= path_child.start_byte() {
            break;
        }
        siblings.push(sibling);
    }
    for sibling in siblings.into_iter().rev() {
        if let Some(resolution) = lexical_binding_resolution(sibling, name, source) {
            return Some(resolution);
        }
    }
    None
}

fn lexical_binding_resolution(node: Node<'_>, name: &str, source: &str) -> Option<NameResolution> {
    if node.kind() == "local_function_declaration"
        && local_function_name(node, source).as_deref() == Some(name)
    {
        return Some(NameResolution::Shadowed);
    }
    if is_callable_node(node.kind()) {
        return None;
    }
    if declaration_binds_name(node, name, source) {
        return Some(declaration_type_resolution(node, name, source));
    }
    let mut cursor = node.walk();
    let mut children = node.named_children(&mut cursor).collect::<Vec<_>>();
    children.reverse();
    for child in children {
        if let Some(resolution) = lexical_binding_resolution(child, name, source) {
            return Some(resolution);
        }
    }
    None
}

fn callable_parameter_resolution(
    node: Node<'_>,
    name: &str,
    source: &str,
) -> Option<NameResolution> {
    if !is_callable_node(node.kind()) {
        return None;
    }
    for parameters in direct_parameter_lists(node) {
        for parameter in direct_formal_parameters(parameters) {
            if formal_parameter_name(parameter, source).as_deref() == Some(name) {
                return Some(declaration_type_resolution(parameter, name, source));
            }
        }
    }
    None
}

fn class_member_resolution(
    class_body: Node<'_>,
    name: &str,
    source: &str,
) -> Option<NameResolution> {
    let mut cursor = class_body.walk();
    for member in class_body.named_children(&mut cursor) {
        if member.kind() == "method_declaration"
            && method_name(member, source).as_deref() == Some(name)
        {
            return Some(if method_is_getter(member, source) {
                declaration_type_resolution(member, name, source)
            } else {
                NameResolution::Shadowed
            });
        }
        if is_callable_node(member.kind()) {
            continue;
        }
        if declaration_binds_name(member, name, source) {
            return Some(declaration_type_resolution(member, name, source));
        }
    }
    None
}

fn declaration_type_resolution(node: Node<'_>, name: &str, source: &str) -> NameResolution {
    if node.kind() == "formal_parameter"
        && let Some(type_name) = formal_parameter_type(node, name, source)
    {
        return NameResolution::Type(type_name);
    }
    declared_type_for_name(node, name, source)
        .or_else(|| initializer_type_for_name(node, name, source))
        .map_or(NameResolution::Shadowed, NameResolution::Type)
}

fn declared_type_for_name(node: Node<'_>, name: &str, source: &str) -> Option<String> {
    let text = node.utf8_text(source.as_bytes()).ok()?;
    if declaration_binds_name(node, name, source)
        && let Some(type_name) = declared_type_before_name(text, name)
    {
        return Some(type_name);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if is_callable_node(child.kind()) {
            continue;
        }
        if let Some(type_name) = declared_type_for_name(child, name, source) {
            return Some(type_name);
        }
    }
    None
}

fn declared_type_before_name(text: &str, name: &str) -> Option<String> {
    let declaration = text.split('=').next().unwrap_or(text);
    let name_start = find_identifier(declaration, name)?;
    let before_name = declaration[..name_start].trim();
    let type_name = before_name
        .split_whitespace()
        .map(|part| part.trim_matches(type_token_trim))
        .rfind(|part| !part.is_empty() && !declaration_type_keyword(part))?;
    Some(simple_type_name(type_name.trim_end_matches('?')))
}

fn initializer_type_for_name(node: Node<'_>, name: &str, source: &str) -> Option<String> {
    if field_text(node, "name", source).as_deref() == Some(name)
        && let Some(type_name) = initializer_type(node, source)
    {
        return Some(type_name);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if is_callable_node(child.kind()) {
            continue;
        }
        if let Some(type_name) = initializer_type_for_name(child, name, source) {
            return Some(type_name);
        }
    }
    None
}

fn initializer_type(node: Node<'_>, source: &str) -> Option<String> {
    let text = node.utf8_text(source.as_bytes()).ok()?;
    let (_, right) = text.split_once('=')?;
    let expression = right.trim().trim_end_matches([',', ';']).trim();
    let unwrapped = unwrap_parenthesized_text(expression).unwrap_or(expression);
    let without_keyword = strip_constructor_keyword_prefix(unwrapped).unwrap_or(unwrapped);
    let compact = strip_whitespace(without_keyword);
    for target_type in [BUILD_CONTEXT, GO_ROUTER] {
        if direct_constructor_call_text(&compact, target_type)
            || (target_type == GO_ROUTER
                && GO_ROUTER_FACTORY_METHODS
                    .iter()
                    .any(|method| direct_static_member_call_text(&compact, target_type, method)))
        {
            return Some(target_type.to_owned());
        }
    }
    None
}

fn scoped_header_binding_exists(
    scope: Node<'_>,
    path_child: Node<'_>,
    usage_start: usize,
    name: &str,
    source: &str,
) -> bool {
    if catch_header_binding_exists(scope, path_child, usage_start, name, source) {
        return true;
    }
    if !matches!(
        scope.kind(),
        "for_element"
            | "for_statement"
            | "if_element"
            | "if_statement"
            | "switch_expression_case"
            | "switch_statement_case"
    ) {
        return false;
    }
    let mut cursor = scope.walk();
    for child in scope.named_children(&mut cursor) {
        if same_node(child, path_child) || child.start_byte() >= path_child.start_byte() {
            break;
        }
        if header_binding_child(scope.kind(), child)
            && child.end_byte() <= usage_start
            && declaration_binds_name(child, name, source)
        {
            return true;
        }
    }
    false
}

fn catch_header_binding_exists(
    scope: Node<'_>,
    path_child: Node<'_>,
    usage_start: usize,
    name: &str,
    source: &str,
) -> bool {
    if scope.kind() == "catch_clause" {
        return scope.start_byte() < path_child.start_byte()
            && catch_clause_binds_name(scope, name, source);
    }
    if scope.kind() == "try_statement" {
        let mut previous: Option<Node<'_>> = None;
        let mut cursor = scope.walk();
        for child in scope.named_children(&mut cursor) {
            if same_node(child, path_child) || child.start_byte() >= path_child.start_byte() {
                return previous.is_some_and(|header| {
                    header.kind() == "catch_clause"
                        && matches!(path_child.kind(), "block" | "function_body")
                        && header.end_byte() <= usage_start
                        && source
                            .get(header.end_byte()..path_child.start_byte())
                            .is_some_and(|text| !text.contains("finally"))
                        && catch_clause_binds_name(header, name, source)
                });
            }
            previous = Some(child);
        }
    }
    false
}

fn catch_clause_binds_name(node: Node<'_>, name: &str, source: &str) -> bool {
    field_text(node, "exception", source).as_deref() == Some(name)
        || field_text(node, "stack_trace", source).as_deref() == Some(name)
}

fn header_binding_child(scope_kind: &str, child: Node<'_>) -> bool {
    if matches!(scope_kind, "for_element" | "for_statement") {
        return !is_body_statement_child(child.kind())
            || child.kind() == "local_variable_declaration";
    }
    !is_body_statement_child(child.kind()) && node_can_contain_pattern_binding(child)
}

fn node_can_contain_pattern_binding(node: Node<'_>) -> bool {
    let mut found = false;
    visit_named(node, &mut |candidate| {
        if !found
            && matches!(
                candidate.kind(),
                "list_pattern"
                    | "local_variable_declaration"
                    | "map_pattern"
                    | "object_pattern"
                    | "pattern_variable_declaration"
                    | "record_pattern"
                    | "variable_pattern"
            )
        {
            found = true;
        }
    });
    found
}

fn is_body_statement_child(kind: &str) -> bool {
    matches!(
        kind,
        "assert_statement"
            | "block"
            | "break_statement"
            | "continue_statement"
            | "declaration"
            | "do_statement"
            | "empty_statement"
            | "expression_statement"
            | "for_statement"
            | "if_statement"
            | "local_function_declaration"
            | "local_variable_declaration"
            | "return_statement"
            | "switch_statement"
            | "try_statement"
            | "while_statement"
            | "yield_statement"
    )
}

fn declaration_binds_name(node: Node<'_>, name: &str, source: &str) -> bool {
    if binding_name(node, source).as_deref() == Some(name) {
        return true;
    }
    if is_callable_node(node.kind()) {
        return false;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| declaration_binds_name(child, name, source))
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
    field_text(node, "name", source)
}

fn direct_parameter_lists(node: Node<'_>) -> Vec<Node<'_>> {
    match node.kind() {
        "function_expression" => parameter_field_lists(node),
        "function_declaration" | "local_function_declaration" | "method_declaration" => {
            helper_signature(node)
                .and_then(parameter_list)
                .or_else(|| parameter_list(node))
                .into_iter()
                .collect()
        }
        _ => Vec::new(),
    }
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

fn parameter_field_lists(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.children_by_field_name("parameters", &mut cursor)
        .filter_map(formal_parameter_list)
        .collect()
}

fn helper_signature(node: Node<'_>) -> Option<Node<'_>> {
    let signature = node
        .child_by_field_name("signature")
        .or_else(|| direct_named_child(node, "function_signature"))
        .or_else(|| direct_named_child(node, "method_signature"))?;
    if signature.kind() == "method_signature" {
        return direct_named_child(signature, "function_signature").or(Some(signature));
    }
    Some(signature)
}

fn parameter_list(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("parameters")
        .or_else(|| direct_named_child(node, "formal_parameter_list"))
}

fn formal_parameter_list(node: Node<'_>) -> Option<Node<'_>> {
    if node.kind() == "formal_parameter_list" {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == "formal_parameter_list")
}

fn formal_parameter_name(param: Node<'_>, source: &str) -> Option<String> {
    field_text(param, "name", source).or_else(|| last_identifier_child(param, source))
}

fn formal_parameter_type(param: Node<'_>, name: &str, source: &str) -> Option<String> {
    let text = param.utf8_text(source.as_bytes()).ok()?;
    let name_index = find_identifier(text, name)?;
    let before_name = text[..name_index].trim();
    let type_name = before_name
        .split_whitespace()
        .map(|part| part.trim_matches(type_token_trim))
        .rfind(|part| !part.is_empty() && !declaration_type_keyword(part))?;
    Some(simple_type_name(type_name.trim_end_matches('?')))
}

fn last_identifier_child(node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| {
            matches!(
                child.kind(),
                "identifier" | "identifier_dollar_escaped" | "type_identifier"
            ) && child.utf8_text(source.as_bytes()).ok() != Some("key")
        })
        .filter_map(|child| child.utf8_text(source.as_bytes()).ok())
        .last()
        .map(str::to_owned)
}

fn field_text(node: Node<'_>, field: &str, source: &str) -> Option<String> {
    node.child_by_field_name(field)
        .and_then(|child| child.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn method_name(node: Node<'_>, source: &str) -> Option<String> {
    direct_named_child(node, "function_signature")
        .and_then(|signature| field_text(signature, "name", source))
        .or_else(|| {
            direct_named_child(node, "method_signature").and_then(|signature| {
                direct_named_child(signature, "function_signature")
                    .and_then(|function| field_text(function, "name", source))
                    .or_else(|| field_text(signature, "name", source))
            })
        })
        .or_else(|| field_text(node, "name", source))
}

fn method_is_getter(node: Node<'_>, source: &str) -> bool {
    node.utf8_text(source.as_bytes())
        .ok()
        .is_some_and(|text| text.split_whitespace().any(|part| part == "get"))
}

fn local_function_name(node: Node<'_>, source: &str) -> Option<String> {
    direct_named_child(node, "function_signature")
        .and_then(|signature| signature.child_by_field_name("name"))
        .or_else(|| node.child_by_field_name("name"))
        .and_then(|name| name.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn find_identifier(text: &str, identifier: &str) -> Option<usize> {
    let mut cursor = 0usize;
    while let Some(relative) = text[cursor..].find(identifier) {
        let start = cursor + relative;
        let end = start + identifier.len();
        if !identifier_continues_before(text, start)
            && !text[end..]
                .chars()
                .next()
                .is_some_and(is_identifier_character)
        {
            return Some(start);
        }
        cursor = end;
    }
    None
}

fn identifier_continues_before(text: &str, index: usize) -> bool {
    text[..index]
        .chars()
        .next_back()
        .is_some_and(is_identifier_character)
}

fn is_simple_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(is_identifier_character)
}

fn declaration_type_keyword(text: &str) -> bool {
    matches!(
        text,
        "abstract"
            | "const"
            | "covariant"
            | "external"
            | "final"
            | "get"
            | "late"
            | "required"
            | "set"
            | "static"
            | "var"
    )
}

fn type_token_trim(character: char) -> bool {
    matches!(
        character,
        ',' | ';' | '(' | ')' | '{' | '}' | '[' | ']' | '?' | ':'
    )
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
