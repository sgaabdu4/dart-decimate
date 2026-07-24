use std::path::PathBuf;

use crate::graph::normalize_against;
use crate::{DeclarationKind, DependencyKind, MemberKind, ScannedProject, SourceRange};

use super::{CodeClone, CodeCloneInstance};

pub(super) struct MappingBoundaryFilter {
    boundaries: Vec<MappingBoundary>,
}

struct MappingBoundary {
    model_path: PathBuf,
    model_range: SourceRange,
    entity_path: PathBuf,
    entity_range: SourceRange,
}

impl MappingBoundaryFilter {
    pub(super) fn new(project: &ScannedProject) -> Self {
        let dependencies = project.graph.dependencies();
        let mut boundaries = Vec::new();

        for model_file in &project.files {
            let model_path = normalize_against(&project.root, &model_file.path);
            for member in model_file.members.iter().filter(|member| {
                member.kind == MemberKind::Method
                    && member.name == "toEntity"
                    && member.owner.ends_with("Model")
            }) {
                let Some(model) = model_file.declarations.iter().find(|declaration| {
                    declaration.kind == DeclarationKind::Class && declaration.name == member.owner
                }) else {
                    continue;
                };
                let Some(prefix) = model.name.strip_suffix("Model") else {
                    continue;
                };
                let entity_name = format!("{prefix}Entity");

                for dependency in dependencies.iter().filter(|dependency| {
                    dependency.kind == DependencyKind::Import && dependency.from_path == model_path
                }) {
                    let Some(entity_file) = project
                        .files
                        .iter()
                        .find(|file| file.path == dependency.to_path)
                    else {
                        continue;
                    };
                    let Some(entity) = entity_file.declarations.iter().find(|declaration| {
                        declaration.kind == DeclarationKind::Class
                            && declaration.name == entity_name
                    }) else {
                        continue;
                    };
                    if !model_file
                        .references
                        .iter()
                        .any(|reference| reference.name == entity_name)
                    {
                        continue;
                    }
                    boundaries.push(MappingBoundary {
                        model_path: model_path.clone(),
                        model_range: model.range,
                        entity_path: dependency.to_path.clone(),
                        entity_range: entity.range,
                    });
                }
            }
        }

        Self { boundaries }
    }

    pub(super) fn is_mapping_boundary_clone(&self, group: &CodeClone) -> bool {
        self.boundaries.iter().any(|boundary| {
            let mut saw_model = false;
            let mut saw_entity = false;
            let all_inside_pair = group.instances.iter().all(|instance| {
                if instance_inside(instance, &boundary.model_path, boundary.model_range) {
                    saw_model = true;
                    return true;
                }
                if instance_inside(instance, &boundary.entity_path, boundary.entity_range) {
                    saw_entity = true;
                    return true;
                }
                false
            });
            all_inside_pair && saw_model && saw_entity
        })
    }
}

fn instance_inside(instance: &CodeCloneInstance, path: &PathBuf, range: SourceRange) -> bool {
    instance.path == *path
        && instance.start_line >= range.start_line
        && instance.end_line <= range.end_line
}
