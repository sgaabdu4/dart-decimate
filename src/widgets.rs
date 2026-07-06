use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tree_sitter::Node;

use crate::generated::is_generated_dart_path;
use crate::graph::normalize_against;
use crate::{DeadCodeReport, Location, ScannedProject};

mod forwarding;
mod lifecycle;
mod params;
mod patterns;
mod reads;
mod top_level;
mod unrendered;

use forwarding::{forwarded_param_used, widget_forwarded_param_uses};
pub use lifecycle::MissingContextMountedAfterAwait;
use lifecycle::lifecycle_findings;
use params::constructor_params;
use patterns::object_pattern_field_reads_for_widget;
use reads::{state_body_uses_param, widget_body_uses_param};
use top_level::top_level_widget_functions;
use unrendered::unrendered_widgets;

/// Flutter widget framework analysis.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WidgetReport {
    /// Dart files included in widget analysis.
    pub analyzed_files: usize,
    /// Widget constructor parameters that are never read.
    pub unused_params: Vec<UnusedWidgetParam>,
    /// Private Flutter widget classes.
    pub private_widget_classes: Vec<PrivateWidgetClass>,
    /// Top-level Flutter widget helper functions.
    pub top_level_functions: Vec<WidgetTopLevelFunction>,
    /// Flutter widget classes with no reachable object construction.
    pub unrendered_widgets: Vec<UnrenderedWidgetClass>,
    /// Widget or `State` awaits missing an immediate `context.mounted` guard.
    pub missing_context_mounted_after_await: Vec<MissingContextMountedAfterAwait>,
}

/// A widget constructor parameter that is not used by the widget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnusedWidgetParam {
    /// Dart file containing the widget declaration.
    pub path: PathBuf,
    /// Widget class that owns the parameter.
    pub widget_class: String,
    /// Flutter widget base class.
    pub widget_kind: WidgetClassKind,
    /// Constructor parameter name.
    pub param_name: String,
    /// Location of the constructor parameter identifier.
    pub location: Location,
}

/// A private class that extends a Flutter widget base.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateWidgetClass {
    /// Dart file containing the widget declaration.
    pub path: PathBuf,
    /// Private widget class name.
    pub widget_class: String,
    /// Flutter widget base class.
    pub widget_kind: WidgetClassKind,
    /// Location of the class identifier.
    pub location: Location,
}

/// A top-level function that should be owned by a widget class or helper owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WidgetTopLevelFunction {
    /// Dart file containing the function declaration.
    pub path: PathBuf,
    /// Top-level function name.
    pub function_name: String,
    /// Function return type when declared.
    pub return_type: Option<String>,
    /// Location of the function identifier.
    pub location: Location,
}

/// A widget class that is never constructed from reachable production code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnrenderedWidgetClass {
    /// Dart file containing the widget declaration.
    pub path: PathBuf,
    /// Widget class name.
    pub widget_class: String,
    /// Flutter widget base class.
    pub widget_kind: WidgetClassKind,
    /// Location of the class identifier.
    pub location: Location,
    /// Reachable object construction references for this widget.
    pub render_reference_count: usize,
}

/// Supported Flutter widget base classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WidgetClassKind {
    StatelessWidget,
    StatefulWidget,
    ConsumerWidget,
    ConsumerStatefulWidget,
    HookWidget,
    HookConsumerWidget,
}

