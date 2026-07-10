use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::dependencies::{
    DeclaredPackageDependency, LocalPubPackage, declared_package_dependencies,
    file_uses_codegen_dependency, local_pub_packages,
};
use crate::dependency_scripts::package_used_in_tooling;
use crate::graph::normalize_against;
use crate::output::TRACE_SCHEMA_VERSION;
use crate::{
    DartCombinatorKind, DeadCodeReport, DeclarationKind, DependencyHygieneError, DependencyKind,
    DependencySection, ScannedProject, TopLevelDeclaration,
};

/// File trace JSON envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileTraceReport {
    /// Schema identifier.
    pub schema_version: String,
    /// Typed JSON envelope discriminator.
    pub kind: String,
    /// Tool name.
    pub tool: String,
    /// Trace command name.
    pub command: String,
    /// Traced file path.
    pub path: String,
    /// Whether the file exists in the module graph.
    pub found: bool,
    /// Whether the file is reachable from entry points.
    pub reachable: bool,
    /// Whether the file is an entry point.
    pub entry_point: bool,
    /// Top-level declarations in the file.
    pub declarations: Vec<TraceDeclaration>,
    /// Outgoing non-export dependencies from the file.
    pub imports_from: Vec<TraceDependency>,
    /// Incoming dependencies to the file.
    pub imported_by: Vec<TraceDependency>,
    /// Outgoing export dependencies from the file.
    pub re_exports: Vec<TraceDependency>,
    /// Unresolved dependencies declared by the file.
    pub unresolved: Vec<TraceDependency>,
    /// Short trace interpretation.
    pub reason: String,
}

/// Symbol trace JSON envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolTraceReport {
    /// Schema identifier.
    pub schema_version: String,
    /// Typed JSON envelope discriminator.
    pub kind: String,
    /// Tool name.
    pub tool: String,
    /// Trace command name.
    pub command: String,
    /// File that should declare the symbol.
    pub path: String,
    /// Symbol being traced.
    pub symbol: String,
    /// Whether the declaration exists in the requested file.
    pub found: bool,
    /// Whether the declaration file is reachable from entry points.
    pub reachable_file: bool,
    /// Whether the declaration file is an entry point.
    pub entry_point: bool,
    /// Matched top-level declaration, if any.
    pub declaration: Option<TraceDeclaration>,
    /// Syntactic references to the symbol.
    pub direct_references: Vec<TraceReference>,
    /// Export chains that expose this file through barrels.
    pub re_export_chains: Vec<Vec<String>>,
    /// Short trace interpretation.
    pub reason: String,
}

/// Dependency trace JSON envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct DependencyTraceReport {
    /// Schema identifier.
    pub schema_version: String,
    /// Typed JSON envelope discriminator.
    pub kind: String,
    /// Tool name.
    pub tool: String,
    /// Trace command name.
    pub command: String,
    /// Dependency package being traced.
    pub dependency: String,
    /// Whether the dependency was found as either a declaration or Dart usage.
    pub found: bool,
    /// Whether the dependency is declared in at least one local pubspec.
    pub declared: bool,
    /// Local pubspec declarations for this dependency.
    pub declared_in: Vec<TracePubspecDependency>,
    /// Dart import/export directives that reference the package.
    pub importing_files: Vec<TraceDependencyDirective>,
    /// Dart has no type-only import directive; kept for Fallow-compatible shape.
    pub type_only_importers: Vec<TraceDependencyDirective>,
    /// Number of Dart import/export directives referencing the package.
    pub total_import_count: usize,
    /// Whether non-Dart tooling files reference the package.
    pub used_in_scripts: bool,
    /// Whether Dart source references the package in import/export directives.
    pub is_used: bool,
    /// Short trace interpretation.
    pub reason: String,
}

/// Declaration included in trace output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceDeclaration {
    /// Declaration kind.
    pub kind: DeclarationKind,
    /// Declared name.
    pub name: String,
    /// 1-based line.
    pub line: usize,
    /// 0-based byte column.
    pub column: usize,
}

