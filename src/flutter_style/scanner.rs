use std::collections::{BTreeMap, BTreeSet};

use rayon::prelude::*;
use tree_sitter::Node;

use super::types::{
    FlutterStyleFinding, FlutterStyleFindingKind, FlutterStyleReport, FlutterStyleValueKind,
    ThemeTokenEvidence,
};
use super::usage::used_extension_properties;
use super::{ParsedSourceUnit, SourceUnit, style_parse_error};
use crate::HealthError;

const NEAR_DUPLICATE_DISTANCE_SQUARED: u32 = 4;
type ExtensionFields = (BTreeMap<String, Vec<ThemeTokenEvidence>>, BTreeSet<String>);

pub(super) fn analyze(units: &[SourceUnit]) -> Result<FlutterStyleReport, HealthError> {
    let parsed_units = parse_units(units)?;
    let (extension_fields, ambiguous_extension_names) = extension_fields(&parsed_units);
    let extension_names = extension_fields.keys().cloned().collect::<BTreeSet<_>>();
    let theme_owner_names = theme_owner_names(&parsed_units);
    let mut tokens = extension_fields
        .values()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    tokens.extend(static_theme_owner_tokens(&parsed_units, &theme_owner_names));
    let used_properties = used_extension_properties(&parsed_units, &extension_names);

    for parsed_unit in &parsed_units {
        let unit = parsed_unit.unit;
        let parsed = &parsed_unit.parsed;
        walk(parsed.tree().root_node(), &mut |node| {
            collect_builtin_tokens(node, parsed.source(), &unit.path, &mut tokens);
            populate_extension_values(
                node,
                parsed.source(),
                &unit.path,
                &extension_names,
                &ambiguous_extension_names,
                &mut tokens,
            );
        });
    }
    deduplicate_tokens(&mut tokens);

    let mut findings =
        raw_value_findings(&parsed_units, &extension_names, &theme_owner_names, &tokens);
    findings.extend(near_duplicate_findings(&tokens));
    findings.extend(unused_token_findings(&tokens, &used_properties));
    sort_findings(&mut findings);
    Ok(FlutterStyleReport { findings })
}

