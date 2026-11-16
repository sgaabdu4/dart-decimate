use std::fs;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::*;

const EMBEDDER: &str =
    "embedded_libs:\n  \"dart:core\": \"core/core.dart\"\n  \"dart:ui\": \"ui/ui.dart\"\n";
const LIBRARIES: &str = "{\"vm_common\":{\"libraries\":{\"core\":{\"uri\":\"core/core.dart\"},\"async\":{\"uri\":\"async/async.dart\"}}},\"vm\":{\"libraries\":{\"io\":{\"uri\":\"io/io.dart\"}}}}";

struct Layout {
    project: TempDir,
    toolchain: TempDir,
}

fn write(root: &Path, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = root.join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}

fn config(toolchain: &Path, extra: &str, generator: &str) -> String {
    format!(
        "{{\"configVersion\":2,\"generatorVersion\":\"{generator}\",\"flutterRoot\":\"file://{root}\",\"packages\":[{extra}{{\"name\":\"app\",\"rootUri\":\"../\",\"packageUri\":\"lib/\"}}]}}",
        root = toolchain.join("flutter").display(),
    )
}

fn sky_engine(toolchain: &Path) -> String {
    format!(
        "{{\"name\":\"sky_engine\",\"rootUri\":\"file://{}\",\"packageUri\":\"lib/\"}},",
        toolchain.join("flutter/bin/cache/pkg/sky_engine").display()
    )
}

fn layout(with_sky_engine: bool, sdk_version: &str) -> Result<Layout, Box<dyn std::error::Error>> {
    let project = tempfile::tempdir()?;
    let toolchain = tempfile::tempdir()?;
    let flutter = toolchain.path().join("flutter");
    write(&flutter, "bin/cache/dart-sdk/version", sdk_version)?;
    write(&flutter, "bin/cache/dart-sdk/lib/libraries.json", LIBRARIES)?;
    write(
        &flutter,
        "bin/cache/pkg/sky_engine/lib/_embedder.yaml",
        EMBEDDER,
    )?;
    write(project.path(), "pubspec.yaml", "name: app\n")?;
    write(project.path(), "lib/main.dart", "void main() {}\n")?;
    let extra = if with_sky_engine {
        sky_engine(toolchain.path())
    } else {
        String::new()
    };
    write(
        project.path(),
        ".dart_tool/package_config.json",
        &config(toolchain.path(), &extra, "3.14.0"),
    )?;
    Ok(Layout { project, toolchain })
}

fn library(sources: &OutsideSources, name: &str) -> Option<PathBuf> {
    match sources.resolve(
        &PathBuf::from("/project/lib/main.dart"),
        &format!("dart:{name}"),
        false,
    ) {
        Target::Outside(path) => Some(path),
        Target::Local | Target::Unknown => None,
    }
}

#[test]
fn flutter_application_reads_dart_libraries_from_the_embedder_map()
-> Result<(), Box<dyn std::error::Error>> {
    let layout = layout(true, "3.14.0\n")?;

    let sources = OutsideSources::discover_with_path(layout.project.path(), None)
        .ok_or("no outside sources")?;

    let sky = layout
        .toolchain
        .path()
        .join("flutter/bin/cache/pkg/sky_engine/lib");
    assert_eq!(library(&sources, "ui"), Some(sky.join("ui/ui.dart")));
    assert_eq!(library(&sources, "core"), Some(sky.join("core/core.dart")));
    assert_eq!(library(&sources, "io"), None);

    Ok(())
}

#[test]
fn dart_project_reads_dart_libraries_from_the_flutter_root_sdk()
-> Result<(), Box<dyn std::error::Error>> {
    let layout = layout(false, "3.14.0")?;

    let sources = OutsideSources::discover_with_path(layout.project.path(), None)
        .ok_or("no outside sources")?;

    let lib = layout
        .toolchain
        .path()
        .join("flutter/bin/cache/dart-sdk/lib");
    assert_eq!(library(&sources, "io"), Some(lib.join("io/io.dart")));
    assert_eq!(
        library(&sources, "async"),
        Some(lib.join("async/async.dart"))
    );
    assert_eq!(library(&sources, "ui"), None);

    Ok(())
}