/// Dependency edge included in trace output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceDependency {
    /// Source file.
    pub from: String,
    /// Target file or unresolved attempted target.
    pub to: String,
    /// Raw import/export/part/augment URI.
    pub specifier: String,
    /// `import`, `export`, or `part`.
    pub kind: String,
    /// 1-based line.
    pub line: usize,
    /// 0-based byte column.
    pub column: usize,
}

/// Symbol reference included in trace output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceReference {
    /// Referencing file.
    pub path: String,
    /// 1-based line.
    pub line: usize,
    /// 0-based byte column.
    pub column: usize,
    /// Whether the referencing file is reachable from entry points.
    pub reachable: bool,
}

/// Pubspec declaration included in dependency trace output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TracePubspecDependency {
    /// Package declaring the dependency.
    pub package: String,
    /// Pubspec path, root-relative where possible.
    pub pubspec_path: String,
    /// Dependency package name.
    pub dependency: String,
    /// Pubspec dependency section.
    pub section: DependencySection,
    /// 1-based line.
    pub line: usize,
    /// 0-based byte column.
    pub column: usize,
}

/// Import/export directive included in dependency trace output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceDependencyDirective {
    /// Package containing the directive, when a local pubspec owns the file.
    pub package: Option<String>,
    /// File containing the directive.
    pub path: String,
    /// `import`, `export`, `part`, or `augment`.
    pub kind: String,
    /// Raw package URI.
    pub specifier: String,
    /// Optional import prefix after `as`.
    pub prefix: Option<String>,
    /// Whether the import uses `deferred as`.
    pub deferred: bool,
    /// 1-based line.
    pub line: usize,
    /// 0-based byte column.
    pub column: usize,
}

/// Build a read-only trace for one Dart file.
#[must_use]
pub fn trace_file(
    project: &ScannedProject,
    dead_code: &DeadCodeReport,
    path: impl AsRef<Path>,
) -> FileTraceReport {
    let path = normalize_against(&project.root, path.as_ref());
    let found = project.graph.node_index(&path).is_some();
    let reachable = dead_code.reachable_files.iter().any(|file| file == &path);
    let entry_point = dead_code.entry_points.iter().any(|file| file == &path);
    let dependencies = project.graph.dependencies();

    let declarations = project
        .files
        .iter()
        .find(|file| normalize_against(&project.root, &file.path) == path)
        .map_or_else(Vec::new, |file| declarations(&file.declarations));
    let imports_from = dependencies
        .iter()
        .filter(|dependency| {
            dependency.from_path == path && dependency.kind != DependencyKind::Export
        })
        .map(|dependency| trace_resolved_dependency(&project.root, dependency))
        .collect();
    let re_exports = dependencies
        .iter()
        .filter(|dependency| {
            dependency.from_path == path && dependency.kind == DependencyKind::Export
        })
        .map(|dependency| trace_resolved_dependency(&project.root, dependency))
        .collect();
    let imported_by = dependencies
        .iter()
        .filter(|dependency| dependency.to_path == path)
        .map(|dependency| trace_resolved_dependency(&project.root, dependency))
        .collect();
    let unresolved = project
        .graph
        .unresolved()
        .iter()
        .filter(|dependency| dependency.from_path == path)
        .map(|dependency| TraceDependency {
            from: display_path(&project.root, &dependency.from_path),
            to: display_path(&project.root, &dependency.attempted_path),
            specifier: dependency.specifier.clone(),
            kind: dependency_kind(dependency.kind),
            line: dependency.location.line,
            column: dependency.location.column,
        })
        .collect();

    FileTraceReport {
        schema_version: TRACE_SCHEMA_VERSION.to_owned(),
        kind: "trace-file".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: "trace-file".to_owned(),
        path: display_path(&project.root, &path),
        found,
        reachable,
        entry_point,
        declarations,
        imports_from,
        imported_by,
        re_exports,
        unresolved,
        reason: file_reason(found, reachable, entry_point),
    }
}

