use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use tree_sitter::Node;

use super::params::constructor_params;
use super::reads::widget_body_uses_param;
use super::{
    WidgetAnalysisError, collect_class_declarations, parse_tree, simple_type_name,
    superclass_base_name, widget_kind,
};
use crate::ScannedProject;
use crate::graph::normalize_against;

#[derive(Debug)]
struct ProjectClassFact {
    path: PathBuf,
    name: String,
    superclass: Option<String>,
    widget_params: BTreeSet<String>,
}

pub(super) fn inherited_param_uses_across_files(
    project: &ScannedProject,
    paths: &[PathBuf],
) -> Result<BTreeSet<(PathBuf, String, String)>, WidgetAnalysisError> {
    let mut facts = Vec::new();
    for path in paths {
        facts.extend(class_facts(path)?);
    }
    let dependencies = project
        .graph
        .dependencies()
        .into_iter()
        .map(|dependency| {
            (
                normalize_against(&project.root, &dependency.from_path),
                normalize_against(&project.root, &dependency.to_path),
            )
        })
        .collect::<BTreeSet<_>>();
    let mut uses = BTreeSet::new();
    for path in paths {
        collect_file_inherited_uses(path, &facts, &dependencies, &mut uses)?;
    }
    Ok(uses)
}

fn class_facts(path: &Path) -> Result<Vec<ProjectClassFact>, WidgetAnalysisError> {
    let source = fs::read_to_string(path).map_err(|source| WidgetAnalysisError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let parsed = parse_tree(path, &source)?;
    let mut classes = Vec::new();
    collect_class_declarations(parsed.tree().root_node(), &mut classes);
    Ok(classes
        .into_iter()
        .filter_map(|class| {
            let name = class_name(class, parsed.source())?;
            let widget_params = widget_kind(class, parsed.source())
                .map(|_| {
                    constructor_params(class, &name, parsed.source())
                        .into_iter()
                        .map(|param| param.field_name)
                        .collect()
                })
                .unwrap_or_default();
            Some(ProjectClassFact {
                path: path.to_path_buf(),
                name,
                superclass: superclass_base_name(class, parsed.source()),
                widget_params,
            })
        })
        .collect())
}

fn collect_file_inherited_uses(
    path: &Path,
    facts: &[ProjectClassFact],
    dependencies: &BTreeSet<(PathBuf, PathBuf)>,
    uses: &mut BTreeSet<(PathBuf, String, String)>,
) -> Result<(), WidgetAnalysisError> {
    let source = fs::read_to_string(path).map_err(|source| WidgetAnalysisError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let parsed = parse_tree(path, &source)?;
    let mut classes = Vec::new();
    collect_class_declarations(parsed.tree().root_node(), &mut classes);
    for class in classes {
        let Some(body) = class.child_by_field_name("body") else {
            continue;
        };
        let mut current_path = path;
        let mut parent = superclass_base_name(class, parsed.source());
        let mut visited = BTreeSet::new();
        while let Some(parent_name) = parent.as_deref() {
            let Some(ancestor) = resolve_parent(current_path, parent_name, facts, dependencies)
            else {
                break;
            };
            if !visited.insert((ancestor.path.clone(), ancestor.name.clone())) {
                break;
            }
            for param in &ancestor.widget_params {
                if widget_body_uses_param(body, param, parsed.source()) {
                    uses.insert((ancestor.path.clone(), ancestor.name.clone(), param.clone()));
                }
            }
            current_path = &ancestor.path;
            parent.clone_from(&ancestor.superclass);
        }
    }
    Ok(())
}

fn resolve_parent<'a>(
    from: &Path,
    name: &str,
    facts: &'a [ProjectClassFact],
    dependencies: &BTreeSet<(PathBuf, PathBuf)>,
) -> Option<&'a ProjectClassFact> {
    let mut candidates = facts.iter().filter(|fact| {
        fact.name == name
            && (fact.path == from
                || dependencies.contains(&(from.to_path_buf(), fact.path.clone())))
    });
    let candidate = candidates.next()?;
    candidates.next().is_none().then_some(candidate)
}

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
