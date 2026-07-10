use std::collections::BTreeSet;

use crate::DartFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct CodegenDependency {
    pub(super) name: &'static str,
    pub(super) production: bool,
}

pub(super) fn codegen_dependencies_for_file(file: &DartFile) -> BTreeSet<CodegenDependency> {
    let signals = codegen_signals(file);
    let mut dependencies = BTreeSet::new();

    if signals.contains(&CodegenSignal::Freezed) {
        dependencies.insert(dev_dependency("build_runner"));
        dependencies.insert(dev_dependency("freezed"));
    }
    if signals.contains(&CodegenSignal::JsonSerializable) {
        dependencies.insert(dev_dependency("build_runner"));
        dependencies.insert(dev_dependency("json_serializable"));
        dependencies.insert(production_dependency("json_annotation"));
    }
    if signals.contains(&CodegenSignal::Riverpod) {
        dependencies.insert(dev_dependency("build_runner"));
        dependencies.insert(dev_dependency("riverpod_generator"));
    }
    if signals.contains(&CodegenSignal::GoRouter) {
        dependencies.insert(dev_dependency("build_runner"));
        dependencies.insert(dev_dependency("go_router_builder"));
    }
    if signals.contains(&CodegenSignal::Envied) {
        dependencies.insert(dev_dependency("build_runner"));
        dependencies.insert(dev_dependency("envied_generator"));
    }

    dependencies
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum CodegenSignal {
    Freezed,
    JsonSerializable,
    Riverpod,
    GoRouter,
    Envied,
}

fn codegen_signals(file: &DartFile) -> BTreeSet<CodegenSignal> {
    let imports = file
        .imports
        .iter()
        .map(|import| import.uri.as_str())
        .collect::<Vec<_>>();
    let parts = file
        .parts
        .iter()
        .map(|part| part.uri.as_str())
        .collect::<Vec<_>>();
    let references = file
        .references
        .iter()
        .map(|reference| reference.name.as_str())
        .collect::<Vec<_>>();
    let mut signals = BTreeSet::new();

    if imports.contains(&"package:freezed_annotation/freezed_annotation.dart")
        || parts.iter().any(|part| part.ends_with(".freezed.dart"))
        || references
            .iter()
            .any(|reference| matches!(*reference, "freezed" | "Freezed" | "unfreezed" | "With"))
    {
        signals.insert(CodegenSignal::Freezed);
    }
    if imports.contains(&"package:json_annotation/json_annotation.dart")
        || parts.iter().any(|part| part.ends_with(".g.dart"))
        || references.iter().any(|reference| {
            matches!(
                *reference,
                "JsonSerializable" | "JsonKey" | "JsonEnum" | "JsonValue"
            )
        })
    {
        signals.insert(CodegenSignal::JsonSerializable);
    }
    if imports.contains(&"package:riverpod_annotation/riverpod_annotation.dart")
        || parts.iter().any(|part| part.ends_with(".g.dart"))
            && references
                .iter()
                .any(|reference| matches!(*reference, "riverpod" | "Riverpod"))
    {
        signals.insert(CodegenSignal::Riverpod);
    }
    if parts.iter().any(|part| part.ends_with(".g.dart"))
        && references.iter().any(|reference| {
            matches!(
                *reference,
                "TypedGoRoute"
                    | "TypedShellRoute"
                    | "TypedStatefulShellRoute"
                    | "TypedStatefulShellBranch"
            )
        })
    {
        signals.insert(CodegenSignal::GoRouter);
    }
    if imports.contains(&"package:envied/envied.dart")
        || references
            .iter()
            .any(|reference| matches!(*reference, "Envied" | "EnviedField"))
    {
        signals.insert(CodegenSignal::Envied);
    }

    signals
}

const fn dev_dependency(name: &'static str) -> CodegenDependency {
    CodegenDependency {
        name,
        production: false,
    }
}

const fn production_dependency(name: &'static str) -> CodegenDependency {
    CodegenDependency {
        name,
        production: true,
    }
}
