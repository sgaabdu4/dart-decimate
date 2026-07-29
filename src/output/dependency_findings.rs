use std::path::Path;

use super::format::{dependency_kind, display_path};
use super::{Finding, FindingAction, FindingEdge, FindingKind, Severity};
use crate::{
    DependencyHygieneReport, DependencyIssue, DependencyOverrideMisconfigReason, DependencySection,
    MisconfiguredDependencyOverride, PrivateSrcImport, UnlistedPackageDependency,
    UnusedPackageDependency,
};

pub(super) fn add_dependency_hygiene_findings(
    root: &Path,
    report: &DependencyHygieneReport,
    findings: &mut Vec<Finding>,
) {
    findings.extend(
        report
            .unused_dependencies
            .iter()
            .map(|dependency| unused_dependency_finding(root, dependency)),
    );
    findings.extend(
        report
            .misconfigured_dependency_overrides
            .iter()
            .map(|dependency| misconfigured_override_finding(root, dependency)),
    );
    findings.extend(
        report
            .unlisted_dependencies
            .iter()
            .map(|dependency| unlisted_dependency_finding(root, dependency)),
    );
    findings.extend(
        report
            .private_src_imports
            .iter()
            .map(|dependency| private_src_import_finding(root, dependency)),
    );
}

fn unused_dependency_finding(root: &Path, dependency: &UnusedPackageDependency) -> Finding {
    let path = display_path(root, &dependency.pubspec_path);
    Finding {
        rule_id: rule_id(dependency.issue).to_owned(),
        fingerprint: None,
        kind: finding_kind(dependency.issue),
        severity: severity(dependency.issue),
        message: message(dependency),
        path: path.clone(),
        line: dependency.location.line,
        column: dependency.location.column,
        safe_to_delete: dependency.safe_to_delete,
        files: Vec::new(),
        edge: None,
        actions: vec![
            FindingAction::new(
                action(dependency),
                action_description(dependency),
                dependency.safe_to_delete,
            )
            .with_target_path(path.clone())
            .with_target_dependency(dependency.dependency.clone())
            .with_config_key(unused_action_config_key(dependency))
            .with_dart_decimate_args([
                "trace-dependency",
                "--format",
                "json",
                "--dependency",
                dependency.dependency.as_str(),
            ]),
        ],
    }
}

const fn finding_kind(issue: DependencyIssue) -> FindingKind {
    match issue {
        DependencyIssue::UnusedRuntimeDependency => FindingKind::UnusedDependency,
        DependencyIssue::UnusedDevDependency => FindingKind::UnusedDevDependency,
        DependencyIssue::DevDependencyInProduction => FindingKind::DevDependencyInProduction,
        DependencyIssue::TestOnlyDependency => FindingKind::TestOnlyDependency,
        DependencyIssue::UnusedDependencyOverride => FindingKind::UnusedDependencyOverride,
    }
}

fn rule_id(issue: DependencyIssue) -> &'static str {
    match issue {
        DependencyIssue::UnusedRuntimeDependency => "dart-decimate/unused-dependency",
        DependencyIssue::UnusedDevDependency => "dart-decimate/unused-dev-dependency",
        DependencyIssue::DevDependencyInProduction => "dart-decimate/dev-dependency-in-production",
        DependencyIssue::TestOnlyDependency => "dart-decimate/test-only-dependency",
        DependencyIssue::UnusedDependencyOverride => "dart-decimate/unused-dependency-override",
    }
}

const fn severity(issue: DependencyIssue) -> Severity {
    match issue {
        DependencyIssue::UnusedDependencyOverride | DependencyIssue::DevDependencyInProduction => {
            Severity::Warning
        }
        _ => Severity::Error,
    }
}