fn extension_fields(units: &[ParsedSourceUnit<'_>]) -> ExtensionFields {
    let mut extensions = BTreeMap::<String, Vec<ThemeTokenEvidence>>::new();
    let mut ambiguous = BTreeSet::new();
    for parsed_unit in units {
        let unit = parsed_unit.unit;
        let parsed = &parsed_unit.parsed;
        walk(parsed.tree().root_node(), &mut |node| {
            if node.kind() != "class_declaration"
                || !node_text(node, parsed.source()).contains("ThemeExtension<")
            {
                return;
            }
            let Some(name) = field_text(node, "name", parsed.source()) else {
                return;
            };
            let mut fields = Vec::new();
            walk(node, &mut |candidate| {
                if candidate.kind() != "initialized_identifier"
                    || !is_direct_class_field(candidate, node)
                {
                    return;
                }
                let Some(field_name) = field_text(candidate, "name", parsed.source()) else {
                    return;
                };
                let declaration_prefix = declaration_prefix(candidate, parsed.source());
                let value_kind = if declaration_prefix.contains("TextStyle") {
                    FlutterStyleValueKind::TextStyle
                } else if declaration_prefix.contains("Color") {
                    FlutterStyleValueKind::Color
                } else {
                    return;
                };
                fields.push(ThemeTokenEvidence {
                    name: field_name,
                    path: unit.path.clone(),
                    location: candidate.start_position().into(),
                    value_kind,
                    value: None,
                    custom: true,
                    argb: None,
                    owner: Some(name.clone()),
                    declared_field: true,
                });
            });
            if extensions.contains_key(&name) {
                ambiguous.insert(name.clone());
            }
            extensions.entry(name).or_default().extend(fields);
        });
    }
    (extensions, ambiguous)
}

fn collect_builtin_tokens(
    node: Node<'_>,
    source: &str,
    path: &std::path::Path,
    tokens: &mut Vec<ThemeTokenEvidence>,
) {
    if !is_constructor_node(node) {
        return;
    }
    let Some(constructor) = constructor_type(node, source) else {
        return;
    };
    let value_kind = match constructor.as_str() {
        "ColorScheme" => FlutterStyleValueKind::Color,
        "TextTheme" => FlutterStyleValueKind::TextStyle,
        _ => return,
    };
    for (name, value_node) in named_arguments(node, source) {
        let value = node_text(value_node, source);
        if style_value_kind_node(value_node, source) != Some(value_kind) {
            continue;
        }
        let argb = (value_kind == FlutterStyleValueKind::Color)
            .then(|| parse_argb(value))
            .flatten();
        tokens.push(ThemeTokenEvidence {
            name: format!(
                "{}.{name}",
                if value_kind == FlutterStyleValueKind::Color {
                    "colorScheme"
                } else {
                    "textTheme"
                }
            ),
            path: path.to_path_buf(),
            location: value_node.start_position().into(),
            value_kind,
            value: normalized_value(value_kind, value),
            custom: false,
            argb,
            owner: None,
            declared_field: false,
        });
    }
}

fn populate_extension_values(
    node: Node<'_>,
    source: &str,
    path: &std::path::Path,
    extension_names: &BTreeSet<String>,
    ambiguous_extension_names: &BTreeSet<String>,
    tokens: &mut Vec<ThemeTokenEvidence>,
) {
    if !is_constructor_node(node) {
        return;
    }
    let Some(owner) = constructor_type(node, source) else {
        return;
    };
    if !extension_names.contains(&owner) || ambiguous_extension_names.contains(&owner) {
        return;
    }
    for (name, value_node) in named_arguments(node, source) {
        let Some(template) = tokens.iter().find(|token| {
            token.declared_field
                && token.owner.as_deref() == Some(owner.as_str())
                && token.name == name
        }) else {
            continue;
        };
        let value = node_text(value_node, source);
        if style_value_kind_node(value_node, source) != Some(template.value_kind) {
            continue;
        }
        let mut token = template.clone();
        token.path = path.to_path_buf();
        token.location = value_node.start_position().into();
        token.value = normalized_value(token.value_kind, value);
        token.argb = (token.value_kind == FlutterStyleValueKind::Color)
            .then(|| parse_argb(value))
            .flatten();
        token.declared_field = false;
        tokens.push(token);
    }
}

fn raw_value_findings(
    units: &[ParsedSourceUnit<'_>],
    extension_names: &BTreeSet<String>,
    theme_owner_names: &BTreeSet<String>,
    tokens: &[ThemeTokenEvidence],
) -> Vec<FlutterStyleFinding> {
    let mut findings = Vec::new();
    for parsed_unit in units {
        let unit = parsed_unit.unit;
        let parsed = &parsed_unit.parsed;
        walk(parsed.tree().root_node(), &mut |node| {
            if (!is_constructor_node(node) && !is_colors_constant(node, parsed.source()))
                || is_theme_definer(node, parsed.source(), extension_names, theme_owner_names)
            {
                return;
            }
            let Some(value_kind) = style_value_kind_node(node, parsed.source()) else {
                return;
            };
            let source_value = node_text(node, parsed.source());
            let argb = (value_kind == FlutterStyleValueKind::Color)
                .then(|| parse_argb(source_value))
                .flatten();
            let normalized = normalized_value(value_kind, source_value);
            let Some((nearest, distance)) =
                nearest_token(value_kind, argb, normalized.as_deref(), tokens)
            else {
                return;
            };
            findings.push(FlutterStyleFinding {
                kind: FlutterStyleFindingKind::RawFlutterStyleValue,
                path: unit.path.clone(),
                location: node.start_position().into(),
                value_kind,
                value: normalized,
                token: None,
                nearest_token: Some(nearest.clone()),
                distance,
            });
        });
    }
    findings
}

fn near_duplicate_findings(tokens: &[ThemeTokenEvidence]) -> Vec<FlutterStyleFinding> {
    let custom_colors = tokens
        .iter()
        .filter(|token| {
            token.custom
                && !token.declared_field
                && token.value_kind == FlutterStyleValueKind::Color
        })
        .collect::<Vec<_>>();
    let mut findings = Vec::new();
    for (index, left) in custom_colors.iter().enumerate() {
        for right in custom_colors.iter().skip(index + 1) {
            let (Some(left_argb), Some(right_argb)) = (left.argb, right.argb) else {
                continue;
            };
            let squared = argb_distance_squared(left_argb, right_argb);
            if squared == 0 || squared > NEAR_DUPLICATE_DISTANCE_SQUARED {
                continue;
            }
            findings.push(FlutterStyleFinding {
                kind: FlutterStyleFindingKind::NearDuplicateThemeToken,
                path: left.path.clone(),
                location: left.location,
                value_kind: FlutterStyleValueKind::Color,
                value: left.value.clone(),
                token: Some((*left).clone()),
                nearest_token: Some((*right).clone()),
                distance: Some(format_distance(squared)),
            });
        }
    }
    findings
}

fn unused_token_findings(
    tokens: &[ThemeTokenEvidence],
    used_properties: &BTreeSet<(String, String)>,
) -> Vec<FlutterStyleFinding> {
    tokens
        .iter()
        .filter(|token| {
            token.custom
                && token.declared_field
                && token.owner.as_ref().is_none_or(|owner| {
                    !used_properties.contains(&(owner.clone(), token.name.clone()))
                })
        })
        .map(|token| FlutterStyleFinding {
            kind: FlutterStyleFindingKind::UnusedThemeExtensionToken,
            path: token.path.clone(),
            location: token.location,
            value_kind: token.value_kind,
            value: token.value.clone(),
            token: Some(token.clone()),
            nearest_token: None,
            distance: None,
        })
        .collect()
}

fn nearest_token<'tokens>(
    value_kind: FlutterStyleValueKind,
    argb: Option<u32>,
    normalized: Option<&str>,
    tokens: &'tokens [ThemeTokenEvidence],
) -> Option<(&'tokens ThemeTokenEvidence, Option<String>)> {
    let candidates = tokens
        .iter()
        .filter(|token| token.value_kind == value_kind && token.value.is_some());
    if value_kind == FlutterStyleValueKind::TextStyle {
        return candidates
            .min_by_key(|token| (&token.name, &token.path, token.location.line))
            .map(|token| (token, None));
    }
    let Some(argb) = argb else {
        return candidates
            .filter(|token| token.value.as_deref() == normalized)
            .min_by_key(|token| (&token.name, &token.path, token.location.line))
            .map(|token| (token, Some("0.00".to_owned())));
    };
    candidates
        .filter_map(|token| {
            token
                .argb
                .map(|candidate| (argb_distance_squared(argb, candidate), token))
        })
        .min_by_key(|(distance, token)| {
            (
                *distance,
                &token.name,
                &token.path,
                token.location.line,
                token.location.column,
            )
        })
        .map(|(distance, token)| (token, Some(format_distance(distance))))
}

fn theme_owner_names(units: &[ParsedSourceUnit<'_>]) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for parsed_unit in units {
        let parsed = &parsed_unit.parsed;
        walk(parsed.tree().root_node(), &mut |node| {
            if node.kind() != "class_declaration" {
                return;
            }
            let Some(name) = field_text(node, "name", parsed.source()) else {
                return;
            };
            if !is_theme_owner_name(&name) || is_widget_class(node, parsed.source()) {
                return;
            }
            let mut declares_static_style_token = false;
            walk(node, &mut |candidate| {
                if !matches!(
                    candidate.kind(),
                    "initialized_identifier" | "static_final_declaration"
                ) || !is_direct_class_field(candidate, node)
                {
                    return;
                }
                declares_static_style_token |=
                    is_static_style_declaration(candidate, node, parsed.source());
            });
            if declares_static_style_token {
                names.insert(name);
            }
        });
    }
    names
}

fn static_theme_owner_tokens(
    units: &[ParsedSourceUnit<'_>],
    theme_owner_names: &BTreeSet<String>,
) -> Vec<ThemeTokenEvidence> {
    let mut tokens = Vec::new();
    for parsed_unit in units {
        let unit = parsed_unit.unit;
        let parsed = &parsed_unit.parsed;
        walk(parsed.tree().root_node(), &mut |class| {
            if class.kind() != "class_declaration" {
                return;
            }
            let Some(owner) = field_text(class, "name", parsed.source()) else {
                return;
            };
            if !theme_owner_names.contains(&owner) {
                return;
            }
            walk(class, &mut |candidate| {
                if !matches!(
                    candidate.kind(),
                    "initialized_identifier"
                        | "initialized_variable_definition"
                        | "static_final_declaration"
                ) || !is_direct_class_field(candidate, class)
                    || !is_static_style_declaration(candidate, class, parsed.source())
                {
                    return;
                }
                let Some(name) = field_text(candidate, "name", parsed.source()) else {
                    return;
                };
                let Some(value_node) = candidate.child_by_field_name("value") else {
                    return;
                };
                let Some(value_kind) = style_value_kind_node(value_node, parsed.source()) else {
                    return;
                };
                let value = node_text(value_node, parsed.source());
                tokens.push(ThemeTokenEvidence {
                    name: format!("{owner}.{name}"),
                    path: unit.path.clone(),
                    location: value_node.start_position().into(),
                    value_kind,
                    value: normalized_value(value_kind, value),
                    custom: true,
                    argb: (value_kind == FlutterStyleValueKind::Color)
                        .then(|| parse_argb(value))
                        .flatten(),
                    owner: Some(owner.clone()),
                    declared_field: false,
                });
            });
        });
    }
    tokens
}

fn is_theme_owner_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    ["theme", "tokens", "palette", "colors", "styles"]
        .iter()
        .any(|suffix| name.ends_with(suffix))
}

fn is_static_style_declaration(node: Node<'_>, class: Node<'_>, source: &str) -> bool {
    let mut declaration = Some(node);
    while let Some(candidate) = declaration {
        if candidate.id() == class.id() {
            return false;
        }
        let text = node_text(candidate, source);
        if text.contains("static") && (text.contains("Color") || text.contains("TextStyle")) {
            return true;
        }
        declaration = candidate.parent();
    }
    false
}

fn is_widget_class(node: Node<'_>, source: &str) -> bool {
    let header = node_text(node, source)
        .split_once('{')
        .map_or_else(|| node_text(node, source), |(header, _)| header);
    ["StatelessWidget", "StatefulWidget", "ConsumerWidget"]
        .iter()
        .any(|widget| header.contains(widget))
}

fn is_theme_definer(
    node: Node<'_>,
    source: &str,
    extension_names: &BTreeSet<String>,
    theme_owner_names: &BTreeSet<String>,
) -> bool {
    let mut ancestor = node.parent();
    while let Some(parent) = ancestor {
        if parent.kind() == "class_declaration"
            && field_text(parent, "name", source).is_some_and(|name| {
                extension_names.contains(&name) || theme_owner_names.contains(&name)
            })
        {
            return true;
        }
        if is_constructor_node(parent)
            && constructor_type(parent, source).is_some_and(|name| {
                matches!(name.as_str(), "ThemeData" | "ColorScheme" | "TextTheme")
                    || extension_names.contains(&name)
            })
        {
            return true;
        }
        ancestor = parent.parent();
    }
    false
}

fn is_direct_class_field(node: Node<'_>, class: Node<'_>) -> bool {
    let mut ancestor = node.parent();
    while let Some(parent) = ancestor {
        if parent.id() == class.id() {
            return true;
        }
        if parent.kind().contains("function")
            || parent.kind().contains("method")
            || parent.kind().contains("constructor")
            || parent.kind().contains("parameter")
        {
            return false;
        }
        ancestor = parent.parent();
    }
    false
}

fn declaration_prefix<'source>(node: Node<'_>, source: &'source str) -> &'source str {
    let declaration = node
        .parent()
        .and_then(|parent| parent.parent())
        .unwrap_or(node);
    source
        .get(declaration.start_byte()..node.start_byte())
        .unwrap_or_default()
}

fn named_arguments<'tree>(node: Node<'tree>, source: &str) -> Vec<(String, Node<'tree>)> {
    let Some(arguments) = node.child_by_field_name("arguments") else {
        return Vec::new();
    };
    let mut result = Vec::new();
    let mut cursor = arguments.walk();
    for argument in arguments.named_children(&mut cursor) {
        if argument.kind() != "named_argument" {
            continue;
        }
        let mut children_cursor = argument.walk();
        let children = argument
            .named_children(&mut children_cursor)
            .collect::<Vec<_>>();
        let Some(label) = children.iter().find(|child| child.kind() == "label") else {
            continue;
        };
        let Some(value) = children.iter().find(|child| child.kind() != "label") else {
            continue;
        };
        result.push((
            node_text(*label, source).trim_end_matches(':').to_owned(),
            *value,
        ));
    }
    result
}

