use std::fs;
use std::path::{Path, PathBuf};

use tree_sitter::Node;

use crate::{DartFile, Location, generated::is_generated_dart_path, scan::ScannedProject};

use super::navigation::{
    go_router_symbol_import, has_go_router_import, local_type_declaration_named,
    navigation_receiver_is_imported_go_router_api,
};
use super::{
    argument_list, direct_named_child, field_text, find_first_named_descendant,
    strip_method_type_arguments, strip_whitespace, visit_named,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::output) enum MixedGoRouterUseKind {
    RouteDefinition,
    Navigation { method: String },
    RedirectDestination,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::output) struct MixedGoRouterUse {
    pub path: PathBuf,
    pub location: Location,
    pub kind: MixedGoRouterUseKind,
}

pub(in crate::output) fn mixed_go_router_uses(project: &ScannedProject) -> Vec<MixedGoRouterUse> {
    if !project.files.iter().any(is_typed_route_adoption) {
        return Vec::new();
    }

    let mut uses = project
        .files
        .iter()
        .filter(|file| is_production_path(&file.path))
        .flat_map(mixed_go_router_uses_in_file)
        .collect::<Vec<_>>();
    uses.sort_by(|left, right| {
        (
            &left.path,
            left.location.line,
            left.location.column,
            &left.kind,
        )
            .cmp(&(
                &right.path,
                right.location.line,
                right.location.column,
                &right.kind,
            ))
    });
    uses
}

fn is_typed_route_adoption(file: &DartFile) -> bool {
    if !is_production_path(&file.path) || !file_imports_go_router(file) {
        return false;
    }
    let Ok(source) = fs::read_to_string(&file.path) else {
        return false;
    };
    let Ok(parsed) = crate::dart_parser::parse_dart_source_lossy(&file.path, &source) else {
        return false;
    };
    let root = parsed.tree().root_node();
    let source = parsed.source();
    let mut adopted = false;
    visit_named(root, &mut |node| {
        if adopted || node.kind() != "class_declaration" {
            return;
        }
        adopted = typed_route_class_uses_imported_api(root, node, source);
    });
    adopted
}

fn typed_route_class_uses_imported_api(root: Node<'_>, class: Node<'_>, source: &str) -> bool {
    let mut cursor = class.walk();
    let imported_annotation = class
        .named_children(&mut cursor)
        .filter(|child| child.kind() == "annotation")
        .any(|annotation| imported_typed_route_annotation(root, annotation, source));
    imported_annotation
        && class
            .child_by_field_name("superclass")
            .and_then(|superclass| superclass.utf8_text(source.as_bytes()).ok())
            .is_some_and(|superclass| imported_route_data_superclass(root, superclass, source))
}

fn imported_typed_route_annotation(root: Node<'_>, annotation: Node<'_>, source: &str) -> bool {
    let compact = annotation
        .utf8_text(source.as_bytes())
        .map(strip_whitespace)
        .unwrap_or_default();
    let reference = compact
        .strip_prefix('@')
        .unwrap_or(&compact)
        .split('<')
        .next()
        .unwrap_or_default();
    [
        "TypedGoRoute",
        "TypedRelativeGoRoute",
        "TypedShellRoute",
        "TypedStatefulShellRoute",
        "TypedStatefulShellBranch",
    ]
    .iter()
    .any(|type_name| imported_go_router_type_reference(root, reference, type_name, source))
}

fn imported_route_data_superclass(root: Node<'_>, superclass: &str, source: &str) -> bool {
    let compact = strip_whitespace(superclass);
    let reference = compact.strip_prefix("extends").unwrap_or(&compact);
    [
        "GoRouteData",
        "RelativeGoRouteData",
        "ShellRouteData",
        "StatefulShellRouteData",
        "StatefulShellBranchData",
    ]
    .iter()
    .any(|type_name| imported_go_router_type_reference(root, reference, type_name, source))
}

fn mixed_go_router_uses_in_file(file: &DartFile) -> Vec<MixedGoRouterUse> {
    if !file_imports_go_router(file) {
        return Vec::new();
    }
    let Ok(source) = fs::read_to_string(&file.path) else {
        return Vec::new();
    };
    let Ok(parsed) = crate::dart_parser::parse_dart_source_lossy(&file.path, &source) else {
        return Vec::new();
    };
    let root = parsed.tree().root_node();
    let source = parsed.source();
    if !has_go_router_import(root, source) {
        return Vec::new();
    }

    let mut uses = Vec::new();
    collect_imported_raw_go_route_definitions(root, source, &file.path, &mut uses);
    collect_raw_go_router_navigation(root, source, &file.path, &mut uses);
    collect_raw_redirect_destinations(root, source, file, &mut uses);
    uses
}

