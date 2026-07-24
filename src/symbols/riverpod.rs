use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use super::{IndexedDeclaration, IndexedReference};

pub(super) fn extend_generated_provider_owner_references(
    references_by_name: &mut BTreeMap<String, Vec<IndexedReference>>,
    declarations: &[IndexedDeclaration],
) {
    let mut source_cache = BTreeMap::<PathBuf, Option<String>>::new();
    for declaration in declarations {
        if !has_riverpod_annotation(&mut source_cache, declaration) {
            continue;
        }

        let provider_references = riverpod_provider_names(&declaration.declaration.name)
            .iter()
            .filter_map(|provider_name| references_by_name.get(provider_name))
            .flatten()
            .cloned()
            .collect::<Vec<_>>();
        if provider_references.is_empty() {
            continue;
        }

        references_by_name
            .entry(declaration.declaration.name.clone())
            .or_default()
            .extend(provider_references);
    }
}

fn has_riverpod_annotation(
    source_cache: &mut BTreeMap<PathBuf, Option<String>>,
    declaration: &IndexedDeclaration,
) -> bool {
    let source = source_cache
        .entry(declaration.path.clone())
        .or_insert_with(|| fs::read_to_string(&declaration.path).ok());
    let Some(source) = source.as_deref() else {
        return false;
    };

    let window = annotation_window(source, declaration.declaration.location.line);
    window.contains("@riverpod") || window.contains("@Riverpod")
}

fn annotation_window(source: &str, declaration_line: usize) -> String {
    let lines = source.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return String::new();
    }

    let end = declaration_line.saturating_sub(1).min(lines.len() - 1);
    let start = end.saturating_sub(32);
    lines[start..=end].join("\n")
}

fn riverpod_provider_names(declaration_name: &str) -> Vec<String> {
    let mut source_names = vec![declaration_name];
    if let Some(stripped) = declaration_name.strip_suffix("Notifier")
        && !stripped.is_empty()
    {
        source_names.push(stripped);
    }
    source_names
        .into_iter()
        .flat_map(lower_camel_candidates)
        .map(|name| format!("{name}Provider"))
        .collect()
}

fn lower_camel_candidates(name: &str) -> Vec<String> {
    let mut candidates = BTreeSet::new();
    if let Some(lower_first) = lower_first_character(name) {
        candidates.insert(lower_first);
    }
    if let Some(lower_acronym) = lower_initial_acronym(name) {
        candidates.insert(lower_acronym);
    }
    candidates.into_iter().collect()
}

fn lower_first_character(name: &str) -> Option<String> {
    let mut characters = name.chars();
    let first = characters.next()?;
    Some(first.to_lowercase().chain(characters).collect::<String>())
}

fn lower_initial_acronym(name: &str) -> Option<String> {
    let mut chars = name.char_indices();
    let (_, first) = chars.next()?;
    if !first.is_uppercase() {
        return Some(name.to_owned());
    }

    let mut previous_upper_byte = 0;
    let mut split = name.len();
    for (byte, current) in chars {
        if current.is_lowercase() {
            split = previous_upper_byte;
            break;
        }
        if !current.is_uppercase() {
            split = byte;
            break;
        }
        previous_upper_byte = byte;
    }

    if split == 0 {
        return lower_first_character(name);
    }
    Some(format!(
        "{}{}",
        name[..split].to_ascii_lowercase(),
        &name[split..]
    ))
}
