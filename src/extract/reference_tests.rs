use super::{ExtractError, extract_dart_source};

#[test]
fn raw_adjacent_string_segment_does_not_suppress_later_interpolation() -> Result<(), ExtractError> {
    let source = "\
void boot(String _ignored, String _used) {
  print(r'$_ignored' '$_used');
}
";

    let extracted = extract_dart_source("lib/interpolation.dart", source)?;
    let references = extracted
        .references
        .iter()
        .map(|reference| reference.name.as_str())
        .collect::<Vec<_>>();

    assert!(references.contains(&"_used"));
    assert!(!references.contains(&"_ignored"));

    Ok(())
}

#[test]
fn braced_interpolation_references_are_not_duplicated() -> Result<(), ExtractError> {
    let source = "\
void boot(String suffix) {
  print('${suffix.toUpperCase()} $suffix');
}
";

    let extracted = extract_dart_source("lib/interpolation.dart", source)?;
    let references = extracted
        .references
        .iter()
        .map(|reference| reference.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        references
            .iter()
            .filter(|reference| **reference == "suffix")
            .count(),
        2
    );
    assert_eq!(
        references
            .iter()
            .filter(|reference| **reference == "toUpperCase")
            .count(),
        1
    );

    Ok(())
}