fn collect_imported_raw_go_route_definitions(
    root: Node<'_>,
    source: &str,
    path: &Path,
    uses: &mut Vec<MixedGoRouterUse>,
) {
    visit_named(root, &mut |node| {
        if !matches!(
            node.kind(),
            "call_expression"
                | "constructor_invocation"
                | "const_object_expression"
                | "new_expression"
        ) {
            return;
        }
        let Some(arguments) = argument_list(node) else {
            return;
        };
        if !has_named_argument(arguments, "path", source) {
            return;
        }
        let Some(prefix) = source.get(node.start_byte()..arguments.start_byte()) else {
            return;
        };
        if imported_go_router_type_reference(root, prefix, "GoRoute", source) {
            uses.push(MixedGoRouterUse {
                path: path.to_path_buf(),
                location: node.start_position().into(),
                kind: MixedGoRouterUseKind::RouteDefinition,
            });
        }
    });
}

fn collect_raw_go_router_navigation(
    root: Node<'_>,
    source: &str,
    path: &Path,
    uses: &mut Vec<MixedGoRouterUse>,
) {
    visit_named(root, &mut |node| {
        if !matches!(
            node.kind(),
            "call_expression" | "function_expression_invocation"
        ) {
            return;
        }
        let Some(arguments) = argument_list(node) else {
            return;
        };
        let Some(prefix) = source.get(node.start_byte()..arguments.start_byte()) else {
            return;
        };
        let Some(method) = raw_go_router_navigation_name(prefix) else {
            return;
        };
        let Some(receiver) = navigation_receiver(prefix, &method) else {
            return;
        };
        if navigation_receiver_is_imported_go_router_api(root, node, &receiver, source) {
            uses.push(MixedGoRouterUse {
                path: path.to_path_buf(),
                location: node.start_position().into(),
                kind: MixedGoRouterUseKind::Navigation { method },
            });
        }
    });
}

fn collect_raw_redirect_destinations(
    root: Node<'_>,
    source: &str,
    file: &DartFile,
    uses: &mut Vec<MixedGoRouterUse>,
) {
    visit_named(root, &mut |node| {
        if node.kind() == "class_declaration"
            && typed_route_class_uses_imported_api(root, node, source)
        {
            collect_typed_route_redirect_methods(node, source, &file.path, uses);
        }
        if is_imported_go_router_constructor(root, node, source) {
            collect_go_router_redirect_argument(node, source, &file.path, uses);
        }
    });
}

fn collect_typed_route_redirect_methods(
    class: Node<'_>,
    source: &str,
    path: &Path,
    uses: &mut Vec<MixedGoRouterUse>,
) {
    let Some(body) = class.child_by_field_name("body") else {
        return;
    };
    let mut cursor = body.walk();
    for member in body.named_children(&mut cursor) {
        let Some(method) = find_first_named_descendant(member, "method_declaration") else {
            continue;
        };
        if method_name(method, source).as_deref() != Some("redirect") {
            continue;
        }
        if method
            .child_by_field_name("body")
            .is_some_and(|body| redirect_body_has_raw_literal(body, source))
        {
            uses.push(MixedGoRouterUse {
                path: path.to_path_buf(),
                location: method.start_position().into(),
                kind: MixedGoRouterUseKind::RedirectDestination,
            });
        }
    }
}

fn collect_go_router_redirect_argument(
    constructor: Node<'_>,
    source: &str,
    path: &Path,
    uses: &mut Vec<MixedGoRouterUse>,
) {
    let Some(arguments) = argument_list(constructor) else {
        return;
    };
    let mut cursor = arguments.walk();
    for argument in arguments.named_children(&mut cursor) {
        if argument.kind() != "named_argument"
            || named_argument_label(argument, source).as_deref() != Some("redirect")
        {
            continue;
        }
        let Some(value) = named_argument_value(argument) else {
            continue;
        };
        if redirect_callback_has_raw_literal(value, source) {
            uses.push(MixedGoRouterUse {
                path: path.to_path_buf(),
                location: argument.start_position().into(),
                kind: MixedGoRouterUseKind::RedirectDestination,
            });
        }
    }
}

fn redirect_callback_has_raw_literal(value: Node<'_>, source: &str) -> bool {
    let body = if value.kind() == "function_expression" {
        value.child_by_field_name("body")
    } else {
        find_first_named_descendant(value, "function_expression_body")
    };
    let Some(body) = body else {
        return false;
    };
    redirect_body_has_raw_literal(body, source)
}

fn redirect_body_has_raw_literal(body: Node<'_>, source: &str) -> bool {
    let Some(expression_or_block) = first_non_label_named_child(body) else {
        return false;
    };
    if expression_or_block.kind() != "block" {
        return raw_redirect_expression(expression_or_block, source);
    }

    let mut found = false;
    visit_returns_without_nested_callables(expression_or_block, &mut |return_statement| {
        if first_non_label_named_child(return_statement)
            .is_some_and(|expression| raw_redirect_expression(expression, source))
        {
            found = true;
        }
    });
    found
}

