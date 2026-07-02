use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::graph::normalize_path;
use crate::{
    BoundaryCallRule, BoundaryPreset, BoundaryRule, DuplicateMode, DuplicateOptions,
    DuplicationThreshold, FeatureFlagOptions, HealthOptions, SecurityCategory, SecurityOptions,
};

mod boundary_config;
mod cache;
mod dependencies;
mod health;
mod jsonc;
mod policy;
mod rule_aliases;
mod rules;
use boundary_config::RawBoundaries;
pub use cache::CacheConfig;
pub use dependencies::IgnoreDependencyOverrideRule;
pub(crate) use dependencies::{filter_ignored_dependencies, filter_ignored_dependency_overrides};
pub use health::HealthConfig;
pub use rules::{
    RuleConfig, RuleError, RuleLevel, apply_rules_to_report, missing_suppression_reasons_enabled,
    private_type_leaks_enabled, validate_rules,
};

/// Stable Dart Decimate configuration schema version.
pub const CONFIG_SCHEMA_VERSION: &str = "dart-decimate.config.v1";

/// A discovered or explicitly loaded Dart Decimate configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DartDecimateConfig {
    /// Config file path, when one was loaded.
    pub path: Option<PathBuf>,
    /// Default output format for CLI commands.
    pub output_format: Option<ConfigOutputFormat>,
    /// Default reachability entry points.
    pub entry_points: Vec<PathBuf>,
    /// Whether default reachability should use production entry points only.
    pub production: bool,
    /// Whether unused public declarations in entry libraries should be reported.
    pub include_entry_exports: bool,
    /// Architecture boundary rules.
    pub boundaries: Vec<BoundaryRule>,
    /// Built-in architecture boundary presets expanded into boundary rules.
    pub boundary_presets: Vec<BoundaryPreset>,
    /// Whether configured boundaries must cover every Dart library file.
    pub boundary_coverage: bool,
    /// Root-relative globs exempt from boundary coverage gaps.
    pub boundary_allow_unmatched: Vec<String>,
    /// Boundary-local forbidden call rules.
    pub boundary_calls: Vec<BoundaryCallRule>,
    /// Declarative policy pack paths.
    pub policy_packs: Vec<PathBuf>,
    /// Glob patterns excluded from Dart file discovery.
    pub ignore_patterns: Vec<String>,
    /// Pub dependency names ignored by dependency hygiene checks.
    pub ignore_dependencies: Vec<String>,
    /// Known intentional dependency override declarations.
    pub ignore_dependency_overrides: Vec<IgnoreDependencyOverrideRule>,
    /// Cache configuration metadata.
    pub cache: CacheConfig,
    /// Health analyzer defaults.
    pub health: HealthConfig,
    /// Duplicate-code analyzer defaults.
    pub dupes: DuplicateConfig,
    /// Feature flag analyzer defaults.
    pub flags: FeatureFlagConfig,
    /// Security analyzer defaults.
    pub security: SecurityConfig,
    /// Rule severity controls.
    pub rules: RuleConfig,
}

/// Output formats accepted in Dart Decimate config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConfigOutputFormat {
    /// Human-readable terminal output.
    Human,
    /// Agent-readable JSON output.
    Json,
}

/// Duplicate-code analyzer config defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DuplicateConfig {
    /// Detection mode.
    #[serde(default)]
    pub mode: Option<DuplicateMode>,
    /// Minimum tokens per clone.
    #[serde(default, alias = "minTokens")]
    pub min_tokens: Option<usize>,
    /// Minimum lines per clone.
    #[serde(default, alias = "minLines")]
    pub min_lines: Option<usize>,
    /// Minimum clone instances per group.
    #[serde(default, alias = "minOccurrences")]
    pub min_occurrences: Option<usize>,
    /// Limit output to the N largest clone groups.
    #[serde(default)]
    pub top: Option<usize>,
    /// Fail when duplicated Dart lines exceed this percentage.
    #[serde(default)]
    pub threshold: Option<DuplicationThreshold>,
    /// Only report cross-directory duplicates.
    #[serde(default, alias = "skipLocal")]
    pub skip_local: Option<bool>,
    /// Ignore import/export/part/augment directives.
    #[serde(default, alias = "ignoreImports")]
    pub ignore_imports: Option<bool>,
}

