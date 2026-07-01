use std::io;
use std::path::PathBuf;

use thiserror::Error;

use crate::baseline::BaselineError;
use crate::changed_scope::ChangedScopeError;
use crate::config::ConfigError;
use crate::scan::ScanError;
use crate::{
    DependencyHygieneError, DuplicateCodeError, FeatureFlagError, HealthError, PolicyError,
    SecurityError, WidgetAnalysisError, WorkspaceScopeError,
};

/// CLI execution errors.
#[derive(Debug, Error)]
pub enum CliError {
    /// Argument parsing failed.
    #[error(transparent)]
    Clap(#[from] clap::Error),
    /// Project scanning failed.
    #[error(transparent)]
    Scan(#[from] ScanError),
    /// Dependency hygiene analysis failed.
    #[error(transparent)]
    DependencyHygiene(#[from] DependencyHygieneError),
    /// Duplicate-code analysis failed.
    #[error(transparent)]
    DuplicateCode(#[from] DuplicateCodeError),
    /// Health analysis failed.
    #[error(transparent)]
    Health(#[from] HealthError),
    /// Feature flag analysis failed.
    #[error(transparent)]
    FeatureFlags(#[from] FeatureFlagError),
    /// Security candidate analysis failed.
    #[error(transparent)]
    Security(#[from] SecurityError),
    /// Flutter widget parameter analysis failed.
    #[error(transparent)]
    Widgets(#[from] WidgetAnalysisError),
    /// Security top truncation cannot safely run before changed-line scoping.
    #[error(
        "security --top cannot be combined with --gate, --diff-file, --diff-stdin, or --changed-since"
    )]
    UnsupportedSecurityTopScope,
    /// Cross-language duplicate detection is JavaScript/TypeScript-specific in Fallow.
    #[error("dupes --cross-language is not supported for Dart-only analysis")]
    UnsupportedCrossLanguageDupes,
    /// Browser opening only applies to HTML report output.
    #[error("--open only supports HTML reports; use --open by itself or --format html --open")]
    HtmlOpenRequiresHtml,
    /// User-level agent hook installation is intentionally not supported.
    #[error(
        "setup-hooks --user is not supported; Dart Decimate only manages repo-local agent hooks"
    )]
    UnsupportedSetupHooksUser,
    /// Mutating `.gitignore` from setup-hooks is intentionally not supported.
    #[error("setup-hooks --gitignore-claude is not supported; update .gitignore explicitly")]
    UnsupportedSetupHooksGitignoreClaude,
    /// Security review gate could not be applied.
    #[error(transparent)]
    SecurityGate(#[from] crate::security_gate::SecurityGateError),
    /// Changed-file scope could not be computed.
    #[error(transparent)]
    ChangedScope(#[from] ChangedScopeError),
    /// Workspace scope could not be computed.
    #[error(transparent)]
    WorkspaceScope(#[from] WorkspaceScopeError),
    /// Dart Decimate config could not be loaded.
    #[error(transparent)]
    Config(#[from] ConfigError),
    /// Dart Decimate rule config could not be applied.
    #[error(transparent)]
    Rule(#[from] crate::config::RuleError),
    /// Finding baseline could not be loaded or saved.
    #[error(transparent)]
    Baseline(#[from] BaselineError),
    /// Regression tolerance syntax was invalid.
    #[error("invalid regression tolerance {value:?}; expected COUNT or PERCENT like 2 or 10%")]
    Tolerance {
        /// Raw tolerance value.
        value: String,
    },
    /// JSON rendering failed.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    /// Output writing failed.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// Boundary rule syntax was invalid.
    #[error("invalid boundary rule {value:?}; expected FROM:DISALLOW")]
    BoundaryRule {
        /// Raw boundary argument.
        value: String,
    },
    /// Boundary call rule syntax was invalid.
    #[error("invalid boundary call rule {value:?}; expected FROM:PATTERN")]
    BoundaryCallRule {
        /// Raw boundary call argument.
        value: String,
    },
    /// Policy pack loading or analysis failed.
    #[error(transparent)]
    Policy(#[from] PolicyError),
    /// Trace-file target was missing after argument parsing.
    #[error("trace-file requires --file PATH")]
    MissingTraceFile,
    /// Trace-dependency package was missing after argument parsing.
    #[error("trace-dependency requires --dependency PACKAGE")]
    MissingTraceDependency,
    /// Inspect target was missing after argument parsing.
    #[error("inspect requires --file PATH or --symbol FILE:SYMBOL")]
    MissingInspectTarget,
    /// Symbol trace syntax was invalid.
    #[error("invalid trace symbol {value:?}; expected FILE:SYMBOL or --file FILE --symbol SYMBOL")]
    TraceSymbol {
        /// Raw symbol trace argument.
        value: String,
    },
    /// Dead-code analysis did not have any entry points.
    #[error("no entry points provided and no default Dart entry points found under {root}")]
    MissingEntryPoints {
        /// Project root.
        root: PathBuf,
    },
    #[error("coverage analyze requires --runtime-coverage PATH")]
    MissingRuntimeCoverage,
    #[error("cloud runtime coverage is not supported yet; provide --runtime-coverage PATH")]
    UnsupportedCoverageCloud,
    #[error("{command} is offline-only in this release; pass --dry-run")]
    CoverageUploadDryRunRequired { command: &'static str },
    #[error("coverage upload-source-maps directory does not exist: {path}")]
    CoverageUploadDir { path: PathBuf },
    #[error("invalid coverage upload git SHA {value:?}; expected 7 to 40 hex characters")]
    CoverageUploadGitSha { value: String },
    #[error("invalid coverage upload repo {value:?}; expected OWNER/REPO")]
    CoverageUploadRepo { value: String },
    #[error("ci reconcile-review requires --envelope PATH")]
    MissingCiReviewEnvelope,
    #[error(transparent)]
    CiTemplate(#[from] crate::CiTemplateError),
    /// Project initialization failed.
    #[error(transparent)]
    Init(#[from] crate::InitError),
    /// Hook management failed.
    #[error(transparent)]
    Hooks(#[from] crate::HooksError),
    /// SARIF output is not available for this command.
    #[error("--format sarif is not supported by dart-decimate {command}")]
    UnsupportedSarifFormat { command: &'static str },
}
