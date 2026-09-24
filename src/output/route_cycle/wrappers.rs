use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use tree_sitter::Node;

use super::{
    argument_list,
    navigation::{go_router_symbol_import, term_identifier_shadowed_at},
    registry_api::dependency_imports_name,
    route_constructor_receiver, visit_named,
};
use crate::{DartFile, DependencyKind, ResolvedDependency};

pub(super) fn helper_has_typed_route_wrapper_call(
    helper_file: &DartFile,
    root: Node<'_>,
    source: &str,
    route_classes: &BTreeSet<String>,
    files_by_path: &BTreeMap<PathBuf, &DartFile>,
    dependencies: &[ResolvedDependency],
) -> bool {
    let mut found = false;
    let mut wrapper_results = BTreeMap::new();
    visit_named(root, &mut |call| {
        if found || call.kind() != "call_expression" {
            return;
        }
        let Some(function) = call.child_by_field_name("function") else {
            return;
        };
        let Some((function, prefix)) = wrapper_call_name_nodes(function) else {
            return;
        };
        let Ok(function_name) = function.utf8_text(source.as_bytes()) else {
            return;
        };
        let prefix_name = prefix.and_then(|node| node.utf8_text(source.as_bytes()).ok());
        if prefix.is_some() && prefix_name.is_none() {
            return;
        }
        if (prefix.is_none()
            && helper_file
                .declarations
                .iter()
                .any(|declaration| declaration.name == function_name))
            || prefix.is_some_and(|node| {
                term_identifier_shadowed_at(root, node, prefix_name.unwrap_or_default(), source)
            })
            || (prefix.is_none()
                && term_identifier_shadowed_at(root, function, function_name, source))
        {
            return;
        }
        let Some(arguments) = argument_list(call) else {
            return;
        };
        let mut cursor = arguments.walk();
        for argument in arguments.named_children(&mut cursor) {
            let Some((parameter_name, value)) = named_argument(argument, source) else {
                continue;
            };
            if !route_constructor_receiver(root, value, route_classes, source) {
                continue;
            }
            found = dependencies.iter().any(|import| {
                import.kind == DependencyKind::Import
                    && import.from_path == helper_file.path
                    && import.visibility.prefix.as_deref() == prefix_name
                    && dependency_imports_name(import, function_name)
                    && files_by_path
                        .get(&import.to_path)
                        .is_some_and(|wrapper_file| {
                            *wrapper_results
                                .entry((
                                    import.to_path.clone(),
                                    function_name.to_owned(),
                                    parameter_name.to_owned(),
                                ))
                                .or_insert_with(|| {
                                    wrapper_navigates_parameter(
                                        wrapper_file,
                                        function_name,
                                        parameter_name,
                                    )
                                })
                        })
            });
            if found {
                break;
            }
        }
    });
    found
}

fn wrapper_call_name_nodes(function: Node<'_>) -> Option<(Node<'_>, Option<Node<'_>>)> {
    if function.kind() == "identifier" {
        return Some((function, None));
    }
    if function.kind() == "member_expression" {
        let receiver = function.child_by_field_name("object")?;
        let receiver = callback_body_identifier(receiver)
            .or_else(|| (receiver.kind() == "identifier").then_some(receiver))?;
        let method = function.child_by_field_name("property")?;
        return (method.kind() == "identifier").then_some((method, Some(receiver)));
    }
    // Dart Tree-Sitter represents `() => navigate(...)` as a call whose
    // function is the closure and whose body is the callee identifier.
    callback_body_identifier(function).map(|name| (name, None))
}

fn callback_body_identifier(function: Node<'_>) -> Option<Node<'_>> {
    if function.kind() != "function_expression" {
        return None;
    }
    let parameters = function.child_by_field_name("parameters")?;
    if parameters.named_child_count() != 0 {
        return None;
    }
    let body = function.child_by_field_name("body")?;
    if body.kind() != "function_expression_body" || body.named_child_count() != 1 {
        return None;
    }
    let name = body.named_child(0)?;
    (name.kind() == "identifier").then_some(name)
}

fn named_argument<'tree, 'source>(
    argument: Node<'tree>,
    source: &'source str,
) -> Option<(&'source str, Node<'tree>)> {
    if argument.kind() != "named_argument" {
        return None;
    }
    let mut cursor = argument.walk();
    let mut children = argument.named_children(&mut cursor);
    let label = children.next()?;
    if label.kind() != "label" {
        return None;
    }
    let name = label.named_child(0)?.utf8_text(source.as_bytes()).ok()?;
    Some((name, children.next()?))
}

fn wrapper_navigates_parameter(file: &DartFile, function_name: &str, parameter_name: &str) -> bool {
    let Ok(source) = fs::read_to_string(&file.path) else {
        return false;
    };
    let Ok(parsed) = crate::dart_parser::parse_dart_source_lossy(&file.path, &source) else {
        return false;
    };
    let root = parsed.tree().root_node();
    let source = parsed.source();
    if !go_router_symbol_import(root, None, "GoRouteData", source) {
        return false;
    }
    let mut cursor = root.walk();
    root.named_children(&mut cursor).any(|function| {
        if function.kind() != "function_declaration" {
            return false;
        }
        let Some(signature) = function.child_by_field_name("signature") else {
            return false;
        };
        if field_text(signature, "name", source) != Some(function_name) {
            return false;
        }
        let Some(parameters) = signature.child_by_field_name("parameters") else {
            return false;
        };
        if !typed_parameter(parameters, parameter_name, "GoRouteData", source) {
            return false;
        }
        let Some(body) = function.child_by_field_name("body") else {
            return false;
        };
        let mut navigates = false;
        visit_named(body, &mut |call| {
            if navigates || call.kind() != "call_expression" {
                return;
            }
            let Some(member) = call.child_by_field_name("function") else {
                return;
            };
            if member.kind() != "member_expression"
                || field_text(member, "object", source) != Some(parameter_name)
                || !matches!(
                    field_text(member, "property", source),
                    Some("go" | "push" | "pushReplacement" | "replace")
                )
            {
                return;
            }
            let Some(destination) = member.child_by_field_name("object") else {
                return;
            };
            if term_identifier_shadowed_at(body, destination, parameter_name, source) {
                return;
            }
            let Some(arguments) = argument_list(call) else {
                return;
            };
            let Some(context) = arguments.named_child(0) else {
                return;
            };
            let Ok(context_name) = context.utf8_text(source.as_bytes()) else {
                return;
            };
            navigates = context.kind() == "identifier"
                && !term_identifier_shadowed_at(body, context, context_name, source)
                && typed_parameter(parameters, context_name, "BuildContext", source);
        });
        navigates
    })
}

fn typed_parameter(parameters: Node<'_>, name: &str, type_name: &str, source: &str) -> bool {
    let mut found = false;
    visit_named(parameters, &mut |parameter| {
        if found || parameter.kind() != "formal_parameter" {
            return;
        }
        let mut cursor = parameter.walk();
        found = field_text(parameter, "name", source) == Some(name)
            && parameter
                .named_children(&mut cursor)
                .find(|child| child.kind() == "type")
                .is_some_and(|kind| kind.utf8_text(source.as_bytes()).ok() == Some(type_name));
    });
    found
}

fn field_text<'source>(node: Node<'_>, field: &str, source: &'source str) -> Option<&'source str> {
    node.child_by_field_name(field)?
        .utf8_text(source.as_bytes())
        .ok()
}
