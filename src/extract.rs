use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tree_sitter::{Node, TreeCursor};

mod directives;
mod members;
mod primary_constructors;
mod references;
mod routes;
mod signatures;
mod strings;
use directives::{
    extract_directive, extract_library_name, extract_part_directive, extract_part_of_directive,
};
use members::push_class_like_members;
use primary_constructors::{
    extract_primary_constructor_identifier_references,
    extract_primary_constructor_signature_references,
};
use references::extract_identifier_references;
pub use routes::DartRouteDeclaration;
use routes::extract_route_declarations;
pub use signatures::SignatureReference;
use signatures::extract_signature_references;
use strings::unquote_dart_string;

/// A 1-based line and 0-based byte column in a Dart source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    /// 1-based source line.
    pub line: usize,
    /// 0-based UTF-8 byte column.
    pub column: usize,
}

impl From<tree_sitter::Point> for Location {
    fn from(point: tree_sitter::Point) -> Self {
        Self {
            line: point.row + 1,
            column: point.column,
        }
    }
}

/// A 1-based source line range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRange {
    /// 1-based first line.
    pub start_line: usize,
    /// 1-based last line.
    pub end_line: usize,
}

impl SourceRange {
    fn from_node(node: Node<'_>) -> Self {
        Self {
            start_line: node.start_position().row + 1,
            end_line: node.end_position().row + 1,
        }
    }
}

/// A parsed Dart file reduced to graph-relevant syntax facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DartFile {
    /// File path supplied by the caller.
    pub path: PathBuf,
    /// Optional `library` directive metadata.
    pub library: Option<DartLibrary>,
    /// Optional `part of` directive metadata.
    pub part_of: Option<DartPartOf>,
    /// Static import directives in source order.
    pub imports: Vec<DartImport>,
    /// Static export directives in source order.
    pub exports: Vec<DartExport>,
    /// Part directives in source order.
    pub parts: Vec<DartPart>,
    /// Top-level declarations in source order.
    pub declarations: Vec<TopLevelDeclaration>,
    /// Class-like member declarations in source order.
    pub members: Vec<MemberDeclaration>,
    /// Identifier uses in source order, including non-raw string interpolation,
    /// excluding import/export metadata and obvious declaration-name positions.
    pub references: Vec<IdentifierReference>,
    /// Type references from public declaration signatures, excluding bodies.
    pub signature_references: Vec<SignatureReference>,
    /// Typed `GoRouter` route declarations found in metadata.
    pub routes: Vec<DartRouteDeclaration>,
}

/// A Dart `library` directive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DartLibrary {
    /// Dotted library name, if the directive names the library.
    pub name: Option<String>,
    /// Augmentation URI from `library augment`, if present.
    pub augment_uri: Option<String>,
    /// Location of the `library_name` syntax node.
    pub location: Location,
}

/// A Dart `import` directive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DartImport {
    /// The import URI without surrounding quotes.
    pub uri: String,
    /// Conditional import guard for a `if (dart.library.*)` branch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<DartUriCondition>,
    /// Optional import prefix after `as`.
    pub prefix: Option<String>,
    /// Whether this import uses `deferred as`.
    pub deferred: bool,
    /// `show` and `hide` combinators applied to this import.
    pub combinators: Vec<DartCombinator>,
    /// Location of the `import_or_export` syntax node.
    pub location: Location,
}

/// A Dart `export` directive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DartExport {
    /// The export URI without surrounding quotes.
    pub uri: String,
    /// Conditional export guard for a `if (dart.library.*)` branch.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<DartUriCondition>,
    /// `show` and `hide` combinators applied to this export.
    pub combinators: Vec<DartCombinator>,
    /// Location of the `import_or_export` syntax node.
    pub location: Location,
}

/// A Dart import/export combinator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DartCombinator {
    /// `show` or `hide`.
    pub kind: DartCombinatorKind,
    /// Names listed by the combinator.
    pub names: Vec<String>,
    /// Location of the combinator syntax node.
    pub location: Location,
}

/// A Dart configurable URI condition, such as `dart.library.html == 'true'`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DartUriCondition {
    /// Environment declaration tested by the configurable URI branch.
    pub variable: String,
    /// String value required by the branch. Bare tests require `"true"`.
    pub expected_value: String,
    /// Location of the `uri_test` syntax node.
    pub location: Location,
}

/// Dart import/export combinator kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DartCombinatorKind {
    /// `show`.
    Show,
    /// `hide`.
    Hide,
}