fn message(dependency: &UnusedPackageDependency) -> String {
    match dependency.issue {
        DependencyIssue::UnusedRuntimeDependency => format!(
            "{} declares unused pub dependency {}",
            dependency.package, dependency.dependency
        ),
        DependencyIssue::UnusedDevDependency => format!(
            "{} declares unused dev dependency {}",
            dependency.package, dependency.dependency
        ),
        DependencyIssue::DevDependencyInProduction => format!(
            "{} imports dev dependency {} from production code",
            dependency.package, dependency.dependency
        ),
        DependencyIssue::TestOnlyDependency => format!(
            "{} declares runtime dependency {} that is only imported from dev/test files",
            dependency.package, dependency.dependency
        ),
        DependencyIssue::UnusedDependencyOverride => format!(
            "{} declares unused dependency override {} that is absent from pubspec.lock resolved packages",
            dependency.package, dependency.dependency
        ),
    }
}

fn action(dependency: &UnusedPackageDependency) -> &'static str {
    match dependency.issue {
        DependencyIssue::UnusedRuntimeDependency | DependencyIssue::UnusedDevDependency
            if dependency.safe_to_delete =>
        {
            "remove-pubspec-dependency"
        }
        DependencyIssue::UnusedRuntimeDependency | DependencyIssue::UnusedDevDependency => {
            "review-pubspec-dependency"
        }
        DependencyIssue::DevDependencyInProduction => "move-pubspec-dependency-to-dependencies",
        DependencyIssue::TestOnlyDependency => "move-pubspec-dependency-to-dev-dependencies",
        DependencyIssue::UnusedDependencyOverride => "review-unused-dependency-override",
    }
}

fn action_description(dependency: &UnusedPackageDependency) -> &'static str {
    match dependency.issue {
        DependencyIssue::UnusedRuntimeDependency | DependencyIssue::UnusedDevDependency
            if dependency.safe_to_delete =>
        {
            "Remove this simple unused pubspec dependency"
        }
        DependencyIssue::UnusedRuntimeDependency | DependencyIssue::UnusedDevDependency => {
            "Review non-Dart usage such as build tools before removing this pubspec dependency"
        }
        DependencyIssue::DevDependencyInProduction => {
            "Move the package from dev_dependencies to dependencies"
        }
        DependencyIssue::TestOnlyDependency => {
            "Move the package from dependencies to dev_dependencies after checking runtime usage"
        }
        DependencyIssue::UnusedDependencyOverride => {
            "Review the dependency override against pubspec.lock before removing it"
        }
    }
}

fn misconfigured_override_finding(
    root: &Path,
    dependency: &MisconfiguredDependencyOverride,
) -> Finding {
    let path = display_path(root, &dependency.pubspec_path);
    Finding {
        rule_id: "dart-decimate/misconfigured-dependency-override".to_owned(),
        fingerprint: None,
        kind: FindingKind::MisconfiguredDependencyOverride,
        severity: Severity::Error,
        message: misconfigured_override_message(dependency),
        path: path.clone(),
        line: dependency.location.line,
        column: dependency.location.column,
        safe_to_delete: false,
        files: Vec::new(),
        edge: None,
        actions: vec![
            FindingAction::new(
                "fix-dependency-override",
                "Fix the dependency_overrides key or value so Pub can honor it",
                false,
            )
            .with_target_path(path)
            .with_target_dependency(
                dependency
                    .dependency
                    .clone()
                    .unwrap_or_else(|| dependency.raw_key.clone()),
            )
            .with_config_key("dependency_overrides"),
        ],
    }
}

fn misconfigured_override_message(dependency: &MisconfiguredDependencyOverride) -> String {
    let reason = match dependency.reason {
        DependencyOverrideMisconfigReason::UnparsableKey => "has an invalid package name",
        DependencyOverrideMisconfigReason::EmptyValue => "has an empty value",
    };
    format!(
        "{} declares dependency override {} that {reason}",
        dependency.package, dependency.raw_key
    )
}

