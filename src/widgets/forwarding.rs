use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WidgetParamForwarder {
    name: String,
    fields: BTreeSet<String>,
}

pub(super) fn forwarded_param_used(
    forwarders: Option<&Vec<WidgetParamForwarder>>,
    state_bodies: Option<&Vec<Node<'_>>>,
    field_name: &str,
    source: &str,
) -> bool {
    let Some(forwarders) = forwarders else {
        return false;
    };
    let Some(state_bodies) = state_bodies else {
        return false;
    };
    forwarders.iter().any(|forwarder| {
        forwarder.fields.contains(field_name)
            && state_bodies
                .iter()
                .any(|state_body| state_body_calls_forwarder(*state_body, &forwarder.name, source))
    })
}

pub(super) fn widget_forwarded_param_uses<'tree>(
    classes: &[Node<'tree>],
    source: &str,
) -> BTreeMap<String, Vec<WidgetParamForwarder>> {
    let mut uses = BTreeMap::<String, Vec<WidgetParamForwarder>>::new();
    for class in classes {
        let mut declarations = Vec::new();
        collect_nodes(*class, "declaration", &mut declarations);
        for declaration in declarations {
            let Some(signature) = declaration_constructor_signature(declaration) else {
                continue;
            };
            let Some(name) = constructor_qualified_name(*class, signature, source) else {
                continue;
            };
            for (param_name, widget_class) in constructor_widget_params(signature, source) {
                let fields = member_fields_read_from(declaration, &param_name, source);
                if fields.is_empty() {
                    continue;
                }
                uses.entry(widget_class.clone())
                    .or_default()
                    .push(WidgetParamForwarder {
                        name: name.clone(),
                        fields,
                    });
            }
        }
    }
    uses
}

const CONSTRUCTOR_SIGNATURES: &[&str] =
    &["constructor_signature", "constant_constructor_signature"];

fn declaration_constructor_signature(declaration: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = declaration.walk();
    declaration
        .named_children(&mut cursor)
        .find(|child| CONSTRUCTOR_SIGNATURES.contains(&child.kind()))
}

fn constructor_qualified_name(
    class: Node<'_>,
    signature: Node<'_>,
    source: &str,
) -> Option<String> {
    let owner = class
        .child_by_field_name("name")
        .and_then(|name| name.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)?;
    let mut cursor = signature.walk();
    let mut parts = signature
        .children_by_field_name("name", &mut cursor)
        .filter(|child| matches!(child.kind(), "identifier" | "new"))
        .filter_map(|child| child.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned);
    let first = parts.next()?;
    let member = member_name_without_owner(&parts.last().unwrap_or(first), &owner);
    if member == owner || member == "new" {
        Some(owner)
    } else {
        Some(format!("{owner}.{member}"))
    }
}

fn member_name_without_owner(name: &str, owner: &str) -> String {
    name.strip_prefix(owner)
        .and_then(|rest| rest.strip_prefix('.'))
        .unwrap_or(name)
        .to_owned()
}

fn constructor_widget_params(signature: Node<'_>, source: &str) -> Vec<(String, String)> {
    let Some(parameters) = signature.child_by_field_name("parameters") else {
        return Vec::new();
    };
    let mut formal_parameters = Vec::new();
    collect_nodes(parameters, "formal_parameter", &mut formal_parameters);
    formal_parameters
        .into_iter()
        .filter_map(|param| {
            let name = formal_parameter_name(param, source)?;
            let type_name = formal_parameter_type(param, &name, source)?;
            Some((name, type_name))
        })
        .collect()
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
        .filter(|part| !matches!(*part, "required" | "covariant" | "final" | "var"))
        .next_back()?;
    Some(simple_type_name(type_name.trim_end_matches('?')))
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

fn member_fields_read_from(node: Node<'_>, object_name: &str, source: &str) -> BTreeSet<String> {
    let mut fields = BTreeSet::new();
    visit_named(node, &mut |child| {
        if let Some(field) = member_property_for_object(child, object_name, source) {
            fields.insert(field);
        }
    });
    fields
}

fn member_property_for_object(node: Node<'_>, object_name: &str, source: &str) -> Option<String> {
    if !matches!(
        node.kind(),
        "member_expression" | "null_aware_member_expression" | "assignable_expression"
    ) {
        return None;
    }
    let object = node.child_by_field_name("object")?;
    if object.utf8_text(source.as_bytes()).ok() != Some(object_name) {
        return None;
    }
    node.child_by_field_name("property")
        .and_then(|property| property.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
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

fn state_body_calls_forwarder(body: Node<'_>, forwarder_name: &str, source: &str) -> bool {
    let Ok(text) = body.utf8_text(source.as_bytes()) else {
        return false;
    };
    call_arguments_for_name(text, forwarder_name)
        .iter()
        .any(|arguments| arguments_pass_widget(arguments))
}

fn call_arguments_for_name<'source>(source: &'source str, name: &str) -> Vec<&'source str> {
    let mut arguments = Vec::new();
    let mut search_start = 0;
    while let Some(relative) = source[search_start..].find(name) {
        let name_start = search_start + relative;
        let mut open = name_start + name.len();
        while source
            .as_bytes()
            .get(open)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            open += 1;
        }
        if source.as_bytes().get(open) != Some(&b'(') {
            search_start = open;
            continue;
        }
        if let Some(close) = matching_paren(source, open) {
            arguments.push(&source[open + 1..close]);
            search_start = close + 1;
        } else {
            break;
        }
    }
    arguments
}

fn matching_paren(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (index, byte) in source.as_bytes().iter().enumerate().skip(open) {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn arguments_pass_widget(arguments: &str) -> bool {
    split_arguments(arguments).iter().any(|argument| {
        let compact = strip_whitespace(argument);
        matches!(compact.as_str(), "widget" | "oldWidget")
            || compact.ends_with(":widget")
            || compact.ends_with(":oldWidget")
    })
}

fn split_arguments(arguments: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    for (index, byte) in arguments.bytes().enumerate() {
        match byte {
            b'(' => paren_depth += 1,
            b')' => paren_depth = paren_depth.saturating_sub(1),
            b'[' => bracket_depth += 1,
            b']' => bracket_depth = bracket_depth.saturating_sub(1),
            b'{' => brace_depth += 1,
            b'}' => brace_depth = brace_depth.saturating_sub(1),
            b',' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                parts.push(arguments[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    if start < arguments.len() {
        parts.push(arguments[start..].trim());
    }
    parts
}

fn strip_whitespace(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn simple_type_name(text: &str) -> String {
    text.trim_end_matches('?')
        .rsplit('.')
        .next()
        .unwrap_or(text)
        .to_owned()
}

fn visit_named(node: Node<'_>, visitor: &mut impl FnMut(Node<'_>)) {
    visitor(node);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visit_named(child, visitor);
    }
}
