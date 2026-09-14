use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tree_sitter::Node;

use super::{Location, field_text};

/// A `GoRouter` route declaration found in Dart metadata or route trees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DartRouteDeclaration {
    /// Generic route data class, such as `HomeRoute`.
    pub route_class: String,
    /// Canonical full path when the route path is statically known.
    pub path: Option<String>,
    /// Raw `path:` argument expression.
    pub path_expression: String,
    /// Optional literal route name.
    pub name: Option<String>,
    /// Location of the route annotation or constructor call.
    pub location: Location,
}

pub(super) fn extract_route_declarations(
    root: Node<'_>,
    source: &str,
) -> Vec<DartRouteDeclaration> {
    let constants = collect_string_constants(root, source);
    let mut routes = Vec::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() != "class_declaration" {
            continue;
        }
        let class_name = field_text(child, "name", source);
        let mut class_cursor = child.walk();
        for annotation in child
            .named_children(&mut class_cursor)
            .filter(|node| node.kind() == "annotation")
        {
            let Ok(text) = annotation.utf8_text(source.as_bytes()) else {
                continue;
            };
            collect_routes_in_text(
                text,
                0,
                None,
                annotation.start_position().into(),
                class_name.as_deref(),
                &constants,
                &mut routes,
            );
        }
    }
    collect_raw_route_declarations(root, source, &constants, &mut routes);
    routes
}

fn collect_raw_route_declarations(
    root: Node<'_>,
    source: &str,
    constants: &BTreeMap<String, String>,
    routes: &mut Vec<DartRouteDeclaration>,
) {
    collect_raw_routes_in(root, source, constants, None, true, routes);
}

fn collect_raw_routes_in(
    node: Node<'_>,
    source: &str,
    constants: &BTreeMap<String, String>,
    parent_path: Option<&str>,
    parent_resolved: bool,
    routes: &mut Vec<DartRouteDeclaration>,
) {
    if is_object_constructor(node)
        && let Some(route_kind) = raw_route_constructor_kind(node, source)
    {
        let next_parent = match route_kind {
            RawRouteKind::GoRoute => collect_go_route(
                node,
                source,
                constants,
                parent_path,
                parent_resolved,
                routes,
            ),
            RawRouteKind::RouteContainer => None,
        };
        let next_parent_resolved = match route_kind {
            RawRouteKind::GoRoute => next_parent.is_some(),
            RawRouteKind::RouteContainer => parent_resolved,
        };
        for child_routes_arg in route_child_argument_nodes(node, source) {
            collect_raw_routes_in(
                child_routes_arg,
                source,
                constants,
                next_parent.as_deref().or(parent_path),
                next_parent_resolved,
                routes,
            );
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_raw_routes_in(
            child,
            source,
            constants,
            parent_path,
            parent_resolved,
            routes,
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RawRouteKind {
    GoRoute,
    RouteContainer,
}

fn is_object_constructor(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "call_expression" | "constructor_invocation" | "const_object_expression" | "new_expression"
    )
}

fn raw_route_constructor_kind(node: Node<'_>, source: &str) -> Option<RawRouteKind> {
    let route_type = constructor_type_name(node, source)?;
    if route_type.split('.').any(|segment| segment == "GoRoute") {
        return Some(RawRouteKind::GoRoute);
    }
    if route_type.split('.').any(|segment| {
        matches!(
            segment,
            "ShellRoute" | "StatefulShellRoute" | "StatefulShellBranch"
        )
    }) {
        return Some(RawRouteKind::RouteContainer);
    }
    None
}

fn constructor_type_name(node: Node<'_>, source: &str) -> Option<String> {
    let arguments = node.child_by_field_name("arguments")?;
    let prefix = source.get(node.start_byte()..arguments.start_byte())?;
    let route_type = prefix
        .trim()
        .strip_prefix("const ")
        .or_else(|| prefix.trim().strip_prefix("new "))
        .unwrap_or(prefix.trim())
        .split('<')
        .next()
        .unwrap_or("")
        .replace(' ', "");
    (!route_type.is_empty()).then_some(route_type)
}

fn route_child_argument_nodes<'a>(node: Node<'a>, source: &str) -> Vec<Node<'a>> {
    ["routes", "branches"]
        .into_iter()
        .filter_map(|name| named_argument_node(node, name, source))
        .collect()
}

fn collect_go_route(
    node: Node<'_>,
    source: &str,
    constants: &BTreeMap<String, String>,
    parent_path: Option<&str>,
    parent_resolved: bool,
    routes: &mut Vec<DartRouteDeclaration>,
) -> Option<String> {
    let path_node = named_argument_node(node, "path", source)?;
    let path_expression = expression_text_from_named_argument(path_node, source)?;
    let path_segment = resolve_path_expression(&path_expression, constants);
    let full_path = path_segment
        .as_deref()
        .filter(|_| parent_resolved)
        .map(|segment| join_route_path(parent_path, segment));
    let name = named_argument_node(node, "name", source)
        .and_then(|name_node| expression_text_from_named_argument(name_node, source))
        .and_then(|value| unquote_dart_string(value.trim()));
    routes.push(DartRouteDeclaration {
        route_class: "GoRoute".to_owned(),
        path: full_path.clone(),
        path_expression,
        name,
        location: node.start_position().into(),
    });
    full_path
}

fn named_argument_node<'a>(node: Node<'a>, name: &str, source: &str) -> Option<Node<'a>> {
    let arguments = node.child_by_field_name("arguments")?;
    let mut cursor = arguments.walk();
    arguments.named_children(&mut cursor).find(|child| {
        child.kind() == "named_argument" && named_argument_label(*child, source) == Some(name)
    })
}