/// Errors returned while analyzing Flutter widgets.
#[derive(Debug, Error)]
pub enum WidgetAnalysisError {
    /// A Dart file could not be read.
    #[error("failed to read Dart file {path}: {source}")]
    ReadFile {
        /// File path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Tree-Sitter rejected the Dart grammar.
    #[error("failed to load Dart grammar: {0}")]
    Language(#[from] tree_sitter::LanguageError),
    /// Tree-Sitter did not produce a parse tree.
    #[error("tree-sitter did not return a parse tree for {path}")]
    ParseCancelled {
        /// Path being parsed.
        path: PathBuf,
    },
    /// The source parsed with syntax errors.
    #[error("Dart syntax errors found in {path}")]
    Syntax {
        /// Path being parsed.
        path: PathBuf,
    },
}

/// Detect Flutter widget framework issues.
///
/// # Errors
///
/// Returns [`WidgetAnalysisError`] if a scanned Dart file cannot be read or
/// parsed during widget analysis.
pub fn analyze_widgets(
    project: &ScannedProject,
    dead_code: Option<&DeadCodeReport>,
) -> Result<WidgetReport, WidgetAnalysisError> {
    let paths = widget_analysis_paths(project, dead_code);
    let file_findings = paths
        .par_iter()
        .map(|path| analyze_file(path))
        .collect::<Result<Vec<_>, _>>()?;
    let mut findings = merge_file_widget_findings(file_findings);
    sort_file_widget_findings(&mut findings);
    let unrendered_widgets = unrendered_widgets(project, &paths)?;

    Ok(WidgetReport {
        analyzed_files: paths.len(),
        unused_params: findings.unused_params,
        private_widget_classes: findings.private_widget_classes,
        top_level_functions: findings.top_level_functions,
        unrendered_widgets,
        missing_context_mounted_after_await: findings.missing_context_mounted_after_await,
    })
}

fn widget_analysis_paths(
    project: &ScannedProject,
    dead_code: Option<&DeadCodeReport>,
) -> Vec<PathBuf> {
    let dead_files = dead_code
        .map(|report| {
            report
                .dead_files
                .iter()
                .map(|dead| dead.path.clone())
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    project
        .files
        .iter()
        .map(|file| normalize_against(&project.root, &file.path))
        .filter(|path| path.starts_with(&project.root))
        .filter(|path| !dead_files.contains(path))
        .filter(|path| !is_generated_dart_path(path) && !is_test_path(path))
        .collect()
}

fn merge_file_widget_findings(file_findings: Vec<FileWidgetFindings>) -> FileWidgetFindings {
    let mut merged = FileWidgetFindings::default();
    for mut findings in file_findings {
        merged.unused_params.append(&mut findings.unused_params);
        merged
            .private_widget_classes
            .append(&mut findings.private_widget_classes);
        merged
            .top_level_functions
            .append(&mut findings.top_level_functions);
        merged
            .missing_context_mounted_after_await
            .append(&mut findings.missing_context_mounted_after_await);
    }
    merged
}

fn sort_file_widget_findings(findings: &mut FileWidgetFindings) {
    sort_widget_core_findings(findings);
    sort_lifecycle_findings(findings);
}

fn sort_widget_core_findings(findings: &mut FileWidgetFindings) {
    findings.unused_params.sort_by(|left, right| {
        (
            &left.path,
            left.location.line,
            left.location.column,
            &left.widget_class,
            &left.param_name,
        )
            .cmp(&(
                &right.path,
                right.location.line,
                right.location.column,
                &right.widget_class,
                &right.param_name,
            ))
    });
    findings.private_widget_classes.sort_by(|left, right| {
        (
            &left.path,
            left.location.line,
            left.location.column,
            &left.widget_class,
        )
            .cmp(&(
                &right.path,
                right.location.line,
                right.location.column,
                &right.widget_class,
            ))
    });
    findings.top_level_functions.sort_by(|left, right| {
        (
            &left.path,
            left.location.line,
            left.location.column,
            &left.function_name,
        )
            .cmp(&(
                &right.path,
                right.location.line,
                right.location.column,
                &right.function_name,
            ))
    });
}

fn sort_lifecycle_findings(findings: &mut FileWidgetFindings) {
    findings
        .missing_context_mounted_after_await
        .sort_by(|left, right| {
            (
                &left.path,
                left.location.line,
                left.location.column,
                &left.owner,
            )
                .cmp(&(
                    &right.path,
                    right.location.line,
                    right.location.column,
                    &right.owner,
                ))
        });
}

fn analyze_file(path: &Path) -> Result<FileWidgetFindings, WidgetAnalysisError> {
    let source = fs::read_to_string(path).map_err(|source| WidgetAnalysisError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let parsed = parse_tree(path, &source)?;
    let root = parsed.tree().root_node();

    Ok(findings_in_source(path, root, parsed.source()))
}

fn parse_tree<'source>(
    path: &Path,
    source: &'source str,
) -> Result<crate::dart_parser::ParsedDart<'source>, WidgetAnalysisError> {
    crate::dart_parser::parse_dart_source_strict(path, source).map_err(widget_parse_error)
}

fn widget_parse_error(error: crate::dart_parser::DartParseError) -> WidgetAnalysisError {
    match error {
        crate::dart_parser::DartParseError::Language(source) => {
            WidgetAnalysisError::Language(source)
        }
        crate::dart_parser::DartParseError::ParseCancelled { path } => {
            WidgetAnalysisError::ParseCancelled { path }
        }
        crate::dart_parser::DartParseError::Syntax { path } => WidgetAnalysisError::Syntax { path },
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct FileWidgetFindings {
    unused_params: Vec<UnusedWidgetParam>,
    private_widget_classes: Vec<PrivateWidgetClass>,
    top_level_functions: Vec<WidgetTopLevelFunction>,
    missing_context_mounted_after_await: Vec<MissingContextMountedAfterAwait>,
}

fn findings_in_source(path: &Path, root: Node<'_>, source: &str) -> FileWidgetFindings {
    let mut classes = Vec::new();
    collect_class_declarations(root, &mut classes);
    let states = state_classes_by_widget(&classes, source);
    let forwarded_uses = widget_forwarded_param_uses(&classes, source);
    let mut findings = FileWidgetFindings::default();
    let has_widget_class = classes
        .iter()
        .any(|class| widget_kind(*class, source).is_some());
    findings.top_level_functions = top_level_widget_functions(path, root, source, has_widget_class);
    let lifecycle = lifecycle_findings(path, &classes, source);
    findings.missing_context_mounted_after_await = lifecycle.missing_context_mounted_after_await;

    for class in classes {
        let Some(widget_kind) = widget_kind(class, source) else {
            continue;
        };
        let Some(name_node) = class.child_by_field_name("name") else {
            continue;
        };
        let Ok(widget_class) = name_node.utf8_text(source.as_bytes()).map(str::to_owned) else {
            continue;
        };
        if widget_class.starts_with('_') {
            findings.private_widget_classes.push(PrivateWidgetClass {
                path: path.to_path_buf(),
                widget_class: widget_class.clone(),
                widget_kind,
                location: name_node.start_position().into(),
            });
        }
        let Some(body) = class.child_by_field_name("body") else {
            continue;
        };
        let object_pattern_reads = object_pattern_field_reads_for_widget(
            root,
            &widget_class,
            body,
            states.get(&widget_class),
            source,
        );
        for param in constructor_params(class, &widget_class, source) {
            if widget_body_uses_param(body, &param.field_name, source)
                || object_pattern_reads.contains(&param.field_name)
                || states.get(&widget_class).is_some_and(|state_bodies| {
                    state_bodies.iter().any(|state_body| {
                        state_body_uses_param(*state_body, &param.field_name, source)
                    })
                })
                || forwarded_param_used(
                    forwarded_uses.get(&widget_class),
                    body,
                    states.get(&widget_class),
                    &param.field_name,
                    source,
                )
            {
                continue;
            }
            findings.unused_params.push(UnusedWidgetParam {
                path: path.to_path_buf(),
                widget_class: widget_class.clone(),
                widget_kind,
                param_name: param.name,
                location: param.location,
            });
        }
    }

    findings
}

fn collect_class_declarations<'tree>(node: Node<'tree>, classes: &mut Vec<Node<'tree>>) {
    if node.kind() == "class_declaration" {
        classes.push(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_class_declarations(child, classes);
    }
}

fn state_classes_by_widget<'tree>(
    classes: &[Node<'tree>],
    source: &str,
) -> BTreeMap<String, Vec<Node<'tree>>> {
    let mut states = BTreeMap::<String, Vec<Node<'tree>>>::new();
    for class in classes {
        let Some(widget_class) = state_widget_class(*class, source) else {
            continue;
        };
        if let Some(body) = class.child_by_field_name("body") {
            states.entry(widget_class).or_default().push(body);
        }
    }
    states
}

pub(super) fn widget_kind(class: Node<'_>, source: &str) -> Option<WidgetClassKind> {
    let base = superclass_base_name(class, source)?;
    match base.as_str() {
        "StatelessWidget" => Some(WidgetClassKind::StatelessWidget),
        "StatefulWidget" => Some(WidgetClassKind::StatefulWidget),
        "ConsumerWidget" => Some(WidgetClassKind::ConsumerWidget),
        "ConsumerStatefulWidget" => Some(WidgetClassKind::ConsumerStatefulWidget),
        "HookWidget" => Some(WidgetClassKind::HookWidget),
        "HookConsumerWidget" => Some(WidgetClassKind::HookConsumerWidget),
        _ => None,
    }
}

pub(super) fn state_widget_class(class: Node<'_>, source: &str) -> Option<String> {
    let type_text = superclass_type_text(class, source)?;
    let compact = strip_whitespace(&type_text);
    let base = simple_type_name(compact.split('<').next().unwrap_or(&compact));
    if !matches!(base.as_str(), "State" | "ConsumerState") {
        return None;
    }
    let generic = compact
        .split_once('<')
        .and_then(|(_, rest)| rest.rsplit_once('>').map(|(inside, _)| inside))?;
    generic.split(',').next().map(simple_type_name)
}

fn superclass_base_name(class: Node<'_>, source: &str) -> Option<String> {
    superclass_type_text(class, source).map(|text| {
        let compact = strip_whitespace(&text);
        simple_type_name(compact.split('<').next().unwrap_or(&compact))
    })
}

pub(super) fn superclass_type_text(class: Node<'_>, source: &str) -> Option<String> {
    let superclass = class.child_by_field_name("superclass")?;
    let type_text = superclass
        .child_by_field_name("type")?
        .utf8_text(source.as_bytes())
        .ok()
        .map(str::to_owned)?;
    if type_text.contains('<') {
        return Some(type_text);
    }
    superclass
        .utf8_text(source.as_bytes())
        .ok()
        .and_then(|text| {
            let without_extends = text.trim().strip_prefix("extends")?.trim();
            Some(
                without_extends
                    .split(" with ")
                    .next()
                    .unwrap_or(without_extends)
                    .to_owned(),
            )
        })
        .or(Some(type_text))
}

fn strip_whitespace(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

pub(super) fn simple_type_name(text: &str) -> String {
    let text = text.trim_end_matches('?');
    let text = text.split('<').next().unwrap_or(text).trim();
    text.rsplit('.').next().unwrap_or(text).to_owned()
}

fn visit_named(node: Node<'_>, visitor: &mut impl FnMut(Node<'_>)) {
    visitor(node);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visit_named(child, visitor);
    }
}

fn has_ancestor_kind(node: Node<'_>, kinds: &[&str]) -> bool {
    let mut parent = node.parent();
    while let Some(ancestor) = parent {
        if kinds.contains(&ancestor.kind()) {
            return true;
        }
        parent = ancestor.parent();
    }
    false
}

fn is_test_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with("_test.dart"))
        || path.components().any(|component| {
            component.as_os_str().to_str().is_some_and(|segment| {
                matches!(segment, "test" | "integration_test" | "test_driver")
            })
        })
}

#[cfg(test)]
mod tests;
