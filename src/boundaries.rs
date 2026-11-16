use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::generated::is_generated_dart_path;
use crate::graph::normalize_against;
use crate::symbols::path_filters::is_library_source;
use crate::{BoundaryRule, Location, ScannedProject};

/// Built-in architecture boundary presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BoundaryPreset {
    /// Classic inward-only layers: domain/data/application cannot depend outward.
    Layered,
    /// Hexagonal architecture: domain/application stay independent of adapters.
    Hexagonal,
    /// Feature-sliced layer order: shared -> entities -> features -> widgets -> pages -> app.
    FeatureSliced,
    /// Feature-first app layout with shared/core kept independent of feature code.
    Bulletproof,
}

impl BoundaryPreset {
    /// Preset identifier used in config and JSON output.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Layered => "layered",
            Self::Hexagonal => "hexagonal",
            Self::FeatureSliced => "feature-sliced",
            Self::Bulletproof => "bulletproof",
        }
    }
}

/// A Dart library file not covered by any configured architecture boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryCoverageGap {
    /// File outside every configured boundary path.
    pub path: PathBuf,
    /// Boundary paths Dart Decimate considered as architecture zones.
    pub configured_boundaries: Vec<PathBuf>,
    /// Location used for report anchoring.
    pub location: Location,
}

/// Architecture boundary inventory for `dart-decimate list --boundaries`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryInventory {
    /// Whether any boundary rules are configured.
    pub configured: bool,
    /// Built-in presets contributing boundary rules.
    pub presets: Vec<BoundaryPreset>,
    /// Root-relative globs exempt from boundary coverage gaps.
    pub allow_unmatched: Vec<String>,
    /// Directory zones inferred from configured rules.
    pub zones: Vec<BoundaryZone>,
    /// Configured access rules.
    pub rules: Vec<BoundaryAccessRule>,
    /// Dart library files outside every configured zone.
    pub uncovered_files: Vec<PathBuf>,
}

/// One configured boundary zone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryZone {
    /// Boundary path.
    pub path: PathBuf,
    /// Dart library files under this boundary path.
    pub file_count: usize,
}

/// One configured boundary access rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryAccessRule {
    /// Source zone.
    pub from: PathBuf,
    /// Disallowed target zone.
    pub disallow: PathBuf,
    /// Files covered by the source zone.
    pub covered_files: usize,
}

/// Build a boundary inventory from configured directory rules.
#[must_use]
pub fn boundary_inventory(
    project: &ScannedProject,
    rules: &[BoundaryRule],
    presets: &[BoundaryPreset],
    allow_unmatched: &[String],
) -> BoundaryInventory {
    let zones = configured_boundary_paths(&project.root, rules);
    let library_files = library_files(project);
    let listed_zones = zones
        .iter()
        .map(|zone| BoundaryZone {
            path: zone.clone(),
            file_count: library_files
                .iter()
                .filter(|path| path.starts_with(zone))
                .count(),
        })
        .collect::<Vec<_>>();
    let rules = rules
        .iter()
        .map(|rule| {
            let from = normalize_against(&project.root, &rule.from);
            let disallow = normalize_against(&project.root, &rule.disallow);
            BoundaryAccessRule {
                covered_files: library_files
                    .iter()
                    .filter(|path| path.starts_with(&from))
                    .count(),
                from,
                disallow,
            }
        })
        .collect::<Vec<_>>();
    let uncovered_files = library_files
        .into_iter()
        .filter(|path| {
            !zones.iter().any(|zone| path.starts_with(zone))
                && !matches_allow_unmatched(&project.root, path, allow_unmatched)
        })
        .collect::<Vec<_>>();

    BoundaryInventory {
        configured: !listed_zones.is_empty(),
        presets: presets.to_vec(),
        allow_unmatched: allow_unmatched.to_vec(),
        zones: listed_zones,
        rules,
        uncovered_files,
    }
}

/// Report Dart library files that do not belong to any configured boundary zone.
#[must_use]
pub fn detect_boundary_coverage(
    project: &ScannedProject,
    rules: &[BoundaryRule],
    allow_unmatched: &[String],
) -> Vec<BoundaryCoverageGap> {
    let configured_boundaries = configured_boundary_paths(&project.root, rules);
    if configured_boundaries.is_empty() {
        return Vec::new();
    }

    let mut gaps = project
        .files
        .iter()
        .filter_map(|file| {
            let path = normalize_against(&project.root, &file.path);
            if !is_library_source(&project.root, &path)
                || is_generated_dart_path(&path)
                || configured_boundaries
                    .iter()
                    .any(|boundary| path.starts_with(boundary))
                || matches_allow_unmatched(&project.root, &path, allow_unmatched)
            {
                return None;
            }

            Some(BoundaryCoverageGap {
                path,
                configured_boundaries: configured_boundaries.clone(),
                location: file
                    .declarations
                    .first()
                    .map_or(Location { line: 1, column: 0 }, |declaration| {
                        declaration.location
                    }),
            })
        })
        .collect::<Vec<_>>();

    gaps.sort_by(|left, right| left.path.cmp(&right.path));
    gaps
}