fn named_argument_label<'a>(node: Node<'a>, source: &'a str) -> Option<&'a str> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == "label")
        .and_then(|label| label.utf8_text(source.as_bytes()).ok())
        .map(str::trim)
        .and_then(|label| label.strip_suffix(':'))
}

fn expression_text_from_named_argument(node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() != "label")
        .and_then(|expression| expression.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn collect_string_constants(root: Node<'_>, source: &str) -> BTreeMap<String, String> {
    let mut constants = BTreeMap::new();
    collect_string_constants_in(root, source, &mut constants);
    constants
}

fn collect_string_constants_in(
    node: Node<'_>,
    source: &str,
    constants: &mut BTreeMap<String, String>,
) {
    if matches!(
        node.kind(),
        "static_final_declaration" | "initialized_identifier"
    ) && is_const_string_declaration(node, source)
        && let Some(name) = field_text(node, "name", source)
        && let Some(value) = node.child_by_field_name("value").and_then(|value| {
            value
                .utf8_text(source.as_bytes())
                .ok()
                .and_then(unquote_dart_string)
        })
    {
        constants.insert(name.clone(), value.clone());
        if let Some(class_name) = containing_class_name(node, source) {
            constants.insert(format!("{class_name}.{name}"), value);
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_string_constants_in(child, source, constants);
    }
}

fn is_const_string_declaration(node: Node<'_>, source: &str) -> bool {
    let mut current = node.parent();
    for _ in 0..5 {
        let Some(parent) = current else {
            return false;
        };
        if matches!(
            parent.kind(),
            "declaration" | "top_level_variable_declaration" | "local_variable_declaration"
        ) {
            return parent
                .utf8_text(source.as_bytes())
                .is_ok_and(|text| text.contains("const") && text.contains('='));
        }
        current = parent.parent();
    }
    false
}

fn containing_class_name(node: Node<'_>, source: &str) -> Option<String> {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "class_declaration" {
            return field_text(parent, "name", source);
        }
        current = parent.parent();
    }
    None
}

fn collect_routes_in_text(
    text: &str,
    offset: usize,
    parent_path: Option<&str>,
    base: Location,
    default_class: Option<&str>,
    constants: &BTreeMap<String, String>,
    routes: &mut Vec<DartRouteDeclaration>,
) {
    let mut cursor = 0;
    while let Some((found, constructor)) = find_next_route_call(text, cursor) {
        let Some(call) = parse_route_call(
            text,
            found,
            constructor,
            offset,
            parent_path,
            base,
            constants,
        ) else {
            cursor = found + constructor.len();
            continue;
        };

        let route_class = call
            .route_class
            .clone()
            .or_else(|| default_class.map(str::to_owned))
            .unwrap_or_else(|| "<unknown-route>".to_owned());
        routes.push(DartRouteDeclaration {
            route_class,
            path: call.full_path.clone(),
            path_expression: call.path_expression.clone(),
            name: call.name.clone(),
            location: call.location,
        });

        if let Some(routes_arg) = named_argument_region(call.arguments, "routes") {
            collect_routes_in_text(
                &call.arguments[routes_arg.clone()],
                offset + call.arguments_start + routes_arg.start,
                call.full_path.as_deref(),
                base,
                None,
                constants,
                routes,
            );
        }
        cursor = call.end;
    }
}

#[derive(Debug)]
struct ParsedRouteCall<'a> {
    route_class: Option<String>,
    path_expression: String,
    full_path: Option<String>,
    name: Option<String>,
    location: Location,
    arguments: &'a str,
    arguments_start: usize,
    end: usize,
}

