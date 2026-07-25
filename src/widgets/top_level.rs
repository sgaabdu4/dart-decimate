use std::path::Path;

use tree_sitter::Node;

use crate::{WidgetTopLevelFunction, extract::declared_return_type};

pub(super) fn top_level_widget_functions(
    path: &Path,
    root: Node<'_>,
    source: &str,
    has_widget_class: bool,
) -> Vec<WidgetTopLevelFunction> {
    if !has_widget_class && !path_looks_like_widget_file(path) && !imports_flutter_ui(root, source)
    {
        return Vec::new();
    }

    let mut functions = Vec::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if !matches!(
            child.kind(),
            "function_declaration" | "external_function_declaration"
        ) {
            continue;
        }
        let Some(function) = function_candidate(path, child, source) else {
            continue;
        };
        functions.push(function);
    }
    functions
}

fn function_candidate(path: &Path, node: Node<'_>, source: &str) -> Option<WidgetTopLevelFunction> {
    let signature = node.child_by_field_name("signature")?;
    let name_node = signature.child_by_field_name("name")?;
    let name = name_node.utf8_text(source.as_bytes()).ok()?.to_owned();
    if name == "main" {
        return None;
    }

    let signature_text = signature.utf8_text(source.as_bytes()).ok()?;
    let return_type = declared_return_type(signature, source);
    let returns_widget = return_type.as_deref().is_some_and(return_type_has_ui_type);
    let build_helper =
        is_build_helper_name(&name) && (signature_text.contains("BuildContext") || returns_widget);
    if !build_helper && !returns_widget {
        return None;
    }

    Some(WidgetTopLevelFunction {
        path: path.to_path_buf(),
        function_name: name,
        return_type,
        location: name_node.start_position().into(),
    })
}

fn return_type_has_ui_type(return_type: &str) -> bool {
    return_type
        .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .any(|token| {
            matches!(
                token,
                "Widget" | "PreferredSizeWidget" | "SliverPersistentHeaderDelegate"
            )
        })
}

fn is_build_helper_name(name: &str) -> bool {
    name.strip_prefix("_build")
        .and_then(|suffix| suffix.chars().next())
        .is_some_and(|character| character.is_ascii_uppercase() || character.is_ascii_digit())
}

fn path_looks_like_widget_file(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    ["_screen.dart", "_page.dart", "_view.dart", "_widget.dart"]
        .iter()
        .any(|suffix| file_name.ends_with(suffix))
        || path.components().any(|component| {
            component.as_os_str().to_str().is_some_and(|segment| {
                matches!(segment, "presentation" | "screen" | "screens" | "widgets")
            })
        })
}

fn imports_flutter_ui(root: Node<'_>, source: &str) -> bool {
    let mut cursor = root.walk();
    root.named_children(&mut cursor)
        .filter(|child| child.kind().contains("import"))
        .filter_map(|child| child.utf8_text(source.as_bytes()).ok())
        .any(|text| {
            text.contains("package:flutter/")
                || text.contains("package:flutter_hooks/")
                || text.contains("package:flutter_riverpod/")
                || text.contains("package:hooks_riverpod/")
        })
}
