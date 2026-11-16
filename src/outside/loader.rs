use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use rayon::prelude::*;

use crate::DartFile;
use crate::extract::extract_dart_outline;

pub(crate) enum Outline {
    Missing,
    Unparsed,
    Skipped,
    Read(Rc<DartFile>),
}

pub(crate) type SharedLoader = Rc<RefCell<OutlineLoader>>;

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

struct Built {
    forwards: bool,
    type_names: BTreeSet<String>,
    parsed: Option<Result<DartFile, ()>>,
}

impl OutlineLoader {
    pub(crate) fn prefetch(&mut self, paths: &[PathBuf], wanted: &str) {
        let missing = paths
            .iter()
            .filter(|path| !self.entries.contains_key(*path))
            .collect::<Vec<_>>();
        let built = missing
            .par_iter()
            .map(|path| (*path, build(path, wanted)))
            .collect::<Vec<_>>();
        for (path, built) in built {
            self.entries.insert(path.clone(), entry_from(built));
        }
    }

    pub(crate) fn load(&mut self, path: &Path, wanted: &str) -> Outline {
        if !self.entries.contains_key(path) {
            self.entries
                .insert(path.to_path_buf(), entry_from(build(path, wanted)));
        }
        let Some(Entry::Scanned(scanned)) = self.entries.get_mut(path) else {
            return Outline::Missing;
        };
        if scanned.parsed.is_none() {
            if !scanned.forwards && !scanned.type_names.contains(wanted) {
                return Outline::Skipped;
            }
            let Ok(source) = fs::read_to_string(path) else {
                return Outline::Missing;
            };
            scanned.parsed = Some(parse(path, &source).map(Rc::new));
        }
        match &scanned.parsed {
            Some(Ok(file)) => Outline::Read(Rc::clone(file)),
            _ => Outline::Unparsed,
        }
    }
}

fn build(path: &Path, wanted: &str) -> Option<Built> {
    let source = fs::read_to_string(path).ok()?;
    let forwards = forwards_libraries(&source);
    let type_names = declared_type_names(&source);
    let parsed = (forwards || type_names.contains(wanted)).then(|| parse(path, &source));
    Some(Built {
        forwards,
        type_names,
        parsed,
    })
}

fn entry_from(built: Option<Built>) -> Entry {
    built.map_or(Entry::Missing, |built| {
        Entry::Scanned(Scanned {
            forwards: built.forwards,
            type_names: built.type_names,
            parsed: built.parsed.map(|parsed| parsed.map(Rc::new)),
        })
    })
}

fn parse(path: &Path, source: &str) -> Result<DartFile, ()> {
    extract_dart_outline(path, source).map_err(|_| ())
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
