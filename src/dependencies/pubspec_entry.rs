use noyalib::Value;

use super::DependencySection;
use crate::Location;

pub(super) fn is_simple_scalar_dependency(
    source: &str,
    section: DependencySection,
    dependency: &str,
    location: Location,
) -> bool {
    if !matches!(
        section,
        DependencySection::Dependencies | DependencySection::DevDependencies
    ) {
        return false;
    }
    let Some(index) = location.line.checked_sub(1) else {
        return false;
    };
    let lines = source.split_inclusive('\n').collect::<Vec<_>>();
    let Some(line) = lines.get(index) else {
        return false;
    };
    if line.contains('#') {
        return false;
    }
    let Ok(indent) = indentation(line) else {
        return false;
    };
    if indent == 0 || enclosing_section(&lines, index, indent) != Some(section) {
        return false;
    }
    let Some((key, value)) = line.trim().split_once(':') else {
        return false;
    };
    key.trim() == dependency && is_safe_scalar_value(value.trim())
}

fn is_safe_scalar_value(value: &str) -> bool {
    if value.is_empty()
        || value.contains('{')
        || value.contains('}')
        || value.contains('[')
        || value.contains(']')
        || value.contains("path:")
        || value.contains("git:")
        || value.contains("sdk:")
    {
        return false;
    }
    noyalib::from_str::<Value>(value)
        .is_ok_and(|value| !matches!(value, Value::Mapping(_) | Value::Sequence(_) | Value::Null))
}

fn enclosing_section(
    lines: &[&str],
    index: usize,
    target_indent: usize,
) -> Option<DependencySection> {
    for line in lines[..index].iter().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = indentation(line).ok()?;
        if indent >= target_indent {
            continue;
        }
        if indent != 0 {
            return None;
        }
        let (key, value) = trimmed.split_once(':')?;
        if !value.trim().is_empty() {
            return None;
        }
        return match key.trim() {
            "dependencies" => Some(DependencySection::Dependencies),
            "dev_dependencies" => Some(DependencySection::DevDependencies),
            _ => None,
        };
    }
    None
}

fn indentation(line: &str) -> Result<usize, ()> {
    let mut spaces = 0;
    for character in line.chars() {
        match character {
            ' ' => spaces += 1,
            '\t' => return Err(()),
            _ => return Ok(spaces),
        }
    }
    Ok(spaces)
}