/// Feature flag analyzer config defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureFlagConfig {
    /// Limit output to the N most frequently referenced flags.
    #[serde(default)]
    pub top: Option<usize>,
}

/// Security analyzer config defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityConfig {
    /// Limit output to the N most frequent candidate groups.
    #[serde(default)]
    pub top: Option<usize>,
    /// Include attack-surface inventory entries.
    #[serde(default)]
    pub surface: Option<bool>,
    /// Security candidate categories to include. Empty means all categories.
    #[serde(default)]
    pub categories: Vec<SecurityCategory>,
}

/// Errors returned while reading Dart Decimate config.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Current directory could not be read.
    #[error("failed to read current directory: {source}")]
    CurrentDir {
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Explicit config path did not exist.
    #[error("config file not found: {path}")]
    NotFound {
        /// Missing config path.
        path: PathBuf,
    },
    /// Config file could not be read.
    #[error("failed to read config {path}: {source}")]
    Read {
        /// Config path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// JSON or JSONC config could not be parsed.
    #[error("failed to parse config {path}: {source}")]
    ParseJson {
        /// Config path.
        path: PathBuf,
        /// JSON parse error.
        source: serde_json::Error,
    },
    /// TOML config could not be parsed.
    #[error("failed to parse config {path}: {source}")]
    ParseToml {
        /// Config path.
        path: PathBuf,
        /// TOML parse error.
        source: toml::de::Error,
    },
    /// Boundary rule syntax was invalid.
    #[error("invalid config boundary rule {value:?}; expected FROM:DISALLOW or from/disallow")]
    BoundaryRule {
        /// Raw boundary value.
        value: String,
    },
    /// Boundary call rule syntax was invalid.
    #[error("invalid config boundary call rule {value:?}; expected FROM:PATTERN or from/forbidden")]
    BoundaryCallRule {
        /// Raw boundary call value.
        value: String,
    },
    /// Health threshold override syntax was invalid.
    #[error("invalid config health.thresholdOverrides[{index}]: {message}")]
    HealthThresholdOverride {
        /// Override index.
        index: usize,
        /// Validation failure.
        message: String,
    },
    /// Configured rule name was invalid.
    #[error(transparent)]
    Rule(#[from] RuleError),
}

/// Load Dart Decimate config discovered from `root`, or from an explicit config path.
///
/// # Errors
///
/// Returns [`ConfigError`] when an explicit config is missing, unreadable,
/// malformed, or contains an invalid boundary rule.
pub fn load_dart_decimate_config(
    root: impl AsRef<Path>,
    explicit: Option<&Path>,
) -> Result<DartDecimateConfig, ConfigError> {
    let root = normalize_config_root(root.as_ref())?;
    let Some(path) = config_path(&root, explicit)? else {
        return Ok(DartDecimateConfig::default());
    };
    parse_config_file(&path)
}

/// Return a JSON schema describing supported Dart Decimate config keys.
#[must_use]
pub fn config_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "schema_version": CONFIG_SCHEMA_VERSION,
        "title": "Dart Decimate configuration",
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "format": format_schema(),
            "production": { "type": "boolean" },
            "include_entry_exports": { "type": "boolean" },
            "includeEntryExports": { "type": "boolean" },
            "cli": cli_schema(),
            "entry": path_list_schema(),
            "entries": path_list_schema(),
            "boundary": boundary_config::boundary_schema(),
            "boundaries": boundary_config::boundary_schema(),
            "boundary_coverage": { "type": "boolean" },
            "boundaryCoverage": { "type": "boolean" },
            "boundary_calls": policy::boundary_calls_schema(),
            "boundaryCalls": policy::boundary_calls_schema(),
            "rule_packs": policy::rule_packs_schema(),
            "rulePacks": policy::rule_packs_schema(),
            "ignore_patterns": string_list_schema(),
            "ignorePatterns": string_list_schema(),
            "ignore_dependencies": string_list_schema(),
            "ignoreDependencies": string_list_schema(),
            "ignore_dependency_overrides": dependencies::ignore_dependency_overrides_schema(),
            "ignoreDependencyOverrides": dependencies::ignore_dependency_overrides_schema(),
            "cache": cache::cache_schema(),
            "health": health::health_schema(),
            "dupes": dupes_schema(),
            "flags": top_schema(),
            "security": security_schema(),
            "rules": rules_schema()
        }
    })
}