fn parse_route_call<'a>(
    text: &'a str,
    start: usize,
    constructor: &str,
    offset: usize,
    parent_path: Option<&str>,
    base: Location,
    constants: &BTreeMap<String, String>,
) -> Option<ParsedRouteCall<'a>> {
    let name_end = start + constructor.len();
    let (route_class, after_type_args) = parse_type_arguments(text, name_end)?;
    let open = text[after_type_args..].find('(')? + after_type_args;
    let close = matching_delimiter(text, open, '(', ')')?;
    let arguments = &text[open + 1..close];
    let path_expression = named_argument(arguments, "path")?.trim().to_owned();
    let path_segment = resolve_path_expression(&path_expression, constants);
    let full_path = path_segment
        .as_deref()
        .map(|segment| join_route_path(parent_path, segment));
    let name =
        named_argument(arguments, "name").and_then(|value| unquote_dart_string(value.trim()));
    Some(ParsedRouteCall {
        route_class,
        path_expression,
        full_path,
        name,
        location: location_for_offset(text, base, route_location_offset(text, start) + offset),
        arguments,
        arguments_start: open + 1,
        end: close + 1,
    })
}

fn find_next_route_call(text: &str, start: usize) -> Option<(usize, &'static str)> {
    const CONSTRUCTORS: [&str; 2] = ["TypedGoRoute", "TypedRelativeGoRoute"];
    let mut index = start;
    while index < text.len() {
        for constructor in CONSTRUCTORS {
            if starts_identifier_at(text, index, constructor) {
                return Some((index, constructor));
            }
        }
        index += text[index..].chars().next()?.len_utf8();
    }
    None
}

fn starts_identifier_at(text: &str, index: usize, ident: &str) -> bool {
    text[index..].starts_with(ident)
        && text[..index]
            .chars()
            .next_back()
            .is_none_or(|ch| !is_identifier_char(ch))
        && text[index + ident.len()..]
            .chars()
            .next()
            .is_none_or(|ch| !is_identifier_char(ch))
}

fn is_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn parse_type_arguments(text: &str, start: usize) -> Option<(Option<String>, usize)> {
    let open = text[start..].find('<')? + start;
    let close = matching_delimiter(text, open, '<', '>')?;
    let route_class = text[open + 1..close]
        .split(',')
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    Some((route_class, close + 1))
}

fn named_argument(arguments: &str, name: &str) -> Option<String> {
    named_argument_region(arguments, name).map(|range| arguments[range].to_owned())
}

fn named_argument_region(arguments: &str, name: &str) -> Option<std::ops::Range<usize>> {
    let index = find_label(arguments, name, 0)?;
    let expression_start = index + name.len() + 1;
    let expression_end = end_of_expression(arguments, expression_start);
    Some(expression_start..expression_end)
}

fn find_label(text: &str, name: &str, start: usize) -> Option<usize> {
    let needle = format!("{name}:");
    let mut cursor = start;
    while let Some(index) = text[cursor..].find(&needle).map(|index| index + cursor) {
        if text[..index]
            .chars()
            .next_back()
            .is_none_or(|ch| !is_identifier_char(ch))
        {
            return Some(index);
        }
        cursor = index + needle.len();
    }
    None
}

fn end_of_expression(text: &str, start: usize) -> usize {
    let mut depth = Vec::new();
    let mut quote = None;
    let mut index = start;
    while index < text.len() {
        let ch = text[index..].chars().next().unwrap_or_default();
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
        } else {
            match ch {
                '\'' | '"' => quote = Some(ch),
                '(' | '[' | '{' | '<' => depth.push(ch),
                ')' | ']' | '}' | '>' => {
                    depth.pop();
                }
                ',' if depth.is_empty() => return index,
                _ => {}
            }
        }
        index += ch.len_utf8();
    }
    text.len()
}

fn matching_delimiter(text: &str, open: usize, left: char, right: char) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    let mut index = open;
    while index < text.len() {
        let ch = text[index..].chars().next()?;
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
        } else if ch == '\'' || ch == '"' {
            quote = Some(ch);
        } else if ch == left {
            depth += 1;
        } else if ch == right {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return Some(index);
            }
        }
        index += ch.len_utf8();
    }
    None
}

fn resolve_path_expression(
    expression: &str,
    constants: &BTreeMap<String, String>,
) -> Option<String> {
    unquote_dart_string(expression.trim()).or_else(|| {
        let key = expression.trim().replace(' ', "");
        constants.get(&key).cloned()
    })
}

fn join_route_path(parent: Option<&str>, child: &str) -> String {
    let joined = if child.starts_with('/') {
        child.to_owned()
    } else if let Some(parent) = parent {
        format!(
            "{}/{}",
            parent.trim_end_matches('/'),
            child.trim_start_matches('/')
        )
    } else {
        format!("/{}", child.trim_start_matches('/'))
    };
    normalize_route_path(&joined)
}

fn normalize_route_path(path: &str) -> String {
    let mut normalized = path.replace("//", "/");
    while normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }
    if normalized.is_empty() {
        "/".to_owned()
    } else if normalized.starts_with('/') {
        normalized
    } else {
        format!("/{normalized}")
    }
}

