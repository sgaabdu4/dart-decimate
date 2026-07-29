use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

static TRACKED_PARSE_COUNTS: OnceLock<Mutex<BTreeMap<PathBuf, usize>>> = OnceLock::new();

pub(crate) struct ParseCountGuard {
    path: PathBuf,
}

impl ParseCountGuard {
    pub(crate) fn count(&self) -> usize {
        tracked_parse_counts_lock()
            .get(&self.path)
            .copied()
            .unwrap_or(0)
    }
}

impl Drop for ParseCountGuard {
    fn drop(&mut self) {
        tracked_parse_counts_lock().remove(&self.path);
    }
}

pub(crate) fn track_parse_count(path: &Path) -> ParseCountGuard {
    let path = path.to_path_buf();
    let previous = tracked_parse_counts_lock().insert(path.clone(), 0);
    assert!(
        previous.is_none(),
        "parse count already tracked for {path:?}"
    );
    ParseCountGuard { path }
}

pub(super) fn record_parse(path: &Path) {
    if let Some(count) = tracked_parse_counts_lock().get_mut(path) {
        *count += 1;
    }
}

fn tracked_parse_counts() -> &'static Mutex<BTreeMap<PathBuf, usize>> {
    TRACKED_PARSE_COUNTS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn tracked_parse_counts_lock() -> std::sync::MutexGuard<'static, BTreeMap<PathBuf, usize>> {
    match tracked_parse_counts().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