fn constructor_kind(node: Node<'_>, source: &str) -> Option<FlutterStyleValueKind> {
    if !is_constructor_node(node) {
        return None;
    }
    match constructor_type(node, source)?.as_str() {
        "Color" => Some(FlutterStyleValueKind::Color),
        "TextStyle" => Some(FlutterStyleValueKind::TextStyle),
        _ => None,
    }
}

fn style_value_kind_node(node: Node<'_>, source: &str) -> Option<FlutterStyleValueKind> {
    constructor_kind(node, source)
        .or_else(|| is_colors_constant(node, source).then_some(FlutterStyleValueKind::Color))
}

fn is_colors_constant(node: Node<'_>, source: &str) -> bool {
    node.kind() == "member_expression"
        && field_text(node, "object", source)
            .is_some_and(|object| object.rsplit('.').next().map(str::trim) == Some("Colors"))
        && field_text(node, "property", source).is_some()
}

fn constructor_type(node: Node<'_>, source: &str) -> Option<String> {
    let value = match node.kind() {
        "call_expression" => field_text(node, "function", source)?,
        "const_object_expression" | "constructor_invocation" => field_text(node, "type", source)?,
        _ => return None,
    };
    value
        .split('.')
        .map(str::trim)
        .find(|name| {
            name.chars()
                .next()
                .is_some_and(|character| character.is_ascii_uppercase())
        })
        .map(str::to_owned)
}

