use std::path::Path;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct DependencyUsage {
    pub(super) production: bool,
    pub(super) development: bool,
}

impl DependencyUsage {
    pub(super) fn record(&mut self, package_root: &Path, path: &Path) {
        if allows_dev_dependency(package_root, path) {
            self.development = true;
        } else {
            self.production = true;
        }
    }

    pub(super) fn record_tooling(&mut self) {
        self.development = true;
    }

    pub(super) const fn any(self) -> bool {
        self.production || self.development
    }
}

pub(super) fn allows_dev_dependency(package_root: &Path, path: &Path) -> bool {
    let relative = path.strip_prefix(package_root).unwrap_or(path);
    !matches!(
        relative.components().next(),
        Some(std::path::Component::Normal(name))
            if matches!(
                name.to_str(),
                Some("lib" | "bin" | "web" | "hook")
            )
    )
}