/// Build a read-only trace for one top-level symbol in one Dart file.
#[must_use]
pub fn trace_symbol(
    project: &ScannedProject,
    dead_code: &DeadCodeReport,
    path: impl AsRef<Path>,
    symbol: &str,
) -> SymbolTraceReport {
    let path = normalize_against(&project.root, path.as_ref());
    let reachable_files = dead_code
        .reachable_files
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let declaration = find_declaration(project, &path, symbol);
    let found = declaration.is_some();
    let reachable_file = reachable_files.contains(&path);
    let entry_point = dead_code.entry_points.iter().any(|file| file == &path);
    let direct_references = direct_references(project, &reachable_files, symbol);
    let re_export_chains = re_export_chains(project, &path, symbol);

    SymbolTraceReport {
        schema_version: TRACE_SCHEMA_VERSION.to_owned(),
        kind: "trace-symbol".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: "trace-symbol".to_owned(),
        path: display_path(&project.root, &path),
        symbol: symbol.to_owned(),
        found,
        reachable_file,
        entry_point,
        declaration,
        reason: symbol_reason(found, reachable_file, entry_point, &direct_references),
        direct_references,
        re_export_chains,
    }
}

/// Build a read-only trace for one pub dependency.
///
/// # Errors
///
/// Returns [`DependencyHygieneError`] if local pubspec discovery or parsing fails.
pub fn trace_dependency(
    project: &ScannedProject,
    dependency: &str,
) -> Result<DependencyTraceReport, DependencyHygieneError> {
    let packages = local_pub_packages(&project.root)?;
    let mut importing_files = dependency_directives(project, &packages, dependency);
    importing_files.sort_by(|left, right| {
        (
            &left.path,
            left.line,
            left.column,
            &left.kind,
            &left.specifier,
        )
            .cmp(&(
                &right.path,
                right.line,
                right.column,
                &right.kind,
                &right.specifier,
            ))
    });

    let declared_in = declared_package_dependencies(&project.root, dependency)?
        .iter()
        .map(|declaration| trace_pubspec_dependency(&project.root, declaration))
        .collect::<Vec<_>>();
    let declared = !declared_in.is_empty();
    let total_import_count = importing_files.len();
    let used_in_scripts = packages
        .iter()
        .any(|package| package_used_in_tooling(&package.root, dependency))
        || project
            .files
            .iter()
            .any(|file| file_uses_codegen_dependency(file, dependency));
    let is_used = total_import_count > 0 || used_in_scripts;

    Ok(DependencyTraceReport {
        schema_version: TRACE_SCHEMA_VERSION.to_owned(),
        kind: "trace-dependency".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: "trace-dependency".to_owned(),
        dependency: dependency.to_owned(),
        found: declared || is_used,
        declared,
        declared_in,
        importing_files,
        type_only_importers: Vec::new(),
        total_import_count,
        used_in_scripts,
        is_used,
        reason: dependency_reason(declared, is_used),
    })
}

/// Render a concise human file trace.
#[must_use]
pub fn render_file_trace(report: &FileTraceReport) -> String {
    let mut rendered = String::new();
    let _ = writeln!(
        rendered,
        "trace-file {}: found={} reachable={} entry_point={}",
        report.path, report.found, report.reachable, report.entry_point
    );
    let _ = writeln!(rendered, "{}", report.reason);
    rendered
}

/// Render a concise human symbol trace.
#[must_use]
pub fn render_symbol_trace(report: &SymbolTraceReport) -> String {
    let mut rendered = String::new();
    let _ = writeln!(
        rendered,
        "trace-symbol {}:{}: found={} reachable_file={} references={}",
        report.path,
        report.symbol,
        report.found,
        report.reachable_file,
        report.direct_references.len()
    );
    let _ = writeln!(rendered, "{}", report.reason);
    rendered
}

