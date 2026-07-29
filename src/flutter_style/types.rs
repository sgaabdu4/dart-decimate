use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Location;

/// Opt-in Flutter style advisory category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlutterStyleFindingKind {
    /// A raw theme-able value has a resolvable project token.
    RawFlutterStyleValue,
    /// Two custom color tokens are close enough to merit consolidation review.
    NearDuplicateThemeToken,
    /// A custom `ThemeExtension` field has no observed member access.
    UnusedThemeExtensionToken,
}

impl FlutterStyleFindingKind {
    /// Stable rule identifier.
    #[must_use]
    pub const fn rule_id(self) -> &'static str {
        match self {
            Self::RawFlutterStyleValue => "dart-decimate/raw-flutter-style-value",
            Self::NearDuplicateThemeToken => "dart-decimate/near-duplicate-theme-token",
            Self::UnusedThemeExtensionToken => "dart-decimate/unused-theme-extension-token",
        }
    }
}

/// Theme-able value category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlutterStyleValueKind {
    /// Flutter `Color` value.
    Color,
    /// Flutter `TextStyle` value.
    TextStyle,
}

/// Exact source evidence for a known theme token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeTokenEvidence {
    /// Stable field/role name.
    pub name: String,
    /// Source file containing the token definition.
    pub path: PathBuf,
    /// Token declaration or configured-value location.
    pub location: Location,
    /// Token value category.
    pub value_kind: FlutterStyleValueKind,
    /// Normalized source value when supported.
    pub value: Option<String>,
    /// Whether this token belongs to a custom `ThemeExtension`.
    pub custom: bool,
    #[serde(skip)]
    pub(super) argb: Option<u32>,
    #[serde(skip)]
    pub(super) owner: Option<String>,
    #[serde(skip)]
    pub(super) declared_field: bool,
}

/// One advisory Flutter style row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlutterStyleFinding {
    /// Advisory category.
    pub kind: FlutterStyleFindingKind,
    /// Source file containing the candidate.
    pub path: PathBuf,
    /// Candidate source location.
    pub location: Location,
    /// Theme-able value category.
    pub value_kind: FlutterStyleValueKind,
    /// Normalized raw value when supported.
    pub value: Option<String>,
    /// Primary token for token-pair and unused-token rows.
    pub token: Option<ThemeTokenEvidence>,
    /// Deterministically selected nearest token.
    pub nearest_token: Option<ThemeTokenEvidence>,
    /// Deterministic ARGB distance formatted to two decimal places.
    pub distance: Option<String>,
}

/// Opt-in Flutter style advisory report.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlutterStyleReport {
    /// Deterministically sorted advisory rows.
    pub findings: Vec<FlutterStyleFinding>,
}
