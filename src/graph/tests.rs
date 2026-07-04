use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::*;
use crate::{
    DartCombinator, DartCombinatorKind, DartExport, DartImport, DartLibrary, DartPart, DartPartOf,
    DartUriCondition,
};

#[test]
fn builds_edges_for_relative_imports_and_exports() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;

    let main = fixture.file(
        "lib/main.dart",
        vec![import("src/service.dart"), import("../tool.dart")],
        vec![export("src/public.dart")],
    );
    let service = fixture.file("lib/src/service.dart", vec![], vec![]);
    let tool = fixture.file("tool.dart", vec![], vec![]);
    let public = fixture.file("lib/src/public.dart", vec![], vec![]);

    let graph = build_module_graph(fixture.root(), &[main, service, tool, public])?;

    assert_eq!(graph.node_count(), 4);
    assert_eq!(graph.edge_count(), 3);
    assert!(graph.unresolved().is_empty());
    assert_eq!(
        graph
            .dependencies()
            .into_iter()
            .map(|dependency| (
                dependency.specifier,
                dependency.kind,
                strip_root(fixture.root(), &dependency.to_path)
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "src/service.dart".to_owned(),
                DependencyKind::Import,
                "lib/src/service.dart".to_owned()
            ),
            (
                "../tool.dart".to_owned(),
                DependencyKind::Import,
                "tool.dart".to_owned()
            ),
            (
                "src/public.dart".to_owned(),
                DependencyKind::Export,
                "lib/src/public.dart".to_owned()
            ),
        ]
    );

    Ok(())
}