/// Render a concise human dependency trace.
#[must_use]
pub fn render_dependency_trace(report: &DependencyTraceReport) -> String {
    let mut rendered = String::new();
    let _ = writeln!(
        rendered,
        "trace-dependency {}: found={} declared={} used={} imports={}",
        report.dependency, report.found, report.declared, report.is_used, report.total_import_count
    );
    let _ = writeln!(rendered, "{}", report.reason);
    rendered
}

fn declarations(declarations: &[TopLevelDeclaration]) -> Vec<TraceDeclaration> {
    declarations
        .iter()
        .map(|declaration| TraceDeclaration {
            kind: declaration.kind,
            name: declaration.name.clone(),
            line: declaration.location.line,
            column: declaration.location.column,
        })
        .collect()
}

fn trace_resolved_dependency(
    root: &Path,
    dependency: &crate::ResolvedDependency,
) -> TraceDependency {
    TraceDependency {
        from: display_path(root, &dependency.from_path),
        to: display_path(root, &dependency.to_path),
        specifier: dependency.specifier.clone(),
        kind: dependency_kind(dependency.kind),
        line: dependency.location.line,
        column: dependency.location.column,
    }
}

fn trace_pubspec_dependency(
    root: &Path,
    dependency: &DeclaredPackageDependency,
) -> TracePubspecDependency {
    TracePubspecDependency {
        package: dependency.package.clone(),
        pubspec_path: display_path(root, &dependency.pubspec_path),
        dependency: dependency.dependency.clone(),
        section: dependency.section,
        line: dependency.location.line,
        column: dependency.location.column,
    }
}

fn dependency_directives(
    project: &ScannedProject,
    packages: &[LocalPubPackage],
    dependency: &str,
) -> Vec<TraceDependencyDirective> {
    project
        .files
        .iter()
        .flat_map(|file| {
            let normalized = normalize_against(&project.root, &file.path);
            let package = owning_package(packages, &normalized).map(|package| package.name.clone());
            let path = display_path(&project.root, &normalized);
            file.imports
                .iter()
                .filter(move |import| package_name(&import.uri) == Some(dependency))
                .map({
                    let path = path.clone();
                    let package = package.clone();
                    move |import| TraceDependencyDirective {
                        package: package.clone(),
                        path: path.clone(),
                        kind: "import".to_owned(),
                        specifier: import.uri.clone(),
                        prefix: import.prefix.clone(),
                        deferred: import.deferred,
                        line: import.location.line,
                        column: import.location.column,
                    }
                })
                .chain(
                    file.exports
                        .iter()
                        .filter(move |export| package_name(&export.uri) == Some(dependency))
                        .map(move |export| TraceDependencyDirective {
                            package: package.clone(),
                            path: path.clone(),
                            kind: "export".to_owned(),
                            specifier: export.uri.clone(),
                            prefix: None,
                            deferred: false,
                            line: export.location.line,
                            column: export.location.column,
                        }),
                )
        })
        .collect()
}

fn find_declaration(
    project: &ScannedProject,
    path: &Path,
    symbol: &str,
) -> Option<TraceDeclaration> {
    project.files.iter().find_map(|file| {
        if normalize_against(&project.root, &file.path) != path {
            return None;
        }
        file.declarations
            .iter()
            .find(|declaration| declaration.name == symbol)
            .map(|declaration| TraceDeclaration {
                kind: declaration.kind,
                name: declaration.name.clone(),
                line: declaration.location.line,
                column: declaration.location.column,
            })
    })
}

fn direct_references(
    project: &ScannedProject,
    reachable_files: &BTreeSet<PathBuf>,
    symbol: &str,
) -> Vec<TraceReference> {
    let mut references = project
        .files
        .iter()
        .flat_map(|file| {
            let path = normalize_against(&project.root, &file.path);
            file.references
                .iter()
                .filter(move |reference| reference.name == symbol)
                .map(move |reference| TraceReference {
                    path: display_path(&project.root, &path),
                    line: reference.location.line,
                    column: reference.location.column,
                    reachable: reachable_files.contains(&path),
                })
        })
        .collect::<Vec<_>>();

    references.sort_by(|left, right| {
        (&left.path, left.line, left.column).cmp(&(&right.path, right.line, right.column))
    });
    references
}