#[test]
fn dart_project_without_flutter_root_reads_the_sdk_on_path_when_the_version_matches()
-> Result<(), Box<dyn std::error::Error>> {
    let cases = [("3.14.0", true), ("3.13.0", false)];
    for (version, found) in cases {
        let layout = layout(false, "3.14.0")?;
        let sdk = layout.toolchain.path().join("path-sdk");
        write(&sdk, "version", version)?;
        write(&sdk, "lib/libraries.json", LIBRARIES)?;
        write(&sdk, "bin/dart", "")?;
        write(
            layout.project.path(),
            ".dart_tool/package_config.json",
            "{\"configVersion\":2,\"generatorVersion\":\"3.14.0\",\"packages\":[{\"name\":\"app\",\"rootUri\":\"../\",\"packageUri\":\"lib/\"}]}",
        )?;
        let path_var = std::env::join_paths([sdk.join("bin")])?;

        let sources =
            OutsideSources::discover_with_path(layout.project.path(), Some(path_var.as_os_str()));

        assert_eq!(sources.is_some(), found, "{version}");
        if let Some(sources) = sources {
            assert_eq!(
                library(&sources, "io"),
                Some(fs::canonicalize(&sdk)?.join("lib/io/io.dart"))
            );
        }
    }

    Ok(())
}

#[test]
fn no_package_config_or_other_sdk_version_means_no_outside_reading()
-> Result<(), Box<dyn std::error::Error>> {
    let mismatched = layout(true, "3.13.0")?;
    let missing = layout(true, "3.14.0")?;
    fs::remove_file(
        missing
            .project
            .path()
            .join(".dart_tool/package_config.json"),
    )?;

    assert!(OutsideSources::discover_with_path(mismatched.project.path(), None).is_none());
    assert!(OutsideSources::discover_with_path(missing.project.path(), None).is_none());

    Ok(())
}

#[test]
fn package_uris_resolve_outside_the_project_only() -> Result<(), Box<dyn std::error::Error>> {
    let layout = layout(true, "3.14.0")?;
    let from = layout.project.path().join("lib/main.dart");
    let sources = OutsideSources::discover_with_path(layout.project.path(), None)
        .ok_or("no outside sources")?
        .with_scanned(Rc::new(BTreeSet::from([layout
            .project
            .path()
            .join("lib/other.dart")])));
    let sky = layout
        .toolchain
        .path()
        .join("flutter/bin/cache/pkg/sky_engine/lib");

    assert_eq!(
        sources.resolve(&from, "package:sky_engine/ui/ui.dart", false),
        Target::Outside(sky.join("ui/ui.dart"))
    );
    assert_eq!(
        sources.resolve(&from, "package:app/other.dart", false),
        Target::Local
    );
    assert_eq!(sources.resolve(&from, "other.dart", false), Target::Local);
    assert_eq!(
        sources.resolve(&from, "package:app/ignored.dart", false),
        Target::Outside(layout.project.path().join("lib/ignored.dart"))
    );
    assert_eq!(
        sources.resolve(&from, "package:missing/x.dart", false),
        Target::Unknown
    );
    assert_eq!(sources.resolve(&from, "dart:html", false), Target::Unknown);
    assert_eq!(
        sources.resolve(&sky.join("ui/ui.dart"), "painting.dart", true),
        Target::Outside(sky.join("ui/painting.dart"))
    );

    Ok(())
}

#[test]
fn readers_that_share_a_loader_parse_each_outside_file_once()
-> Result<(), Box<dyn std::error::Error>> {
    let layout = layout(true, "3.14.0")?;
    let ui = layout
        .toolchain
        .path()
        .join("flutter/bin/cache/pkg/sky_engine/lib/ui/ui.dart");
    write(
        layout.toolchain.path(),
        "flutter/bin/cache/pkg/sky_engine/lib/ui/ui.dart",
        "class Window {}\n",
    )?;
    let parses = crate::dart_parser::track_parse_count(&ui);
    let loader = SharedLoader::default();
    let imports = [crate::DartImport {
        uri: "dart:ui".to_owned(),
        condition: None,
        prefix: None,
        deferred: false,
        combinators: Vec::new(),
        location: crate::Location { line: 1, column: 0 },
    }];
    let library = layout.project.path().join("lib/main.dart");

    for _ in 0..3 {
        let mut types =
            OutsideTypes::with_loader(layout.project.path(), loader.clone(), &Rc::default());
        assert!(types.link(&library, &imports, "Window", false).is_some());
    }

    assert_eq!(parses.count(), 1);

    Ok(())
}