/// Expand built-in architecture presets into concrete path rules.
#[must_use]
pub fn boundary_preset_rules(presets: &[BoundaryPreset]) -> Vec<BoundaryRule> {
    presets
        .iter()
        .copied()
        .flat_map(preset_boundary_rules)
        .collect()
}

fn preset_boundary_rules(preset: BoundaryPreset) -> Vec<BoundaryRule> {
    match preset {
        BoundaryPreset::Layered => rules(&[
            ("lib/domain", "lib/data"),
            ("lib/domain", "lib/application"),
            ("lib/domain", "lib/infrastructure"),
            ("lib/domain", "lib/presentation"),
            ("lib/domain", "lib/ui"),
            ("lib/data", "lib/presentation"),
            ("lib/data", "lib/ui"),
            ("lib/application", "lib/infrastructure"),
            ("lib/application", "lib/presentation"),
            ("lib/application", "lib/ui"),
        ]),
        BoundaryPreset::Hexagonal => rules(&[
            ("lib/domain", "lib/application"),
            ("lib/domain", "lib/infrastructure"),
            ("lib/domain", "lib/adapters"),
            ("lib/domain", "lib/presentation"),
            ("lib/domain", "lib/ui"),
            ("lib/application", "lib/infrastructure"),
            ("lib/application", "lib/adapters"),
            ("lib/application", "lib/presentation"),
            ("lib/application", "lib/ui"),
        ]),
        BoundaryPreset::FeatureSliced => rules(&[
            ("lib/shared", "lib/entities"),
            ("lib/shared", "lib/features"),
            ("lib/shared", "lib/widgets"),
            ("lib/shared", "lib/pages"),
            ("lib/shared", "lib/app"),
            ("lib/entities", "lib/features"),
            ("lib/entities", "lib/widgets"),
            ("lib/entities", "lib/pages"),
            ("lib/entities", "lib/app"),
            ("lib/features", "lib/widgets"),
            ("lib/features", "lib/pages"),
            ("lib/features", "lib/app"),
            ("lib/widgets", "lib/pages"),
            ("lib/widgets", "lib/app"),
            ("lib/pages", "lib/app"),
        ]),
        BoundaryPreset::Bulletproof => rules(&[
            ("lib/core", "lib/features"),
            ("lib/core", "lib/app"),
            ("lib/core", "lib/pages"),
            ("lib/shared", "lib/features"),
            ("lib/shared", "lib/app"),
            ("lib/shared", "lib/pages"),
            ("lib/features", "lib/app"),
            ("lib/features", "lib/pages"),
        ]),
    }
}

fn rules(pairs: &[(&str, &str)]) -> Vec<BoundaryRule> {
    pairs
        .iter()
        .map(|(from, disallow)| BoundaryRule::new(*from, *disallow))
        .collect()
}

fn library_files(project: &ScannedProject) -> Vec<PathBuf> {
    project
        .files
        .iter()
        .filter_map(|file| {
            let path = normalize_against(&project.root, &file.path);
            (is_library_source(&project.root, &path) && !is_generated_dart_path(&path))
                .then_some(path)
        })
        .collect()
}

fn configured_boundary_paths(root: &Path, rules: &[BoundaryRule]) -> Vec<PathBuf> {
    let mut paths = rules
        .iter()
        .flat_map(|rule| [&rule.from, &rule.disallow])
        .map(|path| normalize_against(root, path))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn matches_allow_unmatched(root: &Path, path: &Path, allow_unmatched: &[String]) -> bool {
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    allow_unmatched
        .iter()
        .any(|pattern| matches_pattern(pattern, &relative))
}

fn matches_pattern(pattern: &str, relative: &str) -> bool {
    let normalized = pattern.trim().trim_start_matches("./");
    if normalized.is_empty() {
        return false;
    }
    if relative == normalized
        || relative.starts_with(&format!("{}/", normalized.trim_end_matches('/')))
    {
        return true;
    }
    glob::Pattern::new(normalized).is_ok_and(|pattern| pattern.matches(relative))
}

#[cfg(test)]
mod tests;