fn re_export_chains(project: &ScannedProject, target: &Path, symbol: &str) -> Vec<Vec<String>> {
    let export_edges = project
        .graph
        .dependencies()
        .into_iter()
        .filter(|dependency| dependency.kind == DependencyKind::Export)
        .collect::<Vec<_>>();
    let mut chains = Vec::new();
    collect_re_export_chains(
        &project.root,
        &export_edges,
        target,
        symbol,
        &[target.to_path_buf()],
        &mut chains,
    );
    chains.sort();
    chains
}

fn collect_re_export_chains(
    root: &Path,
    export_edges: &[crate::ResolvedDependency],
    current: &Path,
    symbol: &str,
    chain: &[PathBuf],
    chains: &mut Vec<Vec<String>>,
) {
    if chain.len() > 8 {
        return;
    }

    for edge in export_edges.iter().filter(|edge| {
        edge.to_path == current && is_visible_through_export(symbol, &edge.visibility.combinators)
    }) {
        if chain.contains(&edge.from_path) {
            continue;
        }
        let mut next = chain.to_owned();
        next.push(edge.from_path.clone());
        chains.push(
            next.iter()
                .rev()
                .map(|path| display_path(root, path))
                .collect(),
        );
        collect_re_export_chains(root, export_edges, &edge.from_path, symbol, &next, chains);
    }
}

fn is_visible_through_export(name: &str, combinators: &[crate::DartCombinator]) -> bool {
    let mut visible = true;
    for combinator in combinators {
        match combinator.kind {
            DartCombinatorKind::Show => {
                visible = combinator.names.iter().any(|shown| shown == name);
            }
            DartCombinatorKind::Hide => {
                if combinator.names.iter().any(|hidden| hidden == name) {
                    visible = false;
                }
            }
        }
    }
    visible
}

fn file_reason(found: bool, reachable: bool, entry_point: bool) -> String {
    match (found, reachable, entry_point) {
        (false, _, _) => "file is not in the module graph",
        (true, _, true) => "file is configured as an entry point",
        (true, true, false) => "file is reachable from an entry point",
        (true, false, false) => "file is unreachable from configured entry points",
    }
    .to_owned()
}

fn symbol_reason(
    found: bool,
    reachable_file: bool,
    entry_point: bool,
    direct_references: &[TraceReference],
) -> String {
    if !found {
        return "symbol declaration was not found in the requested file".to_owned();
    }
    if entry_point {
        return "symbol is declared in an entry-point file".to_owned();
    }
    if !reachable_file {
        return "symbol's declaring file is unreachable".to_owned();
    }
    if direct_references
        .iter()
        .any(|reference| reference.reachable)
    {
        return "symbol has reachable direct references".to_owned();
    }
    "symbol has no reachable direct references".to_owned()
}

fn dependency_reason(declared: bool, is_used: bool) -> String {
    match (declared, is_used) {
        (true, true) => "dependency is declared and referenced by Dart import/export directives",
        (true, false) => "dependency is declared but no Dart import/export directives reference it",
        (false, true) => {
            "dependency is referenced by Dart source but not declared in a local pubspec"
        }
        (false, false) => "dependency is neither declared nor referenced by Dart source",
    }
    .to_owned()
}

fn package_name(specifier: &str) -> Option<&str> {
    specifier
        .strip_prefix("package:")
        .and_then(|rest| rest.split_once('/'))
        .map(|(package, _)| package)
}

fn owning_package<'package>(
    packages: &'package [LocalPubPackage],
    path: &Path,
) -> Option<&'package LocalPubPackage> {
    packages
        .iter()
        .filter(|package| path.starts_with(&package.root))
        .max_by_key(|package| package.root.components().count())
}

fn dependency_kind(kind: DependencyKind) -> String {
    match kind {
        DependencyKind::Import => "import",
        DependencyKind::Export => "export",
        DependencyKind::Part => "part",
        DependencyKind::Augment => "augment",
    }
    .to_owned()
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests;
