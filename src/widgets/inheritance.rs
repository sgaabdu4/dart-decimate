use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use tree_sitter::Node;

use super::params::constructor_params;
use super::reads::{widget_body_param_reads, widget_body_uses_inherited_param};
use super::resolution::{ClassKey, DeclarationResolver};
use super::{WidgetFileFacts, simple_type_name, superclass_type_text, widget_kind};

#[derive(Debug)]
pub(super) struct ProjectClassFact {
    pub(super) path: PathBuf,
    pub(super) name: String,
    pub(super) superclass: Option<String>,
    pub(super) widget_params: BTreeSet<(String, String)>,
    body_param_reads: BTreeSet<String>,
}

pub(super) fn inherited_param_uses_across_files(
    files: &[WidgetFileFacts],
    resolver: &DeclarationResolver,
) -> BTreeSet<(PathBuf, String, String)> {
    let facts = files
        .iter()
        .flat_map(|file| file.classes.iter())
        .collect::<Vec<_>>();
    let facts_by_key = facts
        .iter()
        .map(|fact| (fact.class_key(), *fact))
        .collect::<BTreeMap<_, _>>();
    let mut uses = BTreeSet::new();
    for file in files {
        collect_file_inherited_uses(file, &facts_by_key, resolver, &mut uses);
    }
    uses
}

pub(super) fn class_facts(
    path: &Path,
    classes: &[Node<'_>],
    source: &str,
) -> Vec<ProjectClassFact> {
    classes
        .iter()
        .filter_map(|class| {
            let name = class_name(*class, source)?;
            let widget_params = widget_kind(*class, source)
                .map(|_| {
                    constructor_params(*class, &name, source)
                        .into_iter()
                        .map(|param| (param.field_name, param.name))
                        .collect()
                })
                .unwrap_or_default();
            Some(ProjectClassFact {
                path: path.to_path_buf(),
                name,
                superclass: superclass_type_text(*class, source),
                widget_params,
                body_param_reads: class
                    .child_by_field_name("body")
                    .map_or_else(BTreeSet::new, |body| widget_body_param_reads(body, source)),
            })
        })
        .collect()
}

fn collect_file_inherited_uses(
    file: &WidgetFileFacts,
    facts: &BTreeMap<ClassKey, &ProjectClassFact>,
    resolver: &DeclarationResolver,
    uses: &mut BTreeSet<(PathBuf, String, String)>,
) {
    for class in &file.classes {
        let mut pending = class
            .superclass
            .iter()
            .map(|parent| (class.path.clone(), parent.clone()))
            .collect::<Vec<_>>();
        let mut visited = BTreeSet::new();
        while let Some((current_path, parent_name)) = pending.pop() {
            for ancestor_key in resolver.resolve(&current_path, &parent_name) {
                if !visited.insert(ancestor_key.clone()) {
                    continue;
                }
                let Some(ancestor) = facts.get(&ancestor_key) else {
                    continue;
                };
                for (field_name, param_name) in &ancestor.widget_params {
                    if class.body_param_reads.contains(field_name) {
                        uses.insert((
                            ancestor.path.clone(),
                            ancestor.name.clone(),
                            param_name.clone(),
                        ));
                    }
                }
                if let Some(parent) = &ancestor.superclass {
                    pending.push((ancestor.path.clone(), parent.clone()));
                }
            }
        }
    }
}

impl ProjectClassFact {
    fn class_key(&self) -> ClassKey {
        ClassKey {
            path: self.path.clone(),
            name: self.name.clone(),
        }
    }
}

pub(super) fn class_superclasses(classes: &[Node<'_>], source: &str) -> BTreeMap<String, String> {
    classes
        .iter()
        .filter_map(|class| {
            Some((
                class_name(*class, source)?,
                superclass_type_text(*class, source)?,
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
                if widget_body_uses_inherited_param(body, param, source) {
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