fn is_constructor_node(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "call_expression" | "const_object_expression" | "constructor_invocation"
    )
}

fn normalized_value(kind: FlutterStyleValueKind, source: &str) -> Option<String> {
    match kind {
        FlutterStyleValueKind::Color => parse_argb(source)
            .map(|argb| format!("0x{argb:08X}"))
            .or_else(|| is_colors_source(source).then(|| source.trim().to_owned())),
        FlutterStyleValueKind::TextStyle => Some(source.split_whitespace().collect()),
    }
}

fn parse_argb(source: &str) -> Option<u32> {
    if let Some(arguments) = constructor_arguments(source, "fromARGB") {
        let channels = positional_numbers(arguments)?;
        if channels.len() != 4 {
            return None;
        }
        return Some(
            u32::from(to_byte(channels[0])?) << 24
                | u32::from(to_byte(channels[1])?) << 16
                | u32::from(to_byte(channels[2])?) << 8
                | u32::from(to_byte(channels[3])?),
        );
    }
    if let Some(arguments) = constructor_arguments(source, "fromRGBO") {
        let channels = positional_numbers(arguments)?;
        if channels.len() != 4 {
            return None;
        }
        return Some(
            u32::from(to_opacity_byte(channels[3])?) << 24
                | u32::from(to_byte(channels[0])?) << 16
                | u32::from(to_byte(channels[1])?) << 8
                | u32::from(to_byte(channels[2])?),
        );
    }
    if let Some(arguments) = constructor_arguments(source, "from") {
        let values = arguments
            .split(',')
            .filter_map(|argument| argument.split_once(':'))
            .map(|(name, value)| (name.trim(), parse_number(value.trim())))
            .collect::<BTreeMap<_, _>>();
        let alpha = to_opacity_byte(values.get("alpha").copied().flatten()?)?;
        let red = to_opacity_byte(values.get("red").copied().flatten()?)?;
        let green = to_opacity_byte(values.get("green").copied().flatten()?)?;
        let blue = to_opacity_byte(values.get("blue").copied().flatten()?)?;
        return Some(
            u32::from(alpha) << 24 | u32::from(red) << 16 | u32::from(green) << 8 | u32::from(blue),
        );
    }
    let arguments = color_constructor_arguments(source, None)?;
    let value = parse_number(arguments.trim())?;
    (value.denominator == 1)
        .then(|| u32::try_from(value.numerator.rem_euclid(1_i128 << 32)).ok())
        .flatten()
}