impl DartDecimateConfig {
    /// Build health analyzer options from config defaults.
    #[must_use]
    pub fn health_options(&self) -> HealthOptions {
        let mut options = HealthOptions::default();
        self.health.apply_to(&mut options);
        options
    }

    /// Build duplicate-code analyzer options from config defaults.
    #[must_use]
    pub fn duplicate_options(&self) -> DuplicateOptions {
        let mut options = DuplicateOptions::default();
        self.dupes.apply_to(&mut options);
        options
    }

    /// Build feature flag analyzer options from config defaults.
    #[must_use]
    pub fn feature_flag_options(&self) -> FeatureFlagOptions {
        FeatureFlagOptions {
            top: self.flags.top,
        }
    }

    /// Build security analyzer options from config defaults.
    #[must_use]
    pub fn security_options(&self) -> SecurityOptions {
        SecurityOptions {
            top: self.security.top,
            surface: self.security.surface.unwrap_or_default(),
            categories: self.security.categories.iter().copied().collect(),
        }
    }
}

impl DuplicateConfig {
    fn apply_to(&self, options: &mut DuplicateOptions) {
        if let Some(mode) = self.mode {
            options.mode = mode;
        }
        if let Some(min_tokens) = self.min_tokens {
            options.min_tokens = min_tokens;
        }
        if let Some(min_lines) = self.min_lines {
            options.min_lines = min_lines;
        }
        if let Some(min_occurrences) = self.min_occurrences {
            options.min_occurrences = min_occurrences.max(2);
        }
        if self.top.is_some() {
            options.top = self.top;
        }
        if self.threshold.is_some() {
            options.threshold = self.threshold;
        }
        if let Some(skip_local) = self.skip_local {
            options.skip_local = skip_local;
        }
        if let Some(ignore_imports) = self.ignore_imports {
            options.ignore_imports = ignore_imports;
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawConfig {
    format: Option<ConfigOutputFormat>,
    cli: RawCliConfig,
    #[serde(alias = "entries")]
    entry: Vec<PathBuf>,
    production: Option<bool>,
    #[serde(alias = "includeEntryExports")]
    include_entry_exports: Option<bool>,
    #[serde(alias = "boundary")]
    boundaries: RawBoundaries,
    #[serde(alias = "boundaryCoverage")]
    boundary_coverage: Option<bool>,
    #[serde(alias = "boundaryCalls")]
    boundary_calls: Vec<policy::RawBoundaryCall>,
    #[serde(alias = "rulePacks")]
    rule_packs: Vec<PathBuf>,
    #[serde(alias = "ignorePatterns")]
    ignore_patterns: Vec<String>,
    #[serde(alias = "ignoreDependencies")]
    ignore_dependencies: Vec<String>,
    #[serde(alias = "ignoreDependencyOverrides")]
    ignore_dependency_overrides: Vec<IgnoreDependencyOverrideRule>,
    cache: CacheConfig,
    health: HealthConfig,
    dupes: DuplicateConfig,
    flags: FeatureFlagConfig,
    security: SecurityConfig,
    rules: RuleConfig,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawCliConfig {
    format: Option<ConfigOutputFormat>,
    #[serde(alias = "entries")]
    entry: Vec<PathBuf>,
    production: Option<bool>,
    #[serde(alias = "includeEntryExports")]
    include_entry_exports: Option<bool>,
    #[serde(alias = "boundary")]
    boundaries: RawBoundaries,
    #[serde(alias = "boundaryCoverage")]
    boundary_coverage: Option<bool>,
    #[serde(alias = "boundaryCalls")]
    boundary_calls: Vec<policy::RawBoundaryCall>,
    #[serde(alias = "rulePacks")]
    rule_packs: Vec<PathBuf>,
}

impl RawConfig {
    fn into_config(self, path: PathBuf) -> Result<DartDecimateConfig, ConfigError> {
        validate_rules(&self.rules)?;
        self.health.validate()?;
        let mut entry_points = self.entry;
        entry_points.extend(self.cli.entry);

        let mut boundary_settings = self.boundaries.into_settings()?;
        boundary_settings.merge(self.cli.boundaries.into_settings()?);
        let boundary_require_all_files = boundary_settings.require_all_files;
        let (boundaries, boundary_presets, boundary_allow_unmatched) = boundary_settings.finish();
        let mut raw_boundary_calls = self.boundary_calls;
        raw_boundary_calls.extend(self.cli.boundary_calls);
        let boundary_calls = raw_boundary_calls
            .into_iter()
            .map(policy::parse_boundary_call)
            .collect::<Result<Vec<_>, _>>()?;
        let mut policy_packs = self.rule_packs;
        policy_packs.extend(self.cli.rule_packs);

        Ok(DartDecimateConfig {
            path: Some(path),
            output_format: self.cli.format.or(self.format),
            entry_points,
            production: self.cli.production.or(self.production).unwrap_or_default(),
            include_entry_exports: self
                .cli
                .include_entry_exports
                .or(self.include_entry_exports)
                .unwrap_or_default(),
            boundaries,
            boundary_presets,
            boundary_coverage: self
                .cli
                .boundary_coverage
                .or(self.boundary_coverage)
                .or(boundary_require_all_files)
                .unwrap_or_default(),
            boundary_allow_unmatched,
            boundary_calls,
            policy_packs,
            ignore_patterns: self.ignore_patterns,
            ignore_dependencies: self.ignore_dependencies,
            ignore_dependency_overrides: self.ignore_dependency_overrides,
            cache: self.cache,
            health: self.health,
            dupes: self.dupes,
            flags: self.flags,
            security: self.security,
            rules: self.rules,
        })
    }
}

fn normalize_config_root(root: &Path) -> Result<PathBuf, ConfigError> {
    if root.is_absolute() {
        return Ok(normalize_path(root));
    }

    let current_dir =
        std::env::current_dir().map_err(|source| ConfigError::CurrentDir { source })?;
    Ok(normalize_path(&current_dir.join(root)))
}

fn config_path(root: &Path, explicit: Option<&Path>) -> Result<Option<PathBuf>, ConfigError> {
    if let Some(path) = explicit {
        let path = if path.is_absolute() {
            normalize_path(path)
        } else {
            normalize_path(&root.join(path))
        };
        if path.is_file() {
            return Ok(Some(path));
        }
        return Err(ConfigError::NotFound { path });
    }

    Ok(config_candidates(root)
        .into_iter()
        .find(|candidate| candidate.is_file()))
}

fn config_candidates(root: &Path) -> Vec<PathBuf> {
    [
        ".dart-decimaterc",
        ".dart-decimaterc.json",
        ".dart-decimaterc.jsonc",
        "dart-decimate.toml",
        ".dart-decimate.toml",
    ]
    .into_iter()
    .map(|name| root.join(name))
    .collect()
}

fn parse_config_file(path: &Path) -> Result<DartDecimateConfig, ConfigError> {
    let source = fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let raw: RawConfig = match config_format(path, &source) {
        ConfigFormat::Json => {
            serde_json::from_str(&source).map_err(|source| ConfigError::ParseJson {
                path: path.to_path_buf(),
                source,
            })?
        }
        ConfigFormat::Jsonc => {
            serde_json::from_str(&jsonc::strip_json_comments(&source)).map_err(|source| {
                ConfigError::ParseJson {
                    path: path.to_path_buf(),
                    source,
                }
            })?
        }
        ConfigFormat::Toml => toml::from_str(&source).map_err(|source| ConfigError::ParseToml {
            path: path.to_path_buf(),
            source,
        })?,
    };
    raw.into_config(path.to_path_buf())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfigFormat {
    Json,
    Jsonc,
    Toml,
}

fn config_format(path: &Path, source: &str) -> ConfigFormat {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("json") => ConfigFormat::Json,
        Some("jsonc") => ConfigFormat::Jsonc,
        Some("toml") => ConfigFormat::Toml,
        _ if source.trim_start().starts_with('{') => ConfigFormat::Jsonc,
        _ => ConfigFormat::Toml,
    }
}

fn format_schema() -> Value {
    json!({ "type": "string", "enum": ["human", "json"] })
}

fn cli_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "format": format_schema(),
            "production": { "type": "boolean" },
            "include_entry_exports": { "type": "boolean" },
            "includeEntryExports": { "type": "boolean" },
            "entry": path_list_schema(),
            "entries": path_list_schema(),
            "boundary": boundary_config::boundary_schema(),
            "boundaries": boundary_config::boundary_schema(),
            "boundary_coverage": { "type": "boolean" },
            "boundaryCoverage": { "type": "boolean" },
            "boundary_calls": policy::boundary_calls_schema(),
            "boundaryCalls": policy::boundary_calls_schema(),
            "rule_packs": policy::rule_packs_schema(),
            "rulePacks": policy::rule_packs_schema()
        }
    })
}