/// A Dart `part` directive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DartPart {
    /// The part URI without surrounding quotes.
    pub uri: String,
    /// Location of the `part_directive` syntax node.
    pub location: Location,
}

/// A Dart `part of` directive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DartPartOf {
    /// Dotted library name form, if the directive uses `part of name;`.
    pub name: Option<String>,
    /// URI form, if the directive uses `part of 'library.dart';`.
    pub uri: Option<String>,
    /// Location of the `part_of_directive` syntax node.
    pub location: Location,
}

/// A top-level declaration that can become a graph or symbol node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopLevelDeclaration {
    /// Declaration kind.
    pub kind: DeclarationKind,
    /// Declared name.
    pub name: String,
    /// Location of the declaration node.
    pub location: Location,
    /// Source line range covered by the declaration node.
    pub range: SourceRange,
}

/// A class-like member declaration that can become a future symbol node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberDeclaration {
    /// Owning top-level class-like declaration.
    pub owner: String,
    /// Member kind.
    pub kind: MemberKind,
    /// Declared member name.
    pub name: String,
    /// Location of the member declaration node.
    pub location: Location,
}

/// A syntactic identifier or type identifier use, including references found in
/// non-raw string interpolation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentifierReference {
    /// Referenced identifier text.
    pub name: String,
    /// Location of the identifier token.
    pub location: Location,
}

/// Top-level declaration categories extracted in Phase 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeclarationKind {
    /// A `class` declaration, including modifiers such as `sealed` or `base`.
    Class,
    /// A `mixin` declaration.
    Mixin,
    /// A named `extension` declaration.
    Extension,
    /// An `extension type` declaration.
    ExtensionType,
    /// An `enum` declaration.
    Enum,
    /// A `typedef` declaration.
    TypeAlias,
    /// A top-level variable declaration.
    Variable,
    /// A top-level function declaration, including `external` functions.
    Function,
}

/// Class-like member declaration categories extracted in Phase 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MemberKind {
    /// An enum constant.
    EnumConstant,
    /// A field declaration.
    Field,
    /// A getter declaration.
    Getter,
    /// A setter declaration.
    Setter,
    /// A method declaration.
    Method,
    /// A constructor or factory constructor.
    Constructor,
    /// An operator overload.
    Operator,
}