fn is_colors_source(source: &str) -> bool {
    source
        .trim()
        .rsplit_once('.')
        .is_some_and(|(object, _)| object.rsplit('.').next() == Some("Colors"))
}

fn constructor_arguments<'source>(source: &'source str, constructor: &str) -> Option<&'source str> {
    color_constructor_arguments(source, Some(constructor))
}

fn color_constructor_arguments<'source>(
    source: &'source str,
    constructor: Option<&str>,
) -> Option<&'source str> {
    let color = source.find("Color")?;
    let mut remainder = source[color + "Color".len()..].trim_start();
    match constructor {
        Some(constructor) => {
            remainder = remainder.strip_prefix('.')?.trim_start();
            remainder = remainder.strip_prefix(constructor)?.trim_start();
        }
        None => {
            if let Some(after_new) = remainder.strip_prefix(".new") {
                remainder = after_new.trim_start();
            } else if remainder.starts_with('.') {
                return None;
            }
        }
    }
    remainder
        .strip_prefix('(')?
        .rsplit_once(')')
        .map(|(arguments, _)| arguments)
}

#[derive(Clone, Copy)]
struct Rational {
    numerator: i128,
    denominator: i128,
}

fn positional_numbers(arguments: &str) -> Option<Vec<Rational>> {
    arguments.split(',').map(parse_number).collect()
}

