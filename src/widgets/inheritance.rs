use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use tree_sitter::Node;

use super::params::constructor_params;
use super::patterns::class_member_names;
use super::reads::{WidgetMemberRead, WidgetMemberReceiver, widget_body_member_reads};
use super::resolution::{ClassKey, DeclarationResolver};
use super::{WidgetFileFacts, simple_type_name, superclass_type_text, widget_kind};

#[derive(Debug)]
pub(super) struct ProjectClassFact {
    pub(super) path: PathBuf,
    pub(super) name: String,
    pub(super) superclass: Option<String>,
    pub(super) widget_params: BTreeSet<(String, String)>,
    member_names: BTreeSet<String>,
    body_member_reads: BTreeSet<WidgetMemberRead>,
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

pub(super) fn flutter_framework_classes_across_files(
    files: &[WidgetFileFacts],
    resolver: &DeclarationResolver,
) -> BTreeSet<(PathBuf, String)> {
    let facts = files
        .iter()
        .flat_map(|file| file.classes.iter())
        .collect::<Vec<_>>();
    let mut framework = facts
        .iter()
        .filter(|fact| {
            fact.superclass
                .as_deref()
                .is_some_and(is_flutter_framework_base)
        })
        .map(|fact| fact.class_key())
        .collect::<BTreeSet<_>>();

    loop {
        let mut changed = false;
        for fact in &facts {
            let key = fact.class_key();
            if framework.contains(&key) {
                continue;
            }
            let Some(parent) = fact.superclass.as_deref() else {
                continue;
            };
            let reference = parent.split('<').next().unwrap_or(parent).trim();
            let ancestors = resolver.resolve(&fact.path, reference);
            if !ancestors.is_empty()
                && ancestors
                    .iter()
                    .all(|ancestor| framework.contains(ancestor))
            {
                changed |= framework.insert(key);
            }
        }
        if !changed {
            break;
        }
    }

    framework
        .into_iter()
        .map(|class| (class.path, class.name))
        .collect()
}

fn is_flutter_framework_base(superclass: &str) -> bool {
    let base = superclass.split('<').next().unwrap_or(superclass);
    matches!(
        simple_type_name(base).as_str(),
        "AnimatedWidget"
            | "StatelessWidget"
            | "ConsumerWidget"
            | "HookWidget"
            | "HookConsumerWidget"
            | "State"
            | "ConsumerState"
    )
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
                member_names: class_member_names(*class, source),
                body_member_reads: class
                    .child_by_field_name("body")
                    .map_or_else(BTreeSet::new, |body| widget_body_member_reads(body, source)),
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
                    if class.body_member_reads.iter().any(|read| {
                        read.name == *field_name
                            && member_owner_for_read(class, read, facts, resolver)
                                .is_some_and(|owner| owner == ancestor_key)
                    }) {
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
    let facts = class_facts(Path::new(""), classes, source);
    let facts_by_name = facts
        .iter()
        .map(|fact| (fact.name.clone(), fact))
        .collect::<BTreeMap<_, _>>();
    let mut uses = BTreeSet::new();

    for fact in &facts {
        for read in &fact.body_member_reads {
            let Some(owner) = local_member_owner(&fact.name, read, &facts_by_name, &superclasses)
            else {
                continue;
            };
            if owner == fact.name {
                continue;
            }
            let Some(owner_fact) = facts_by_name.get(&owner) else {
                continue;
            };
            for (field_name, param_name) in &owner_fact.widget_params {
                if field_name == &read.name {
                    uses.insert((owner.clone(), param_name.clone()));
                }
            }
        }
    }

    uses
}

fn class_name(class: Node<'_>, source: &str) -> Option<String> {
    class
        .child_by_field_name("name")?
        .utf8_text(source.as_bytes())
        .ok()
        .map(simple_type_name)
}

fn member_owner_for_read(
    class: &ProjectClassFact,
    read: &WidgetMemberRead,
    facts: &BTreeMap<ClassKey, &ProjectClassFact>,
    resolver: &DeclarationResolver,
) -> Option<ClassKey> {
    let start = match read.receiver {
        WidgetMemberReceiver::Implicit | WidgetMemberReceiver::This => class.class_key(),
        WidgetMemberReceiver::Super => resolved_superclass(class, facts, resolver)?,
    };
    resolved_member_owner(start, &read.name, facts, resolver, &mut BTreeSet::new())
}

fn resolved_superclass(
    class: &ProjectClassFact,
    facts: &BTreeMap<ClassKey, &ProjectClassFact>,
    resolver: &DeclarationResolver,
) -> Option<ClassKey> {
    let parent = class.superclass.as_deref()?;
    let candidates = resolver.resolve(&class.path, parent);
    let key = candidates.first()?.clone();
    (candidates.len() == 1 && facts.contains_key(&key)).then_some(key)
}

fn resolved_member_owner(
    class_key: ClassKey,
    name: &str,
    facts: &BTreeMap<ClassKey, &ProjectClassFact>,
    resolver: &DeclarationResolver,
    visited: &mut BTreeSet<ClassKey>,
) -> Option<ClassKey> {
    if !visited.insert(class_key.clone()) {
        return None;
    }
    let class = facts.get(&class_key)?;
    if class.member_names.contains(name) {
        return Some(class_key);
    }
    let parent = class.superclass.as_deref()?;
    let candidates = resolver.resolve(&class.path, parent);
    let key = candidates.first()?.clone();
    (candidates.len() == 1).then(|| resolved_member_owner(key, name, facts, resolver, visited))?
}

fn local_member_owner(
    class_name: &str,
    read: &WidgetMemberRead,
    facts: &BTreeMap<String, &ProjectClassFact>,
    superclasses: &BTreeMap<String, String>,
) -> Option<String> {
    let start = match read.receiver {
        WidgetMemberReceiver::Implicit | WidgetMemberReceiver::This => class_name.to_owned(),
        WidgetMemberReceiver::Super => simple_type_name(superclasses.get(class_name)?),
    };
    local_member_owner_from(&start, &read.name, facts, &mut BTreeSet::new())
}

fn local_member_owner_from(
    class_name: &str,
    name: &str,
    facts: &BTreeMap<String, &ProjectClassFact>,
    visited: &mut BTreeSet<String>,
) -> Option<String> {
    if !visited.insert(class_name.to_owned()) {
        return None;
    }
    let class = facts.get(class_name)?;
    if class.member_names.contains(name) {
        return Some(class_name.to_owned());
    }
    let parent = simple_type_name(class.superclass.as_deref()?);
    local_member_owner_from(&parent, name, facts, visited)
}