fn path_list_schema() -> Value {
    json!({ "type": "array", "items": { "type": "string" } })
}

fn string_list_schema() -> Value {
    json!({ "type": "array", "items": { "type": "string" } })
}

fn dupes_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "mode": {
                "type": "string",
                "enum": ["strict", "mild", "weak", "semantic"]
            },
            "min_tokens": positive_integer_schema(),
            "minTokens": positive_integer_schema(),
            "min_lines": positive_integer_schema(),
            "minLines": positive_integer_schema(),
            "min_occurrences": positive_integer_schema(),
            "minOccurrences": positive_integer_schema(),
            "top": positive_integer_schema(),
            "threshold": percentage_schema(),
            "skip_local": { "type": "boolean" },
            "skipLocal": { "type": "boolean" },
            "ignore_imports": { "type": "boolean" },
            "ignoreImports": { "type": "boolean" }
        }
    })
}

fn top_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "top": positive_integer_schema()
        }
    })
}

fn security_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "top": positive_integer_schema(),
            "surface": { "type": "boolean" },
            "categories": {
                "type": "array",
                "items": {
                    "type": "string",
                    "enum": [
                        "hardcoded-secret",
                        "firebase-api-key",
                        "insecure-transport",
                        "tls-bypass",
                        "web-view-risk",
                        "process-execution",
                        "raw-sql",
                        "plain-secret-storage"
                    ]
                },
                "uniqueItems": true
            }
        }
    })
}

fn rules_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": {
            "type": "string",
            "enum": ["error", "warn", "off"]
        }
    })
}

fn positive_integer_schema() -> Value {
    json!({ "type": "integer", "minimum": 1 })
}

fn percentage_schema() -> Value {
    json!({ "type": "number", "minimum": 0, "maximum": 100 })
}