fn unlisted_dependency_finding(root: &Path, dependency: &UnlistedPackageDependency) -> Finding {
    let path = display_path(root, &dependency.path);
    let pubspec_path = display_path(root, &dependency.pubspec_path);
    Finding {
        rule_id: "dart-decimate/unlisted-dependency".to_owned(),
        fingerprint: None,
        kind: FindingKind::UnlistedDependency,
        severity: Severity::Error,
        message: unlisted_message(dependency),
        path: path.clone(),
        line: dependency.location.line,
        column: dependency.location.column,
        safe_to_delete: false,
        files: Vec::new(),
        edge: Some(FindingEdge {
            from: display_path(root, &dependency.path),
            to: dependency.dependency.clone(),
            specifier: dependency.specifier.clone(),
            kind: dependency_kind(dependency.kind),
        }),
        actions: vec![
            FindingAction::new(
                unlisted_action(dependency),
                unlisted_action_description(dependency),
                false,
            )
            .with_target_path(pubspec_path)
            .with_target_dependency(dependency.dependency.clone())
            .with_config_key("dependencies")
            .with_dart_decimate_args([
                "trace-dependency",
                "--format",
                "json",
                "--dependency",
                dependency.dependency.as_str(),
            ]),
        ],
    }
}

const fn dependency_section_key(section: DependencySection) -> &'static str {
    match section {
        DependencySection::Dependencies => "dependencies",
        DependencySection::DevDependencies => "dev_dependencies",
        DependencySection::DependencyOverrides => "dependency_overrides",
    }
}

const fn unused_action_config_key(dependency: &UnusedPackageDependency) -> &'static str {
    match dependency.issue {
        DependencyIssue::DevDependencyInProduction => "dependencies",
        DependencyIssue::TestOnlyDependency => "dev_dependencies",
        _ => dependency_section_key(dependency.section),
    }
}

fn unlisted_message(dependency: &UnlistedPackageDependency) -> String {
    if dependency.declared_section == Some(DependencySection::DevDependencies) {
        return format!(
            "{} imports {} from runtime code but declares it only in dev_dependencies",
            dependency.package, dependency.dependency
        );
    }
    if dependency.declared_section == Some(DependencySection::DependencyOverrides) {
        return format!(
            "{} imports {} but declares it only in dependency_overrides",
            dependency.package, dependency.dependency
        );
    }
    format!(
        "{} imports {} but does not declare it in pubspec.yaml",
        dependency.package, dependency.dependency
    )
}

fn unlisted_action(dependency: &UnlistedPackageDependency) -> &'static str {
    if dependency.declared_section == Some(DependencySection::DevDependencies) {
        "move-pubspec-dependency-to-dependencies"
    } else {
        "add-pubspec-dependency"
    }
}

fn unlisted_action_description(dependency: &UnlistedPackageDependency) -> &'static str {
    if dependency.declared_section == Some(DependencySection::DevDependencies) {
        "Move the imported package from dev_dependencies to dependencies"
    } else if dependency.declared_section == Some(DependencySection::DependencyOverrides) {
        "Declare the imported package in dependencies or dev_dependencies; dependency_overrides alone does not add it"
    } else {
        "Add the imported package to the importing package's pubspec.yaml"
    }
}

fn private_src_import_finding(root: &Path, dependency: &PrivateSrcImport) -> Finding {
    let path = display_path(root, &dependency.path);
    Finding {
        rule_id: "dart-decimate/private-src-import".to_owned(),
        fingerprint: None,
        kind: FindingKind::PrivateSrcImport,
        severity: Severity::Error,
        message: format!(
            "{} imports private implementation library {} from package {}",
            dependency.package, dependency.specifier, dependency.dependency
        ),
        path: path.clone(),
        line: dependency.location.line,
        column: dependency.location.column,
        safe_to_delete: false,
        files: Vec::new(),
        edge: Some(FindingEdge {
            from: path.clone(),
            to: dependency.dependency.clone(),
            specifier: dependency.specifier.clone(),
            kind: dependency_kind(dependency.kind),
        }),
        actions: vec![
            FindingAction::new(
                "replace-package-private-import",
                "Import a public library from the package or move shared code behind a public API",
                false,
            )
            .with_target_path(path)
            .with_target_dependency(dependency.dependency.clone())
            .with_suppression_comment("// dart-decimate-ignore-next-line private-src-import"),
        ],
    }
}