fn route_location_offset(text: &str, start: usize) -> usize {
    if start > 0 && text[..start].trim() == "@" {
        start - 1
    } else {
        start
    }
}

fn location_for_offset(text: &str, base: Location, offset: usize) -> Location {
    let prefix = &text[..offset.min(text.len())];
    let newlines = prefix.bytes().filter(|byte| *byte == b'\n').count();
    if newlines == 0 {
        return Location {
            line: base.line,
            column: base.column + prefix.len(),
        };
    }
    let column = prefix
        .rsplit_once('\n')
        .map_or(0, |(_, suffix)| suffix.len());
    Location {
        line: base.line + newlines,
        column,
    }
}

fn unquote_dart_string(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let without_raw_prefix = trimmed
        .strip_prefix('r')
        .or_else(|| trimmed.strip_prefix('R'))
        .unwrap_or(trimmed);
    without_raw_prefix
        .strip_prefix("'''")
        .and_then(|inner| inner.strip_suffix("'''"))
        .or_else(|| {
            without_raw_prefix
                .strip_prefix("\"\"\"")
                .and_then(|inner| inner.strip_suffix("\"\"\""))
        })
        .or_else(|| {
            without_raw_prefix
                .strip_prefix('\'')
                .and_then(|inner| inner.strip_suffix('\''))
        })
        .or_else(|| {
            without_raw_prefix
                .strip_prefix('"')
                .and_then(|inner| inner.strip_suffix('"'))
        })
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use crate::extract_dart_source;

    #[test]
    fn extracts_typed_go_routes_and_nested_full_paths() -> Result<(), Box<dyn std::error::Error>> {
        let source = "
@TypedGoRoute<HomeRoute>(
  path: '/',
  routes: <TypedRoute<RouteData>>[
    TypedGoRoute<FamilyRoute>(path: 'family/:fid'),
  ],
)
class HomeRoute extends GoRouteData {}
";
        let file = extract_dart_source("lib/routes.dart", source)?;

        assert_eq!(file.routes.len(), 2);
        assert_eq!(file.routes[0].route_class, "HomeRoute");
        assert_eq!(file.routes[0].path.as_deref(), Some("/"));
        assert_eq!(file.routes[1].route_class, "FamilyRoute");
        assert_eq!(file.routes[1].path.as_deref(), Some("/family/:fid"));
        Ok(())
    }

    #[test]
    fn ignores_comments_strings_and_extracts_raw_go_route_calls()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
// @TypedGoRoute<FakeRoute>(path: '/settings')
const text = "@TypedGoRoute<FakeRoute>(path: '/settings')";
final route = GoRoute(path: '/settings', builder: (_, _) => const SizedBox());
@TypedGoRoute<RealRoute>(path: '/settings')
class RealRoute extends GoRouteData {}
"#;
        let file = extract_dart_source("lib/routes.dart", source)?;

        assert_eq!(file.routes.len(), 2);
        assert_eq!(file.routes[0].route_class, "RealRoute");
        assert_eq!(file.routes[1].route_class, "GoRoute");
        assert_eq!(file.routes[1].path.as_deref(), Some("/settings"));
        Ok(())
    }

    #[test]
    fn resolves_same_file_static_const_paths() -> Result<(), Box<dyn std::error::Error>> {
        let source = "
class AppRoutePaths {
  static const String home = '/home';
}

@TypedGoRoute<HomeRoute>(path: AppRoutePaths.home)
class HomeRoute extends GoRouteData {}
";
        let file = extract_dart_source("lib/routes.dart", source)?;

        assert_eq!(file.routes[0].path.as_deref(), Some("/home"));
        Ok(())
    }

    #[test]
    fn extracts_raw_go_routes_and_nested_full_paths() -> Result<(), Box<dyn std::error::Error>> {
        let source = "
final router = GoRouter(
  routes: [
    ShellRoute(
      routes: [
        GoRoute(
          path: '/home',
          name: 'home',
          routes: [
            GoRoute(path: 'details', builder: (_, _) => const SizedBox()),
          ],
          builder: (_, _, child) => child,
        ),
      ],
      builder: (_, _, child) => child,
    ),
  ],
);
";
        let file = extract_dart_source("lib/routes.dart", source)?;

        let raw_routes = file
            .routes
            .iter()
            .filter(|route| route.route_class == "GoRoute")
            .collect::<Vec<_>>();
        assert_eq!(raw_routes.len(), 2);
        assert_eq!(raw_routes[0].path.as_deref(), Some("/home"));
        assert_eq!(raw_routes[0].name.as_deref(), Some("home"));
        assert_eq!(raw_routes[1].path.as_deref(), Some("/home/details"));
        Ok(())
    }
}
