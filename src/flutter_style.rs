use std::fs;
use std::path::PathBuf;

use crate::generated::is_generated_dart_path;
use crate::graph::normalize_against;
use crate::{HealthError, ScannedProject};

mod scanner;
mod types;
mod usage;
pub use types::{
    FlutterStyleFinding, FlutterStyleFindingKind, FlutterStyleReport, FlutterStyleValueKind,
    ThemeTokenEvidence,
};

struct SourceUnit {
    path: PathBuf,
    source: String,
}

struct ParsedSourceUnit<'source> {
    unit: &'source SourceUnit,
    parsed: crate::dart_parser::ParsedDart<'source>,
}

/// Analyze opt-in Flutter theme usage with conservative syntax evidence.
///
/// # Errors
///
/// Returns [`HealthError`] when an included source file cannot be read or parsed.
pub fn analyze_flutter_style(project: &ScannedProject) -> Result<FlutterStyleReport, HealthError> {
    let mut units = Vec::new();
    for file in &project.files {
        let path = normalize_against(&project.root, &file.path);
        if !path.starts_with(&project.root) || ignored_style_path(&path) {
            continue;
        }
        let source = fs::read_to_string(&path).map_err(|source| HealthError::ReadFile {
            path: path.clone(),
            source,
        })?;
        units.push(SourceUnit { path, source });
    }
    scanner::analyze(&units)
}

fn ignored_style_path(path: &std::path::Path) -> bool {
    is_generated_dart_path(path)
        || path.components().any(|component| {
            matches!(
                component.as_os_str().to_str(),
                Some(
                    "test"
                        | "integration_test"
                        | "test_driver"
                        | "__tests__"
                        | "__mocks__"
                        | "example"
                )
            )
        })
}

fn style_parse_error(error: crate::dart_parser::DartParseError) -> HealthError {
    match error {
        crate::dart_parser::DartParseError::Language(source) => HealthError::Language(source),
        crate::dart_parser::DartParseError::ParseCancelled { path }
        | crate::dart_parser::DartParseError::Syntax { path } => {
            HealthError::ParseCancelled { path }
        }
    }
}