fn raw_redirect_expression(node: Node<'_>, source: &str) -> bool {
    let text = node.utf8_text(source.as_bytes()).unwrap_or_default().trim();
    if is_dart_string_literal(text) {
        return true;
    }
    if node.kind() == "conditional_expression" {
        return ["consequence", "alternative"].into_iter().any(|field| {
            node.child_by_field_name(field)
                .is_some_and(|branch| raw_redirect_expression(branch, source))
        });
    }
    if node.kind() == "if_null_expression" {
        let mut cursor = node.walk();
        return node
            .named_children(&mut cursor)
            .any(|branch| raw_redirect_expression(branch, source));
    }
    matches!(
        node.kind(),
        "parenthesized_expression" | "function_expression_body"
    ) && first_non_label_named_child(node)
        .is_some_and(|expression| raw_redirect_expression(expression, source))
}

fn is_dart_string_literal(text: &str) -> bool {
    let text = text.trim_start_matches("const").trim();
    let text = text.strip_prefix('r').unwrap_or(text);
    text.starts_with('\'') || text.starts_with('"')
}

fn visit_returns_without_nested_callables(node: Node<'_>, visitor: &mut impl FnMut(Node<'_>)) {
    if node.kind() == "return_statement" {
        visitor(node);
        return;
    }
    if matches!(
        node.kind(),
        "function_expression" | "method_declaration" | "function_declaration"
    ) {
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visit_returns_without_nested_callables(child, visitor);
    }
}

fn is_imported_go_router_constructor(root: Node<'_>, node: Node<'_>, source: &str) -> bool {
    if !matches!(
        node.kind(),
        "call_expression" | "constructor_invocation" | "const_object_expression" | "new_expression"
    ) {
        return false;
    }
    let Some(arguments) = argument_list(node) else {
        return false;
    };
    let Some(prefix) = source.get(node.start_byte()..arguments.start_byte()) else {
        return false;
    };
    imported_go_router_type_reference(root, prefix, "GoRouter", source)
}

fn imported_go_router_type_reference(
    root: Node<'_>,
    prefix: &str,
    type_name: &str,
    source: &str,
) -> bool {
    let compact = strip_whitespace(prefix);
    let reference = compact
        .strip_prefix("const")
        .or_else(|| compact.strip_prefix("new"))
        .unwrap_or(&compact)
        .split('<')
        .next()
        .unwrap_or_default();
    if reference == type_name {
        return go_router_symbol_import(root, None, type_name, source)
            && !local_type_declaration_named(root, type_name, source);
    }
    let Some(import_prefix) = reference.strip_suffix(&format!(".{type_name}")) else {
        return false;
    };
    is_identifier(import_prefix)
        && go_router_symbol_import(root, Some(import_prefix), type_name, source)
}

fn has_named_argument(arguments: Node<'_>, name: &str, source: &str) -> bool {
    let mut cursor = arguments.walk();
    arguments.named_children(&mut cursor).any(|argument| {
        argument.kind() == "named_argument"
            && named_argument_label(argument, source).as_deref() == Some(name)
    })
}

fn named_argument_label(node: Node<'_>, source: &str) -> Option<String> {
    direct_named_child(node, "label")
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

fn first_non_label_named_child(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() != "label")
}

fn method_name(node: Node<'_>, source: &str) -> Option<String> {
    find_first_named_descendant(node, "function_signature")
        .and_then(|signature| field_text(signature, "name", source))
}

fn raw_go_router_navigation_name(prefix: &str) -> Option<String> {
    let compact = strip_whitespace(prefix);
    let name = compact.rsplit('.').next().unwrap_or(compact.as_str());
    let name = strip_method_type_arguments(name);
    matches!(
        name,
        "go" | "push"
            | "pushReplacement"
            | "replace"
            | "goNamed"
            | "pushNamed"
            | "pushReplacementNamed"
            | "replaceNamed"
            | "namedLocation"
    )
    .then(|| name.to_owned())
}

fn navigation_receiver(prefix: &str, method: &str) -> Option<String> {
    let prefix = prefix.trim();
    let without_keyword = super::strip_constructor_keyword_prefix(prefix).unwrap_or(prefix);
    let compact = strip_whitespace(without_keyword);
    let suffix = format!(".{method}");
    compact
        .strip_suffix(&suffix)
        .or_else(|| {
            compact
                .strip_suffix('>')
                .and_then(|without_end| without_end.rfind('<').map(|start| &compact[..start]))
                .and_then(|without_type_arguments| without_type_arguments.strip_suffix(&suffix))
        })
        .map(str::to_owned)
}

fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(|character| {
            character == '_' || character == '$' || character.is_ascii_alphanumeric()
        })
}

fn is_production_path(path: &Path) -> bool {
    !is_generated_dart_path(path) && !is_test_dart_path(path)
}

fn file_imports_go_router(file: &DartFile) -> bool {
    file.imports
        .iter()
        .any(|import| import.uri == "package:go_router/go_router.dart")
}

fn is_test_dart_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with("_test.dart"))
        || path.components().any(|component| {
            component.as_os_str().to_str().is_some_and(|segment| {
                matches!(segment, "test" | "integration_test" | "test_driver")
            })
        })
}
