use std::fs;

use crate::dart_parser::track_parse_count;

use super::loader::{Outline, OutlineLoader};

#[test]
fn loader_parses_only_files_that_declare_the_wanted_type_or_forward_others()
-> Result<(), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let declares = folder.path().join("declares.dart");
    let forwards = folder.path().join("forwards.dart");
    let other = folder.path().join("other.dart");
    fs::write(
        &declares,
        "abstract class Wanted<T> {\n  void hook() {}\n}\n",
    )?;
    fs::write(
        &forwards,
        "library forwards;\nexport 'declares.dart' show Wanted;\npart 'extra.dart';\n",
    )?;
    fs::write(&other, "class Unrelated {}\nmixin Helper {}\n")?;
    let other_parses = track_parse_count(&other);
    let mut loader = OutlineLoader::default();

    let Outline::Read(declaring_file) = loader.load(&declares, "Wanted") else {
        return Err("declaring file not read".into());
    };
    let Outline::Read(forwarding_file) = loader.load(&forwards, "Wanted") else {
        return Err("forwarding file not read".into());
    };

    assert_eq!(declaring_file.declarations[0].name, "Wanted");
    assert_eq!(declaring_file.members[0].name, "hook");
    assert_eq!(forwarding_file.exports[0].uri, "declares.dart");
    assert_eq!(forwarding_file.parts[0].uri, "extra.dart");
    assert!(matches!(loader.load(&other, "Wanted"), Outline::Skipped));
    assert_eq!(other_parses.count(), 0);
    assert!(matches!(loader.load(&other, "Helper"), Outline::Read(_)));
    assert_eq!(other_parses.count(), 1);
    assert!(matches!(loader.load(&other, "Unrelated"), Outline::Read(_)));
    assert_eq!(other_parses.count(), 1);

    Ok(())
}

#[test]
fn loader_reports_files_it_cannot_read_or_parse() -> Result<(), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let broken = folder.path().join("broken.dart");
    fs::write(&broken, "class Wanted {{{ export 'a.dart';\n")?;
    let mut loader = OutlineLoader::default();

    assert!(matches!(
        loader.load(&folder.path().join("missing.dart"), "Wanted"),
        Outline::Missing
    ));
    assert!(matches!(loader.load(&broken, "Wanted"), Outline::Unparsed));
    assert!(matches!(loader.load(&broken, "Wanted"), Outline::Unparsed));

    Ok(())
}

#[test]
fn loader_finds_declarations_that_modifiers_and_line_breaks_separate()
-> Result<(), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("types.dart");
    fs::write(
        &path,
        "abstract interface class Face {}\nmixin class Both {}\ntypedef Alias = Face;\nclass\n    Spread {}\n",
    )?;

    for name in ["Face", "Both", "Alias", "Spread"] {
        let mut loader = OutlineLoader::default();
        assert!(
            matches!(loader.load(&path, name), Outline::Read(_)),
            "{name}"
        );
    }
    assert!(matches!(
        OutlineLoader::default().load(&path, "Missing"),
        Outline::Skipped
    ));

    Ok(())
}

#[test]
fn prefetch_loads_many_files_at_once_and_load_reuses_them() -> Result<(), Box<dyn std::error::Error>>
{
    let folder = tempfile::tempdir()?;
    let forwarding = folder.path().join("forwarding.dart");
    let declaring = folder.path().join("declaring.dart");
    let other = folder.path().join("other.dart");
    let missing = folder.path().join("missing.dart");
    fs::write(&forwarding, "export 'declaring.dart';\n")?;
    fs::write(&declaring, "class Wanted {}\n")?;
    fs::write(&other, "class Unrelated {}\n")?;
    let forwarding_parses = track_parse_count(&forwarding);
    let declaring_parses = track_parse_count(&declaring);
    let other_parses = track_parse_count(&other);
    let parses = || {
        [
            forwarding_parses.count(),
            declaring_parses.count(),
            other_parses.count(),
        ]
    };
    let mut loader = OutlineLoader::default();

    loader.prefetch(
        &[
            forwarding.clone(),
            declaring.clone(),
            other.clone(),
            missing.clone(),
        ],
        "Wanted",
    );

    assert_eq!(parses(), [1, 1, 0]);
    assert!(matches!(
        loader.load(&forwarding, "Wanted"),
        Outline::Read(_)
    ));
    assert!(matches!(
        loader.load(&declaring, "Wanted"),
        Outline::Read(_)
    ));
    assert!(matches!(loader.load(&other, "Wanted"), Outline::Skipped));
    assert!(matches!(loader.load(&missing, "Wanted"), Outline::Missing));
    assert_eq!(parses(), [1, 1, 0]);

    Ok(())
}
