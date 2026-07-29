use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use super::ParsedSourceUnit;

#[derive(Clone)]
struct ExtensionAlias {
    name: String,
    owner: Option<String>,
    declaration: usize,
    scope_start: usize,
    scope_end: usize,
}

pub(super) fn used_extension_properties(
    units: &[ParsedSourceUnit<'_>],
    extension_names: &BTreeSet<String>,
) -> BTreeSet<(String, String)> {
    let factories = extension_factories(units, extension_names);
    let mut used = BTreeSet::new();
    for parsed_unit in units {
        let parsed = &parsed_unit.parsed;
        let aliases = extension_aliases(
            parsed.tree().root_node(),
            parsed.source(),
            extension_names,
            &factories,
        );
        walk(parsed.tree().root_node(), &mut |node| {
            collect_usage(
                node,
                parsed.source(),
                extension_names,
                &factories,
                &aliases,
                &mut used,
            );
        });
    }
    used
}

fn extension_factories(
    units: &[ParsedSourceUnit<'_>],
    extension_names: &BTreeSet<String>,
) -> BTreeMap<String, String> {
    let mut candidates = BTreeMap::<String, Vec<Option<String>>>::new();
    for parsed_unit in units {
        let parsed = &parsed_unit.parsed;
        walk(parsed.tree().root_node(), &mut |node| {
            if !matches!(
                node.kind(),
                "function_declaration" | "getter_declaration" | "method_declaration"
            ) {
                return;
            }
            let Some(name) = callable_name(node, parsed.source()) else {
                return;
            };
            let mut returns = Vec::new();
            walk(node, &mut |candidate| {
                if candidate.kind() == "return_statement" && belongs_to_callable(candidate, node) {
                    returns.push(direct_extension_owner(
                        node_text(candidate, parsed.source()),
                        extension_names,
                    ));
                }
            });
            if returns.is_empty() {
                let body = first_descendant(node, "function_body")
                    .map(|body| node_text(body, parsed.source()))
                    .unwrap_or_default();
                if body.trim_start().starts_with("=>") {
                    returns.push(direct_extension_owner(body, extension_names));
                }
            }
            let key = enclosing_class_name(node, parsed.source())
                .map_or(name.clone(), |class| format!("{class}.{name}"));
            candidates
                .entry(key)
                .or_default()
                .push(one_proven_owner(&returns));
        });
    }
    candidates
        .into_iter()
        .filter_map(|(key, owners)| one_proven_owner(&owners).map(|owner| (key, owner)))
        .collect()
}

fn extension_aliases(
    root: Node<'_>,
    source: &str,
    extension_names: &BTreeSet<String>,
    factories: &BTreeMap<String, String>,
) -> Vec<ExtensionAlias> {
    let mut aliases = Vec::new();
    walk(root, &mut |node| {
        if !matches!(
            node.kind(),
            "initialized_identifier" | "initialized_variable_definition"
        ) {
            return;
        }
        let Some(name) = field_text(node, "name", source) else {
            return;
        };
        let Some(value) = node.child_by_field_name("value") else {
            return;
        };
        let owner =
            extension_owner_from_expression(node_text(value, source), extension_names, factories);
        let scope = lexical_scope(node).unwrap_or(root);
        aliases.push(ExtensionAlias {
            name,
            owner,
            declaration: node.start_byte(),
            scope_start: scope.start_byte(),
            scope_end: scope.end_byte(),
        });
    });
    aliases.sort_by_key(|alias| (alias.declaration, alias.scope_start, alias.scope_end));
    aliases
}

fn collect_usage(
    node: Node<'_>,
    source: &str,
    extension_names: &BTreeSet<String>,
    factories: &BTreeMap<String, String>,
    aliases: &[ExtensionAlias],
    used: &mut BTreeSet<(String, String)>,
) {
    if !matches!(
        node.kind(),
        "member_expression" | "null_aware_member_expression"
    ) || has_extension_class_ancestor(node, source, extension_names)
    {
        return;
    }
    let Some(property) = field_text(node, "property", source) else {
        return;
    };
    let Some(receiver) = field_text(node, "object", source) else {
        return;
    };
    let owner =
        extension_owner_from_expression(&receiver, extension_names, factories).or_else(|| {
            let receiver = receiver.trim();
            is_identifier(receiver).then(|| {
                aliases
                    .iter()
                    .rev()
                    .find(|alias| {
                        alias.name == receiver
                            && alias.declaration < node.start_byte()
                            && alias.scope_start <= node.start_byte()
                            && alias.scope_end >= node.end_byte()
                    })
                    .and_then(|alias| alias.owner.clone())
            })?
        });
    if let Some(owner) = owner {
        used.insert((owner, property));
    }
}

fn extension_owner_from_expression(
    expression: &str,
    extension_names: &BTreeSet<String>,
    factories: &BTreeMap<String, String>,
) -> Option<String> {
    if let Some(owner) = direct_extension_owner(expression, extension_names) {
        return Some(owner);
    }
    let compact = expression.split_whitespace().collect::<String>();
    let owners = factories
        .iter()
        .filter(|(factory, _)| contains_call(&compact, factory))
        .map(|(_, owner)| owner.clone())
        .collect::<BTreeSet<_>>();
    (owners.len() == 1)
        .then(|| owners.into_iter().next())
        .flatten()
}

fn direct_extension_owner(expression: &str, extension_names: &BTreeSet<String>) -> Option<String> {
    let compact = expression.split_whitespace().collect::<String>();
    let owners = compact
        .match_indices("extension<")
        .filter_map(|(index, _)| {
            let rest = compact.get(index + "extension<".len()..)?;
            let generic = rest.split('>').next()?;
            let owner = generic.rsplit('.').next()?;
            extension_names.contains(owner).then(|| owner.to_owned())
        })
        .collect::<BTreeSet<_>>();
    (owners.len() == 1)
        .then(|| owners.into_iter().next())
        .flatten()
}

fn one_proven_owner(returns: &[Option<String>]) -> Option<String> {
    if returns.is_empty() || returns.iter().any(Option::is_none) {
        return None;
    }
    let owners = returns.iter().flatten().cloned().collect::<BTreeSet<_>>();
    (owners.len() == 1)
        .then(|| owners.into_iter().next())
        .flatten()
}

fn contains_call(expression: &str, callee: &str) -> bool {
    let mut offset = 0usize;
    while let Some(relative) = expression.get(offset..).and_then(|rest| rest.find(callee)) {
        let index = offset + relative;
        let before = index
            .checked_sub(1)
            .and_then(|position| expression.as_bytes().get(position));
        let after = expression.as_bytes().get(index + callee.len());
        if before.is_none_or(|byte| !is_identifier_byte(*byte)) && after == Some(&b'(') {
            return true;
        }
        offset = index + callee.len();
    }
    false
}

fn lexical_scope(mut node: Node<'_>) -> Option<Node<'_>> {
    while let Some(parent) = node.parent() {
        if matches!(
            parent.kind(),
            "block"
                | "catch_clause"
                | "class_body"
                | "compilation_unit"
                | "do_statement"
                | "for_element"
                | "for_statement"
                | "if_element"
                | "if_statement"
                | "switch_expression_case"
                | "switch_statement_case"
                | "while_statement"
        ) {
            return Some(parent);
        }
        node = parent;
    }
    None
}

fn belongs_to_callable(mut node: Node<'_>, callable: Node<'_>) -> bool {
    while let Some(parent) = node.parent() {
        if parent.id() == callable.id() {
            return true;
        }
        if matches!(
            parent.kind(),
            "function_declaration"
                | "function_expression"
                | "getter_declaration"
                | "method_declaration"
        ) {
            return false;
        }
        node = parent;
    }
    false
}

fn enclosing_class_name(mut node: Node<'_>, source: &str) -> Option<String> {
    while let Some(parent) = node.parent() {
        if parent.kind() == "class_declaration" {
            return field_text(parent, "name", source);
        }
        node = parent;
    }
    None
}

fn callable_name(node: Node<'_>, source: &str) -> Option<String> {
    if matches!(
        node.kind(),
        "function_signature" | "getter_signature" | "setter_signature"
    ) {
        return field_text(node, "name", source);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(|child| callable_name(child, source))
}

fn first_descendant<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    if node.kind() == kind {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(|child| first_descendant(child, kind))
}

fn has_extension_class_ancestor(
    mut node: Node<'_>,
    source: &str,
    extension_names: &BTreeSet<String>,
) -> bool {
    while let Some(parent) = node.parent() {
        if parent.kind() == "class_declaration" {
            return field_text(parent, "name", source)
                .is_some_and(|name| extension_names.contains(&name));
        }
        node = parent;
    }
    false
}

fn field_text(node: Node<'_>, field: &str, source: &str) -> Option<String> {
    node.child_by_field_name(field)
        .and_then(|child| child.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn node_text<'source>(node: Node<'_>, source: &'source str) -> &'source str {
    node.utf8_text(source.as_bytes()).unwrap_or_default()
}

fn is_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character.is_ascii_alphabetic())
        && characters.all(|character| {
            character == '_' || character == '$' || character.is_ascii_alphanumeric()
        })
}

fn is_identifier_byte(byte: u8) -> bool {
    byte == b'_' || byte == b'$' || byte.is_ascii_alphanumeric()
}

fn walk(node: Node<'_>, visit: &mut impl FnMut(Node<'_>)) {
    visit(node);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk(child, visit);
    }
}
