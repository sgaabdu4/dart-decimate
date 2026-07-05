use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WidgetParamForwarder {
    name: String,
    parameter: ForwardedParameter,
    fields: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ForwardedParameter {
    name: String,
    positional_index: Option<usize>,
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
            && state_bodies.iter().any(|state_body| {
                state_body_calls_forwarder(
                    *state_body,
                    &forwarder.name,
                    &forwarder.parameter,
                    source,
                )
            })
    })
}

pub(super) fn widget_forwarded_param_uses(
    classes: &[Node<'_>],
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
            for parameter in constructor_widget_params(signature, source) {
                let fields = member_fields_read_from(declaration, &parameter.name, source);
                if fields.is_empty() {
                    continue;
                }
                uses.entry(parameter.widget_class.clone())
                    .or_default()
                    .push(WidgetParamForwarder {
                        name: name.clone(),
                        parameter: ForwardedParameter {
                            name: parameter.name,
                            positional_index: parameter.positional_index,
                        },
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

struct ConstructorWidgetParam {
    name: String,
    widget_class: String,
    positional_index: Option<usize>,
}

fn constructor_widget_params(signature: Node<'_>, source: &str) -> Vec<ConstructorWidgetParam> {
    let Some(parameters) = signature.child_by_field_name("parameters") else {
        return Vec::new();
    };
    let mut formal_parameters = Vec::new();
    collect_nodes(parameters, "formal_parameter", &mut formal_parameters);
    let mut positional_index = 0usize;
    let mut widget_params = Vec::new();
    for param in formal_parameters {
        let is_named = is_named_parameter(param, source);
        let current_positional_index = (!is_named).then_some(positional_index);
        if !is_named {
            positional_index += 1;
        }
        let Some(name) = formal_parameter_name(param, source) else {
            continue;
        };
        let Some(widget_class) = formal_parameter_type(param, &name, source) else {
            continue;
        };
        widget_params.push(ConstructorWidgetParam {
            name,
            widget_class,
            positional_index: current_positional_index,
        });
    }
    widget_params
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

fn state_body_calls_forwarder(
    body: Node<'_>,
    forwarder_name: &str,
    parameter: &ForwardedParameter,
    source: &str,
) -> bool {
    let mut found = false;
    visit_named(body, &mut |node| {
        if found {
            return;
        }
        if invocation_name(node, source).as_deref() == Some(forwarder_name)
            && invocation_arguments_pass_widget(node, parameter, source)
        {
            found = true;
        }
    });
    found
}

fn invocation_name(node: Node<'_>, source: &str) -> Option<String> {
    if !matches!(
        node.kind(),
        "call_expression" | "constructor_invocation" | "const_object_expression" | "new_expression"
    ) {
        return None;
    }
    if let Some(name) = constructor_invocation_name(node, source) {
        return Some(name);
    }
    let arguments = node.child_by_field_name("arguments")?;
    source
        .get(node.start_byte()..arguments.start_byte())
        .map(normalized_invocation_prefix)
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

fn invocation_arguments_pass_widget(
    node: Node<'_>,
    parameter: &ForwardedParameter,
    source: &str,
) -> bool {
    let Some(arguments) = node.child_by_field_name("arguments") else {
        return false;
    };
    let mut positional_index = 0usize;
    let mut cursor = arguments.walk();
    for argument in arguments.named_children(&mut cursor) {
        if argument.kind() == "named_argument" {
            if named_argument_label(argument, source).as_deref() == Some(parameter.name.as_str())
                && named_argument_value(argument)
                    .is_some_and(|value| is_widget_argument(value, source))
            {
                return true;
            }
            continue;
        }
        if parameter.positional_index == Some(positional_index)
            && is_widget_argument(argument, source)
        {
            return true;
        }
        positional_index += 1;
    }
    false
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

fn is_widget_argument(argument: Node<'_>, source: &str) -> bool {
    argument
        .utf8_text(source.as_bytes())
        .ok()
        .is_some_and(|text| matches!(strip_whitespace(text).as_str(), "widget" | "oldWidget"))
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