fn parse_number(source: &str) -> Option<Rational> {
    if let Some((numerator, denominator)) = source.split_once('/') {
        let numerator = parse_number(numerator)?;
        let denominator = parse_number(denominator)?;
        if denominator.numerator == 0 {
            return None;
        }
        let divided = Rational {
            numerator: numerator.numerator.checked_mul(denominator.denominator)?,
            denominator: numerator.denominator.checked_mul(denominator.numerator)?,
        };
        return if divided.denominator < 0 {
            Some(Rational {
                numerator: divided.numerator.checked_neg()?,
                denominator: divided.denominator.checked_neg()?,
            })
        } else {
            Some(divided)
        };
    }
    parse_numeric_literal(source)
}

fn parse_numeric_literal(source: &str) -> Option<Rational> {
    let normalized = source.trim().replace('_', "");
    let (negative, normalized) = if let Some(unsigned) = normalized.strip_prefix('-') {
        (true, unsigned)
    } else {
        (false, normalized.strip_prefix('+').unwrap_or(&normalized))
    };
    if normalized.is_empty() {
        return None;
    }
    if let Some(hex) = normalized
        .strip_prefix("0x")
        .or_else(|| normalized.strip_prefix("0X"))
    {
        let numerator = i128::from_str_radix(hex, 16).ok()?;
        return Some(Rational {
            numerator: if negative {
                numerator.checked_neg()?
            } else {
                numerator
            },
            denominator: 1,
        });
    }

    let (mantissa, exponent) = if let Some(position) = normalized.find(['e', 'E']) {
        (
            &normalized[..position],
            normalized[position + 1..].parse::<i32>().ok()?,
        )
    } else {
        (normalized, 0)
    };
    let (whole, fraction) = mantissa
        .split_once('.')
        .map_or((mantissa, ""), |parts| parts);
    if fraction.contains('.') || (whole.is_empty() && fraction.is_empty()) {
        return None;
    }
    let digits = format!("{whole}{fraction}");
    let mut numerator = digits.parse::<i128>().ok()?;
    if negative {
        numerator = numerator.checked_neg()?;
    }
    let mut denominator = 10_i128.checked_pow(u32::try_from(fraction.len()).ok()?)?;
    if exponent >= 0 {
        numerator = numerator.checked_mul(10_i128.checked_pow(exponent.unsigned_abs())?)?;
    } else {
        denominator = denominator.checked_mul(10_i128.checked_pow(exponent.unsigned_abs())?)?;
    }
    Some(Rational {
        numerator,
        denominator,
    })
}

