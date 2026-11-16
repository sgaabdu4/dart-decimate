use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::DartFile;
use crate::extract::extract_dart_outline;

pub(crate) enum Outline {
    Missing,
    Unparsed,
    Skipped,
    Read(Rc<DartFile>),
}

#[derive(Default)]
pub(crate) struct OutlineLoader {
    entries: BTreeMap<PathBuf, Entry>,
}

enum Entry {
    Missing,
    Scanned(Scanned),
}

struct Scanned {
    forwards: bool,
    type_names: BTreeSet<String>,
    parsed: Option<Result<Rc<DartFile>, ()>>,
}

impl OutlineLoader {
    pub(crate) fn load(&mut self, path: &Path, wanted: &str) -> Outline {
        let mut text = None;
        if !self.entries.contains_key(path) {
            let entry = match fs::read_to_string(path) {
                Ok(source) => {
                    let scanned = Scanned {
                        forwards: forwards_libraries(&source),
                        type_names: declared_type_names(&source),
                        parsed: None,
                    };
                    text = Some(source);
                    Entry::Scanned(scanned)
                }
                Err(_) => Entry::Missing,
            };
            self.entries.insert(path.to_path_buf(), entry);
        }
        let Some(Entry::Scanned(scanned)) = self.entries.get_mut(path) else {
            return Outline::Missing;
        };
        if scanned.parsed.is_none() {
            if !scanned.forwards && !scanned.type_names.contains(wanted) {
                return Outline::Skipped;
            }
            let Some(source) = text.or_else(|| fs::read_to_string(path).ok()) else {
                return Outline::Missing;
            };
            scanned.parsed = Some(
                extract_dart_outline(path, &source)
                    .map(Rc::new)
                    .map_err(|_| ()),
            );
        }
        match &scanned.parsed {
            Some(Ok(file)) => Outline::Read(Rc::clone(file)),
            _ => Outline::Unparsed,
        }
    }
}

fn forwards_libraries(source: &str) -> bool {
    source.lines().any(|line| {
        let line = line.trim_start();
        ["export", "part"].iter().any(|keyword| {
            line.strip_prefix(keyword).is_some_and(|rest| {
                rest.trim_start().starts_with(['\'', '"']) && rest.starts_with(char::is_whitespace)
            })
        })
    })
}

fn declared_type_names(source: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for keyword in ["class", "mixin", "typedef"] {
        for (index, _) in source.match_indices(keyword) {
            let rest = source[index + keyword.len()..].trim_start();
            let end = rest
                .find(|character: char| !(character.is_alphanumeric() || "_$".contains(character)))
                .unwrap_or(rest.len());
            if end > 0 {
                names.insert(rest[..end].to_owned());
            }
        }
    }
    names
}