/// Errors returned by the Dart extraction phase.
#[derive(Debug, Error)]
pub enum ExtractError {
    /// The file could not be read from disk.
    #[error("failed to read Dart file {path}: {source}")]
    Read {
        /// Path that failed to read.
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

/// Extract graph-relevant syntax facts from a Dart file on disk.
///
/// This phase intentionally ignores function bodies and does not perform type
/// evaluation, name resolution, or import resolution.
///
/// # Errors
///
/// Returns [`ExtractError::Read`] when the file cannot be read, or a parser
/// error when Tree-Sitter cannot load the Dart grammar, cannot produce a tree,
/// or finds Dart syntax errors.
pub fn extract_dart_file(path: impl AsRef<Path>) -> Result<DartFile, ExtractError> {
    let path = path.as_ref();
    let source = fs::read_to_string(path).map_err(|source| ExtractError::Read {
        path: path.to_path_buf(),
        source,
    })?;

    extract_dart_source(path, &source)
}

/// Extract graph-relevant syntax facts from Dart source text.
///
/// `path` is carried into the returned facts and error messages. It does not
/// need to exist on disk.
///
/// # Errors
///
/// Returns a parser error when Tree-Sitter cannot load the Dart grammar, cannot
/// produce a tree, or finds Dart syntax errors.
pub fn extract_dart_source(path: impl AsRef<Path>, source: &str) -> Result<DartFile, ExtractError> {
    let path = path.as_ref().to_path_buf();
    let parsed =
        crate::dart_parser::parse_dart_source_strict(&path, source).map_err(extract_parse_error)?;
    let root = parsed.tree().root_node();
    let parse_source = parsed.source();

    let mut library = None;
    let mut part_of = None;
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut parts = Vec::new();
    let mut declarations = Vec::new();
    let mut members = Vec::new();
    let mut references = extract_identifier_references(root, parse_source);
    references.extend(extract_primary_constructor_identifier_references(source));
    sort_identifier_references(&mut references);
    let mut signature_references = extract_signature_references(root, parse_source);
    signature_references.extend(extract_primary_constructor_signature_references(source));
    sort_signature_references(&mut signature_references);
    let routes = extract_route_declarations(root, parse_source);

    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        match child.kind() {
            "library_name" => library = Some(extract_library_name(child, parse_source)),
            "import_or_export" => {
                extract_directive(child, parse_source, &mut imports, &mut exports);
            }
            "part_directive" => extract_part_directive(child, parse_source, &mut parts),
            "part_of_directive" => {
                part_of = Some(extract_part_of_directive(child, parse_source));
            }
            "class_declaration" => {
                push_class_declaration(&mut declarations, child, parse_source);
                push_class_like_members(&mut members, child, parse_source);
            }
            "mixin_declaration" => {
                push_named_declaration(
                    &mut declarations,
                    child,
                    parse_source,
                    DeclarationKind::Mixin,
                );
                push_class_like_members(&mut members, child, parse_source);
            }
            "extension_declaration" => {
                push_named_declaration(
                    &mut declarations,
                    child,
                    parse_source,
                    DeclarationKind::Extension,
                );
                push_class_like_members(&mut members, child, parse_source);
            }
            "extension_type_declaration" => {
                push_named_declaration(
                    &mut declarations,
                    child,
                    parse_source,
                    DeclarationKind::ExtensionType,
                );
                push_class_like_members(&mut members, child, parse_source);
            }
            "enum_declaration" => {
                push_named_declaration(
                    &mut declarations,
                    child,
                    parse_source,
                    DeclarationKind::Enum,
                );
                push_class_like_members(&mut members, child, parse_source);
            }
            "type_alias" => push_type_alias_declaration(&mut declarations, child, parse_source),
            "top_level_variable_declaration" | "external_variable_declaration" => {
                push_variable_declarations(&mut declarations, child, parse_source);
            }
            "function_declaration"
            | "external_function_declaration"
            | "getter_declaration"
            | "external_getter_declaration"
            | "setter_declaration"
            | "external_setter_declaration" => {
                push_function_declaration(&mut declarations, child, parse_source);
            }
            _ => {}
        }
    }

    Ok(DartFile {
        path,
        library,
        part_of,
        imports,
        exports,
        parts,
        declarations,
        members,
        references,
        signature_references,
        routes,
    })
}

fn extract_parse_error(error: crate::dart_parser::DartParseError) -> ExtractError {
    match error {
        crate::dart_parser::DartParseError::Language(source) => ExtractError::Language(source),
        crate::dart_parser::DartParseError::ParseCancelled { path } => {
            ExtractError::ParseCancelled { path }
        }
        crate::dart_parser::DartParseError::Syntax { path } => ExtractError::Syntax { path },
    }
}

fn sort_identifier_references(references: &mut Vec<IdentifierReference>) {
    references.sort_by(|left, right| {
        (left.location.line, left.location.column, left.name.as_str()).cmp(&(
            right.location.line,
            right.location.column,
            right.name.as_str(),
        ))
    });
    references.dedup();
}

fn sort_signature_references(references: &mut Vec<SignatureReference>) {
    references.sort_by(|left, right| {
        (
            left.location.line,
            left.location.column,
            left.declaration.as_str(),
            left.name.as_str(),
        )
            .cmp(&(
                right.location.line,
                right.location.column,
                right.declaration.as_str(),
                right.name.as_str(),
            ))
    });
    references.dedup();
}

fn push_class_declaration(
    declarations: &mut Vec<TopLevelDeclaration>,
    node: Node<'_>,
    source: &str,
) {
    if let Some(name) = field_text(node, "name", source).or_else(|| {
        find_first_named_descendant(node, "mixin_application_class")
            .and_then(|child| first_identifier_text(child, source))
    }) {
        declarations.push(TopLevelDeclaration {
            kind: DeclarationKind::Class,
            name,
            location: node.start_position().into(),
            range: SourceRange::from_node(node),
        });
    }
}

fn push_named_declaration(
    declarations: &mut Vec<TopLevelDeclaration>,
    node: Node<'_>,
    source: &str,
    kind: DeclarationKind,
) {
    if let Some(name) = node
        .child_by_field_name("name")
        .and_then(|child| first_identifier_text(child, source))
        .or_else(|| field_text(node, "name", source))
    {
        declarations.push(TopLevelDeclaration {
            kind,
            name,
            location: node.start_position().into(),
            range: SourceRange::from_node(node),
        });
    }
}

fn push_type_alias_declaration(
    declarations: &mut Vec<TopLevelDeclaration>,
    node: Node<'_>,
    source: &str,
) {
    let mut cursor = node.walk();
    let name = node
        .named_children(&mut cursor)
        .find(|child| matches!(child.kind(), "identifier" | "type_identifier"))
        .and_then(|child| child.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned);

    if let Some(name) = name {
        declarations.push(TopLevelDeclaration {
            kind: DeclarationKind::TypeAlias,
            name,
            location: node.start_position().into(),
            range: SourceRange::from_node(node),
        });
    }
}

fn push_variable_declarations(
    declarations: &mut Vec<TopLevelDeclaration>,
    node: Node<'_>,
    source: &str,
) {
    let mut names = Vec::new();
    collect_named_fields(
        node,
        source,
        &["static_final_declaration", "initialized_identifier"],
        &mut names,
    );

    if names.is_empty() {
        if let Some(identifier_list) = find_first_named_descendant(node, "identifier_list") {
            collect_direct_identifier_children(identifier_list, source, &mut names);
        }
    }

    declarations.extend(names.into_iter().map(|name| TopLevelDeclaration {
        kind: DeclarationKind::Variable,
        name,
        location: node.start_position().into(),
        range: SourceRange::from_node(node),
    }));
}

fn push_function_declaration(
    declarations: &mut Vec<TopLevelDeclaration>,
    node: Node<'_>,
    source: &str,
) {
    let Some(signature) = node.child_by_field_name("signature") else {
        return;
    };
    let Some(name) = field_text(signature, "name", source) else {
        return;
    };

    declarations.push(TopLevelDeclaration {
        kind: DeclarationKind::Function,
        name,
        location: node.start_position().into(),
        range: SourceRange::from_node(node),
    });
}

pub(super) fn extract_uri(uri_node: Node<'_>, source: &str) -> Option<String> {
    let literal = find_first_named_descendant(uri_node, "string_literal")?;
    unquote_dart_string(literal.utf8_text(source.as_bytes()).ok()?)
}

pub(super) fn field_text(node: Node<'_>, field_name: &str, source: &str) -> Option<String> {
    node.child_by_field_name(field_name)
        .and_then(|child| child.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn collect_named_fields(
    node: Node<'_>,
    source: &str,
    owner_kinds: &[&str],
    names: &mut Vec<String>,
) {
    if owner_kinds.contains(&node.kind()) {
        if let Some(name) = field_text(node, "name", source) {
            names.push(name);
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_named_fields(child, source, owner_kinds, names);
    }
}

pub(super) fn collect_direct_identifier_children(
    node: Node<'_>,
    source: &str,
    names: &mut Vec<String>,
) {
    let mut cursor = node.walk();
    names.extend(
        node.named_children(&mut cursor)
            .filter(|child| child.kind() == "identifier")
            .filter_map(|child| child.utf8_text(source.as_bytes()).ok())
            .map(str::to_owned),
    );
}

fn first_identifier_text(node: Node<'_>, source: &str) -> Option<String> {
    if matches!(node.kind(), "identifier" | "type_identifier") {
        return node.utf8_text(source.as_bytes()).ok().map(str::to_owned);
    }

    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(|child| first_identifier_text(child, source))
}

pub(super) fn first_named_child<'tree>(node: &Node<'tree>) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).next()
}

pub(super) fn find_first_named_descendant<'tree>(
    node: Node<'tree>,
    kind: &str,
) -> Option<Node<'tree>> {
    if node.kind() == kind {
        return Some(node);
    }

    let mut cursor = node.walk();
    find_first_named_descendant_with_cursor(node, kind, &mut cursor)
}

fn find_first_named_descendant_in<'tree>(node: Node<'tree>, kinds: &[&str]) -> Option<Node<'tree>> {
    if kinds.contains(&node.kind()) {
        return Some(node);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(found) = find_first_named_descendant_in(child, kinds) {
            return Some(found);
        }
    }

    None
}

fn find_first_named_descendant_with_cursor<'tree>(
    node: Node<'tree>,
    kind: &str,
    cursor: &mut TreeCursor<'tree>,
) -> Option<Node<'tree>> {
    for child in node.named_children(cursor) {
        if child.kind() == kind {
            return Some(child);
        }
        if let Some(found) = find_first_named_descendant(child, kind) {
            return Some(found);
        }
    }

    None
}

#[cfg(test)]
mod error_tests;
#[cfg(test)]
mod string_tests;
#[cfg(test)]
mod tests;
