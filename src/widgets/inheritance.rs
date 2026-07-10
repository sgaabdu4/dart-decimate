use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::Node;

use super::params::constructor_params;
use super::reads::widget_body_uses_param;
use super::{simple_type_name, superclass_base_name, widget_kind};

pub(super) fn class_superclasses(classes: &[Node<'_>], source: &str) -> BTreeMap<String, String> {
    classes
        .iter()
        .filter_map(|class| {
            Some((
                class_name(*class, source)?,
                superclass_base_name(*class, source)?,
            ))
        })
        .collect()
}

pub(super) fn inherited_param_uses(
    classes: &[Node<'_>],
    source: &str,
) -> BTreeSet<(String, String)> {
    let superclasses = class_superclasses(classes, source);
    let widget_params = classes
        .iter()
        .filter(|class| widget_kind(**class, source).is_some())
        .filter_map(|class| {
            let name = class_name(*class, source)?;
            let params = constructor_params(*class, &name, source)
                .into_iter()
                .map(|param| param.field_name)
                .collect::<BTreeSet<_>>();
            Some((name, params))
        })
        .collect::<BTreeMap<_, _>>();
    let mut uses = BTreeSet::new();

    for class in classes {
        let Some(descendant) = class_name(*class, source) else {
            continue;
        };
        let Some(body) = class.child_by_field_name("body") else {
            continue;
        };
        for ancestor in ancestors(&descendant, &superclasses) {
            let Some(params) = widget_params.get(&ancestor) else {
                continue;
            };
            for param in params {
                if widget_body_uses_param(body, param, source) {
                    uses.insert((ancestor.clone(), param.clone()));
                }
            }
        }
    }

    uses
}

fn ancestors(class: &str, superclasses: &BTreeMap<String, String>) -> Vec<String> {
    let mut ancestors = Vec::new();
    let mut visited = BTreeSet::new();
    let mut current = class;
    while let Some(parent) = superclasses.get(current) {
        if !visited.insert(parent.clone()) {
            break;
        }
        ancestors.push(parent.clone());
        current = parent;
    }
    ancestors
}

fn class_name(class: Node<'_>, source: &str) -> Option<String> {
    class
        .child_by_field_name("name")?
        .utf8_text(source.as_bytes())
        .ok()
        .map(simple_type_name)
}
