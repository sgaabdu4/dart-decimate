use super::IssueExplanation;

pub(super) const STYLE_ISSUES: &[IssueExplanation] = &[
    issue!(
        "raw-flutter-style-value",
        "dart-decimate/raw-flutter-style-value",
        &["raw-flutter-style-value", "raw-flutter-style-values"],
        "Raw Flutter style value",
        "A raw Color or TextStyle value has a resolvable project theme token.",
        "Theme token use keeps visual intent consistent while allowing centralized changes.",
        "A widget uses Color(0xFF112233) while colorScheme.primary is defined.",
        "Review the nearest token evidence, then replace the raw value when its semantic role matches.",
        &["// dart-decimate-ignore-next-line raw-flutter-style-value -- <reason>"],
        &["dart-decimate health --format json --flutter-style"],
    ),
    issue!(
        "near-duplicate-theme-token",
        "dart-decimate/near-duplicate-theme-token",
        &["near-duplicate-theme-token", "near-duplicate-theme-tokens"],
        "Near-duplicate theme token",
        "Two custom ThemeExtension color fields have near-identical ARGB values.",
        "Nearly identical custom colors can indicate accidental token drift or duplicate semantic roles.",
        "accent and accentSoft differ by one ARGB channel value.",
        "Review both semantic roles before consolidating; Dart Decimate never replaces tokens automatically.",
        &["// dart-decimate-ignore-next-line near-duplicate-theme-token -- <reason>"],
        &["dart-decimate health --format json --flutter-style"],
    ),
    issue!(
        "unused-theme-extension-token",
        "dart-decimate/unused-theme-extension-token",
        &[
            "unused-theme-extension-token",
            "unused-theme-extension-tokens"
        ],
        "Unused ThemeExtension token",
        "A custom ThemeExtension field has no observed member access in included source.",
        "Unused custom tokens increase design-system surface, but generated or dynamic consumers can remain invisible.",
        "AppTokens.deprecatedAccent is declared and configured but never accessed.",
        "Verify generated and dynamic consumers before removing the field.",
        &["// dart-decimate-ignore-next-line unused-theme-extension-token -- <reason>"],
        &["dart-decimate health --format json --flutter-style"],
    ),
];