#[test]
fn resolves_package_imports_to_pub_workspace_members() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\nworkspace:\n  - packages/*\n")?;
    fixture.write("packages/shared/pubspec.yaml", "name: shared\n")?;

    let main = fixture.file(
        "lib/main.dart",
        vec![import("package:shared/shared.dart")],
        vec![],
    );
    let shared = fixture.file("packages/shared/lib/shared.dart", vec![], vec![]);

    let graph = build_module_graph(fixture.root(), &[main, shared])?;

    assert_eq!(graph.package_names(), vec!["app", "shared"]);
    assert_eq!(graph.edge_count(), 1);
    assert_eq!(
        strip_root(fixture.root(), &graph.dependencies()[0].to_path),
        "packages/shared/lib/shared.dart"
    );

    Ok(())
}

#[test]
fn platform_environment_selects_one_configurable_import_branch()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    let mut conditional = import("web.dart");
    conditional.condition = Some(condition("dart.library.html", "true", 1));

    let main = fixture.file(
        "lib/main.dart",
        vec![import("io.dart"), conditional],
        vec![],
    );
    let io = fixture.file("lib/io.dart", vec![], vec![]);
    let web = fixture.file("lib/web.dart", vec![], vec![]);

    let graph = build_module_graph_with_options(
        fixture.root(),
        &[main, io, web],
        &GraphOptions {
            conditional_environment: [("dart.library.html".to_owned(), "true".to_owned())].into(),
        },
    )?;

    let dependencies = graph.dependencies();
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].specifier, "web.dart");
    assert_eq!(
        dependencies[0]
            .visibility
            .condition
            .as_ref()
            .map(|condition| condition.variable.as_str()),
        Some("dart.library.html")
    );
    assert_eq!(
        strip_root(fixture.root(), &dependencies[0].to_path),
        "lib/web.dart"
    );
    assert!(graph.unresolved().is_empty());

    Ok(())
}

#[test]
fn platform_environment_falls_back_to_default_configurable_import()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    let mut conditional = import("web.dart");
    conditional.condition = Some(condition("dart.library.html", "true", 1));

    let main = fixture.file(
        "lib/main.dart",
        vec![import("io.dart"), conditional],
        vec![],
    );
    let io = fixture.file("lib/io.dart", vec![], vec![]);
    let web = fixture.file("lib/web.dart", vec![], vec![]);

    let graph = build_module_graph_with_options(
        fixture.root(),
        &[main, io, web],
        &GraphOptions {
            conditional_environment: [("dart.library.html".to_owned(), "false".to_owned())].into(),
        },
    )?;

    let dependencies = graph.dependencies();
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].specifier, "io.dart");
    assert!(dependencies[0].visibility.condition.is_none());
    assert_eq!(
        strip_root(fixture.root(), &dependencies[0].to_path),
        "lib/io.dart"
    );
    assert!(graph.unresolved().is_empty());

    Ok(())
}

#[test]
fn platform_environment_selects_package_uri_branches_before_resolution()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\nworkspace:\n  - packages/*\n")?;
    fixture.write("packages/shared/pubspec.yaml", "name: shared\n")?;
    let mut conditional = import("package:shared/web.dart");
    conditional.condition = Some(condition("dart.library.html", "true", 1));

    let main = fixture.file(
        "lib/main.dart",
        vec![import("stub.dart"), conditional],
        vec![],
    );
    let stub = fixture.file("lib/stub.dart", vec![], vec![]);
    let web = fixture.file("packages/shared/lib/web.dart", vec![], vec![]);

    let graph = build_module_graph_with_options(
        fixture.root(),
        &[main, stub, web],
        &GraphOptions {
            conditional_environment: [("dart.library.html".to_owned(), "true".to_owned())].into(),
        },
    )?;

    let dependencies = graph.dependencies();
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0].specifier, "package:shared/web.dart");
    assert_eq!(
        strip_root(fixture.root(), &dependencies[0].to_path),
        "packages/shared/lib/web.dart"
    );
    assert!(graph.unresolved().is_empty());

    Ok(())
}

#[test]
fn default_graph_keeps_all_configurable_import_branches() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    let mut conditional = import("web.dart");
    conditional.condition = Some(condition("dart.library.html", "true", 1));

    let main = fixture.file(
        "lib/main.dart",
        vec![import("io.dart"), conditional],
        vec![],
    );
    let io = fixture.file("lib/io.dart", vec![], vec![]);
    let web = fixture.file("lib/web.dart", vec![], vec![]);

    let graph = build_module_graph(fixture.root(), &[main, io, web])?;

    assert_eq!(
        graph
            .dependencies()
            .into_iter()
            .map(|dependency| dependency.specifier)
            .collect::<Vec<_>>(),
        vec!["io.dart".to_owned(), "web.dart".to_owned()]
    );
    assert!(graph.unresolved().is_empty());

    Ok(())
}

#[test]
fn resolves_package_imports_using_package_config_root_uri_and_package_uri()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write(
        ".dart_tool/package_config.json",
        r#"{
  "configVersion": 2,
  "packages": [
    {"name": "app", "rootUri": "../", "packageUri": "lib/"},
    {"name": "shared", "rootUri": "../packages/shared", "packageUri": "lib/"},
    {"name": "generated", "rootUri": "../packages/generated", "packageUri": "src/"}
  ]
}
"#,
    )?;

    let main = fixture.file(
        "lib/main.dart",
        vec![
            import("package:shared/src/api.dart"),
            import("package:generated/api.dart"),
        ],
        vec![],
    );
    let shared = fixture.file("packages/shared/lib/src/api.dart", vec![], vec![]);
    let generated = fixture.file("packages/generated/src/api.dart", vec![], vec![]);

    let graph = build_module_graph(fixture.root(), &[main, shared, generated])?;

    assert_eq!(graph.package_names(), vec!["app", "generated", "shared"]);
    assert_eq!(
        graph
            .dependencies()
            .into_iter()
            .map(|dependency| strip_root(fixture.root(), &dependency.to_path))
            .collect::<Vec<_>>(),
        vec![
            "packages/shared/lib/src/api.dart".to_owned(),
            "packages/generated/src/api.dart".to_owned(),
        ]
    );
    assert!(graph.unresolved().is_empty());

    Ok(())
}

#[test]
fn package_config_takes_precedence_over_stale_pubspec_paths()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write(
        "pubspec.yaml",
        "name: app\ndependencies:\n  shared:\n    path: old_shared\n",
    )?;
    fixture.write(
        ".dart_tool/package_config.json",
        r#"{
  "configVersion": 2,
  "packages": [
    {"name": "app", "rootUri": "../", "packageUri": "lib/"},
    {"name": "shared", "rootUri": "../actual_shared", "packageUri": "lib/"}
  ]
}
"#,
    )?;
    let main = fixture.file(
        "lib/main.dart",
        vec![import("package:shared/api.dart")],
        vec![],
    );
    let actual = fixture.file("actual_shared/lib/api.dart", vec![], vec![]);
    let stale = fixture.file("old_shared/lib/api.dart", vec![], vec![]);

    let graph = build_module_graph(fixture.root(), &[main, actual, stale])?;

    assert_eq!(graph.edge_count(), 1);
    assert_eq!(
        strip_root(fixture.root(), &graph.dependencies()[0].to_path),
        "actual_shared/lib/api.dart"
    );

    Ok(())
}

#[test]
fn package_config_uri_paths_are_percent_decoded() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write(
        ".dart_tool/package_config.json",
        r#"{
  "configVersion": 2,
  "packages": [
    {"name": "shared", "rootUri": "../packages/shared%20pkg", "packageUri": "custom%20lib/"}
  ]
}
"#,
    )?;
    let main = fixture.file(
        "lib/main.dart",
        vec![import("package:shared/api.dart")],
        vec![],
    );
    let shared = fixture.file("packages/shared pkg/custom lib/api.dart", vec![], vec![]);

    let graph = build_module_graph(fixture.root(), &[main, shared])?;

    assert_eq!(graph.edge_count(), 1);
    assert_eq!(
        strip_root(fixture.root(), &graph.dependencies()[0].to_path),
        "packages/shared pkg/custom lib/api.dart"
    );

    Ok(())
}

#[test]
fn package_config_hosted_entries_remain_external() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write(
        ".dart_tool/package_config.json",
        r#"{
  "configVersion": 2,
  "packages": [
    {"name": "hosted", "rootUri": "file:///tmp/.pub-cache/hosted/hosted-1.0.0", "packageUri": "lib/"}
  ]
}
"#,
    )?;
    let main = fixture.file(
        "lib/main.dart",
        vec![import("package:hosted/hosted.dart")],
        vec![],
    );

    let graph = build_module_graph(fixture.root(), &[main])?;

    assert_eq!(graph.edge_count(), 0);
    assert!(graph.unresolved().is_empty());

    Ok(())
}

#[test]
fn package_config_prevents_nested_same_name_fallback() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write(
        "pubspec.yaml",
        "name: app\ndependencies:\n  shared: ^1.0.0\n",
    )?;
    fixture.write("functions/a/shared/pubspec.yaml", "name: shared\n")?;
    fixture.write(
        ".dart_tool/package_config.json",
        r#"{
  "configVersion": 2,
  "packages": [
    {"name": "app", "rootUri": "../", "packageUri": "lib/"},
    {"name": "shared", "rootUri": "file:///tmp/.pub-cache/hosted/shared-1.0.0", "packageUri": "lib/"}
  ]
}
"#,
    )?;
    let main = fixture.file(
        "lib/main.dart",
        vec![import("package:shared/shared.dart")],
        vec![],
    );
    let nested = fixture.file("functions/a/shared/lib/shared.dart", vec![], vec![]);

    let graph = build_module_graph(fixture.root(), &[main, nested])?;

    assert_eq!(graph.edge_count(), 0);
    assert!(graph.unresolved().is_empty());

    Ok(())
}

#[test]
fn package_config_owned_import_missing_from_config_ignores_nested_pubspec()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    fixture.write("functions/shared/pubspec.yaml", "name: shared\n")?;
    fixture.write(
        ".dart_tool/package_config.json",
        r#"{
  "configVersion": 2,
  "packages": [
    {"name": "app", "rootUri": "../", "packageUri": "lib/"}
  ]
}
"#,
    )?;
    let main = fixture.file(
        "lib/main.dart",
        vec![import("package:shared/shared.dart")],
        vec![],
    );
    let nested = fixture.file("functions/shared/lib/shared.dart", vec![], vec![]);

    let graph = build_module_graph(fixture.root(), &[main, nested])?;

    assert_eq!(graph.edge_count(), 0);
    assert!(graph.unresolved().is_empty());

    Ok(())
}

#[test]
fn package_config_keeps_nested_self_imports_local() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    fixture.write("functions/a/shared/pubspec.yaml", "name: shared\n")?;
    fixture.write(
        ".dart_tool/package_config.json",
        r#"{
  "configVersion": 2,
  "packages": [
    {"name": "app", "rootUri": "../", "packageUri": "lib/"},
    {"name": "shared", "rootUri": "file:///tmp/.pub-cache/hosted/shared-1.0.0", "packageUri": "lib/"}
  ]
}
"#,
    )?;
    let main = fixture.file(
        "functions/a/shared/lib/main.dart",
        vec![import("package:shared/src/foo.dart")],
        vec![],
    );
    let foo = fixture.file("functions/a/shared/lib/src/foo.dart", vec![], vec![]);

    let graph = build_module_graph(fixture.root(), &[main, foo])?;

    assert_eq!(graph.edge_count(), 1);
    assert_eq!(
        strip_root(fixture.root(), &graph.dependencies()[0].to_path),
        "functions/a/shared/lib/src/foo.dart"
    );

    Ok(())
}

#[test]
fn package_config_local_missing_target_is_unresolved() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write(
        ".dart_tool/package_config.json",
        r#"{
  "configVersion": 2,
  "packages": [
    {"name": "shared", "rootUri": "../packages/shared", "packageUri": "lib/"}
  ]
}
"#,
    )?;
    let main = fixture.file(
        "lib/main.dart",
        vec![import("package:shared/missing.dart")],
        vec![],
    );

    let graph = build_module_graph(fixture.root(), &[main])?;

    assert_eq!(graph.edge_count(), 0);
    assert_eq!(graph.unresolved().len(), 1);
    assert_eq!(
        strip_root(fixture.root(), &graph.unresolved()[0].attempted_path),
        "packages/shared/lib/missing.dart"
    );

    Ok(())
}

#[test]
fn preserves_import_and_export_visibility_metadata_on_edges()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    let main = DartFile {
        path: fixture.path("lib/main.dart"),
        library: None,
        part_of: None,
        imports: vec![DartImport {
            uri: "src/service.dart".to_owned(),
            condition: None,
            prefix: Some("svc".to_owned()),
            deferred: true,
            combinators: vec![
                combinator(DartCombinatorKind::Show, &["Service"]),
                combinator(DartCombinatorKind::Hide, &["Hidden"]),
            ],
            location: Location { line: 1, column: 0 },
        }],
        exports: vec![DartExport {
            uri: "src/public.dart".to_owned(),
            condition: None,
            combinators: vec![combinator(DartCombinatorKind::Show, &["PublicApi"])],
            location: Location { line: 2, column: 0 },
        }],
        parts: vec![],
        declarations: vec![],
        members: vec![],
        references: vec![],
        signature_references: vec![],
        routes: vec![],
    };
    let service = fixture.file("lib/src/service.dart", vec![], vec![]);
    let public = fixture.file("lib/src/public.dart", vec![], vec![]);

    let graph = build_module_graph(fixture.root(), &[main, service, public])?;
    let dependencies = graph.dependencies();

    assert_eq!(dependencies[0].visibility.prefix.as_deref(), Some("svc"));
    assert!(dependencies[0].visibility.deferred);
    assert_eq!(
        dependencies[0]
            .visibility
            .combinators
            .iter()
            .map(|combinator| (combinator.kind, combinator.names.as_slice()))
            .collect::<Vec<_>>(),
        vec![
            (DartCombinatorKind::Show, &["Service".to_owned()][..]),
            (DartCombinatorKind::Hide, &["Hidden".to_owned()][..]),
        ]
    );
    assert_eq!(
        dependencies[1].visibility.combinators[0].names,
        vec!["PublicApi"]
    );

    Ok(())
}

#[test]
fn builds_edges_for_part_directives() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    let mut library = fixture.file("lib/model.dart", vec![], vec![]);
    library.parts = vec![part("src/model.g.dart")];
    let mut generated = fixture.file("lib/src/model.g.dart", vec![], vec![]);
    generated.part_of = Some(part_of_uri("../model.dart"));

    let graph = build_module_graph(fixture.root(), &[library, generated])?;

    assert_eq!(graph.edge_count(), 1);
    assert!(graph.invalid_part_relationships().is_empty());
    assert_eq!(graph.dependencies()[0].kind, DependencyKind::Part);
    assert_eq!(
        strip_root(fixture.root(), &graph.dependencies()[0].to_path),
        "lib/src/model.g.dart"
    );

    Ok(())
}

#[test]
fn reports_missing_part_of_for_resolved_part() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    let mut library = fixture.file("lib/model.dart", vec![], vec![]);
    library.parts = vec![part("src/model.g.dart")];
    let generated = fixture.file("lib/src/model.g.dart", vec![], vec![]);

    let graph = build_module_graph(fixture.root(), &[library, generated])?;

    assert_eq!(graph.invalid_part_relationships().len(), 1);
    assert_eq!(
        graph.invalid_part_relationships()[0].reason,
        InvalidPartReason::MissingPartOf
    );
    assert_eq!(graph.edge_count(), 0);

    Ok(())
}

#[test]
fn reports_part_of_uri_mismatch_for_resolved_part() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    let mut library = fixture.file("lib/model.dart", vec![], vec![]);
    library.parts = vec![part("src/model.g.dart")];
    let mut generated = fixture.file("lib/src/model.g.dart", vec![], vec![]);
    generated.part_of = Some(part_of_uri("../other.dart"));

    let graph = build_module_graph(fixture.root(), &[library, generated])?;

    assert_eq!(
        graph.invalid_part_relationships()[0].reason,
        InvalidPartReason::PartOfUriMismatch {
            expected_path: fixture.path("lib/model.dart"),
            actual_path: fixture.path("lib/other.dart"),
            actual_specifier: "../other.dart".to_owned()
        }
    );

    Ok(())
}

#[test]
fn reports_part_of_name_mismatch_for_resolved_part() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    let mut library = fixture.file("lib/model.dart", vec![], vec![]);
    library.library = Some(DartLibrary {
        name: Some("app.model".to_owned()),
        augment_uri: None,
        location: Location { line: 1, column: 0 },
    });
    library.parts = vec![part("src/model.g.dart")];
    let mut generated = fixture.file("lib/src/model.g.dart", vec![], vec![]);
    generated.part_of = Some(part_of_name("app.other"));

    let graph = build_module_graph(fixture.root(), &[library, generated])?;

    assert_eq!(
        graph.invalid_part_relationships()[0].reason,
        InvalidPartReason::PartOfNameMismatch {
            expected_name: Some("app.model".to_owned()),
            actual_name: "app.other".to_owned()
        }
    );

    Ok(())
}

#[test]
fn reports_orphan_part_of_without_owning_part_directive() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    let library = fixture.file("lib/model.dart", vec![], vec![]);
    let mut generated = fixture.file("lib/src/model.g.dart", vec![], vec![]);
    generated.part_of = Some(part_of_uri("../model.dart"));

    let graph = build_module_graph(fixture.root(), &[library, generated])?;

    assert_eq!(
        graph.invalid_part_relationships()[0].reason,
        InvalidPartReason::OrphanPartOf {
            actual_name: None,
            actual_specifier: Some("../model.dart".to_owned())
        }
    );

    Ok(())
}

#[test]
fn resolves_package_imports_to_path_dependencies() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write(
        "pubspec.yaml",
        "name: app\ndependencies:\n  shared:\n    path: ../shared\n",
    )?;
    let shared_root = fixture.path("../shared");
    fs::create_dir_all(shared_root.join("lib"))?;
    fs::write(shared_root.join("pubspec.yaml"), "name: shared\n")?;

    let main = fixture.file(
        "lib/main.dart",
        vec![import("package:shared/src/api.dart")],
        vec![],
    );
    let shared = DartFile {
        path: shared_root.join("lib/src/api.dart"),
        library: None,
        part_of: None,
        imports: vec![],
        exports: vec![],
        parts: vec![],
        declarations: vec![],
        members: vec![],
        references: vec![],
        signature_references: vec![],
        routes: vec![],
    };

    let graph = build_module_graph(fixture.root(), &[main, shared])?;

    assert_eq!(graph.package_names(), vec!["app", "shared"]);
    assert_eq!(graph.edge_count(), 1);

    Ok(())
}

#[test]
fn resolves_package_imports_to_nested_local_pubspecs() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: repo\n")?;
    fixture.write("examples/app/pubspec.yaml", "name: app\n")?;

    let main = fixture.file(
        "examples/app/lib/main.dart",
        vec![import("package:app/src/widget.dart")],
        vec![],
    );
    let widget = fixture.file("examples/app/lib/src/widget.dart", vec![], vec![]);

    let graph = build_module_graph(fixture.root(), &[main, widget])?;

    assert_eq!(graph.package_names(), vec!["app", "repo"]);
    assert_eq!(graph.edge_count(), 1);
    assert!(graph.unresolved().is_empty());

    Ok(())
}

#[test]
fn excludes_external_packages_and_records_missing_local_targets()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\n")?;
    let main = fixture.file(
        "lib/main.dart",
        vec![
            import("dart:async"),
            import("package:http/http.dart"),
            import("src/missing.dart"),
        ],
        vec![],
    );

    let graph = build_module_graph(fixture.root(), &[main])?;

    assert_eq!(graph.edge_count(), 0);
    assert_eq!(graph.unresolved().len(), 1);
    assert_eq!(graph.unresolved()[0].specifier, "src/missing.dart");
    assert_eq!(
        strip_root(fixture.root(), &graph.unresolved()[0].attempted_path),
        "lib/src/missing.dart"
    );

    Ok(())
}

#[test]
fn records_unresolved_known_package_import_target() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write("pubspec.yaml", "name: app\nworkspace:\n  - packages/*\n")?;
    fixture.write("packages/shared/pubspec.yaml", "name: shared\n")?;
    let main = fixture.file(
        "lib/main.dart",
        vec![import("package:shared/missing.dart")],
        vec![],
    );

    let graph = build_module_graph(fixture.root(), &[main])?;

    assert_eq!(graph.edge_count(), 0);
    assert_eq!(graph.unresolved().len(), 1);
    assert_eq!(
        graph.unresolved()[0].specifier,
        "package:shared/missing.dart"
    );
    assert_eq!(
        strip_root(fixture.root(), &graph.unresolved()[0].attempted_path),
        "packages/shared/lib/missing.dart"
    );

    Ok(())
}

#[test]
fn malformed_package_config_is_a_graph_error() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    fixture.write(
        ".dart_tool/package_config.json",
        r#"{ "configVersion": 2, "packages": ["#,
    )?;
    let main = fixture.file("lib/main.dart", vec![], vec![]);

    match build_module_graph(fixture.root(), &[main]) {
        Err(GraphError::ParsePackageConfig { .. }) => {}
        other => panic!("expected package config parse error, got {other:?}"),
    }

    Ok(())
}

#[test]
fn keeps_current_directory_when_normalizing_dot() {
    assert_eq!(normalize_path(Path::new(".")), PathBuf::from("."));
}

#[test]
fn does_not_join_paths_that_already_start_with_root() {
    assert_eq!(
        normalize_against(Path::new("repo/app"), Path::new("repo/app/lib/main.dart")),
        PathBuf::from("repo/app/lib/main.dart")
    );
}

fn import(uri: &str) -> DartImport {
    DartImport {
        uri: uri.to_owned(),
        condition: None,
        prefix: None,
        deferred: false,
        combinators: Vec::new(),
        location: Location { line: 1, column: 0 },
    }
}

fn export(uri: &str) -> DartExport {
    DartExport {
        uri: uri.to_owned(),
        condition: None,
        combinators: Vec::new(),
        location: Location { line: 1, column: 0 },
    }
}

fn part(uri: &str) -> DartPart {
    DartPart {
        uri: uri.to_owned(),
        location: Location { line: 1, column: 0 },
    }
}

fn part_of_uri(uri: &str) -> DartPartOf {
    DartPartOf {
        name: None,
        uri: Some(uri.to_owned()),
        location: Location { line: 1, column: 0 },
    }
}

fn part_of_name(name: &str) -> DartPartOf {
    DartPartOf {
        name: Some(name.to_owned()),
        uri: None,
        location: Location { line: 1, column: 0 },
    }
}

fn combinator(kind: DartCombinatorKind, names: &[&str]) -> DartCombinator {
    DartCombinator {
        kind,
        names: names.iter().map(|name| (*name).to_owned()).collect(),
        location: Location { line: 1, column: 0 },
    }
}

fn condition(variable: &str, expected_value: &str, line: usize) -> DartUriCondition {
    DartUriCondition {
        variable: variable.to_owned(),
        expected_value: expected_value.to_owned(),
        location: Location { line, column: 0 },
    }
}

fn strip_root(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

struct Fixture {
    temp: TempDir,
}

impl Fixture {
    fn new() -> Result<Self, std::io::Error> {
        tempfile::tempdir().map(|temp| Self { temp })
    }

    fn root(&self) -> &Path {
        self.temp.path()
    }

    fn path(&self, path: &str) -> PathBuf {
        normalize_path(&self.root().join(path))
    }

    fn write(&self, path: &str, source: &str) -> Result<(), std::io::Error> {
        let path = self.path(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, source)
    }

    fn file(&self, path: &str, imports: Vec<DartImport>, exports: Vec<DartExport>) -> DartFile {
        DartFile {
            path: self.path(path),
            library: None,
            part_of: None,
            imports,
            exports,
            parts: vec![],
            declarations: vec![],
            members: vec![],
            references: vec![],
            signature_references: vec![],
            routes: vec![],
        }
    }
}
