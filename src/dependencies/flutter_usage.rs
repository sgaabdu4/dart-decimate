use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::Value as JsonValue;
use serde_yaml_ng::Value;

pub(super) fn flutter_metadata_dependencies(
    package_root: &Path,
    pubspec: &Value,
) -> BTreeSet<String> {
    let mut dependencies = BTreeSet::new();
    if is_flutter_package(pubspec) {
        dependencies.insert("cupertino_icons".to_owned());
    }
    if let Some(flutter) = mapping_field(pubspec, "flutter") {
        collect_package_asset_references(flutter, &mut dependencies);
    }
    collect_native_plugins(package_root, &mut dependencies);
    dependencies
}

fn is_flutter_package(pubspec: &Value) -> bool {
    mapping_field(pubspec, "dependencies")
        .and_then(|dependencies| mapping_field(dependencies, "flutter"))
        .and_then(|flutter| mapping_field(flutter, "sdk"))
        .and_then(Value::as_str)
        == Some("flutter")
}

fn collect_package_asset_references(value: &Value, dependencies: &mut BTreeSet<String>) {
    match value {
        Value::String(value) => {
            if let Some(package) = value
                .strip_prefix("packages/")
                .and_then(|path| path.split('/').next())
                .filter(|package| !package.is_empty())
            {
                dependencies.insert(package.to_owned());
            }
        }
        Value::Sequence(values) => {
            for value in values {
                collect_package_asset_references(value, dependencies);
            }
        }
        Value::Mapping(mapping) => {
            for value in mapping.values() {
                collect_package_asset_references(value, dependencies);
            }
        }
        _ => {}
    }
}

fn collect_native_plugins(package_root: &Path, dependencies: &mut BTreeSet<String>) {
    let path = package_root.join(".flutter-plugins-dependencies");
    let Ok(source) = fs::read_to_string(path) else {
        return;
    };
    let Ok(value) = serde_json::from_str::<JsonValue>(&source) else {
        return;
    };
    for section in ["plugins", "dependencyGraph"] {
        if let Some(value) = value.get(section) {
            collect_plugin_names(value, dependencies);
        }
    }
}

fn collect_plugin_names(value: &JsonValue, dependencies: &mut BTreeSet<String>) {
    match value {
        JsonValue::Object(object) => {
            if let Some(name) = object.get("name").and_then(JsonValue::as_str) {
                dependencies.insert(name.to_owned());
            }
            for value in object.values() {
                collect_plugin_names(value, dependencies);
            }
        }
        JsonValue::Array(values) => {
            for value in values {
                collect_plugin_names(value, dependencies);
            }
        }
        _ => {}
    }
}

fn mapping_field<'value>(value: &'value Value, key: &str) -> Option<&'value Value> {
    value.as_mapping()?.get(Value::String(key.to_owned()))
}