fn to_byte(value: Rational) -> Option<u8> {
    (value.denominator != 0 && value.numerator % value.denominator == 0)
        .then(|| {
            u8::try_from((value.numerator / value.denominator).rem_euclid(i128::from(u8::MAX) + 1))
                .ok()
        })
        .flatten()
}

fn to_opacity_byte(value: Rational) -> Option<u8> {
    if value.denominator <= 0 {
        return None;
    }
    if value.numerator <= 0 {
        return Some(0);
    }
    if value.numerator >= value.denominator {
        return Some(u8::MAX);
    }
    let scaled = value.numerator.checked_mul(255)?;
    let rounded = scaled.checked_add(value.denominator / 2)? / value.denominator;
    u8::try_from(rounded).ok()
}

fn argb_distance_squared(left: u32, right: u32) -> u32 {
    [24, 16, 8, 0]
        .into_iter()
        .map(|shift| {
            let left = i64::from((left >> shift) & 0xff);
            let right = i64::from((right >> shift) & 0xff);
            u32::try_from((left - right).pow(2)).unwrap_or(u32::MAX)
        })
        .sum()
}

fn format_distance(squared: u32) -> String {
    format!("{:.2}", f64::from(squared).sqrt())
}

fn deduplicate_tokens(tokens: &mut Vec<ThemeTokenEvidence>) {
    tokens.sort_by(|left, right| {
        (
            &left.name,
            &left.path,
            left.location.line,
            left.location.column,
        )
            .cmp(&(
                &right.name,
                &right.path,
                right.location.line,
                right.location.column,
            ))
    });
    tokens.dedup_by(|left, right| {
        left.name == right.name
            && left.path == right.path
            && left.location == right.location
            && left.value_kind == right.value_kind
    });
}

fn sort_findings(findings: &mut [FlutterStyleFinding]) {
    findings.sort_by(|left, right| {
        (
            left.kind,
            &left.path,
            left.location.line,
            left.location.column,
            &left.value,
        )
            .cmp(&(
                right.kind,
                &right.path,
                right.location.line,
                right.location.column,
                &right.value,
            ))
    });
}

fn parse_units(units: &[SourceUnit]) -> Result<Vec<ParsedSourceUnit<'_>>, HealthError> {
    units
        .par_iter()
        .map(|unit| {
            crate::dart_parser::parse_dart_source_lossy(&unit.path, &unit.source)
                .map(|parsed| ParsedSourceUnit { unit, parsed })
                .map_err(style_parse_error)
        })
        .collect::<Vec<_>>()
        .into_iter()
        .collect()
}

fn walk(node: Node<'_>, visit: &mut impl FnMut(Node<'_>)) {
    visit(node);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk(child, visit);
    }
}

fn node_text<'source>(node: Node<'_>, source: &'source str) -> &'source str {
    source
        .get(node.start_byte()..node.end_byte())
        .unwrap_or_default()
}

fn field_text(node: Node<'_>, field: &str, source: &str) -> Option<String> {
    node.child_by_field_name(field)
        .map(|child| node_text(child, source).to_owned())
}
