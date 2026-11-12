//! The whole real hero skin corpus, parsed end to end.

use std::fs;
use std::path::{Path, PathBuf};

use vinciskin_cn::catalog::gtimg::parse_hero_skins;
use vinciskin_cn::version::PatchVersion;

/// The hero count the corpus holds, one file each.
const HERO_COUNT: usize = 173;

/// The skin count across the corpus.
const SKIN_COUNT: usize = 9158;

/// The version every file in the corpus reports.
const CORPUS_VERSION: &str = "16.19";

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("heroes")
}

fn js_files(directory: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = fs::read_dir(directory)
        .expect("the corpus directory is present")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "js"))
        .collect();

    paths.sort();
    paths
}

fn hero_id_of(path: &Path) -> Option<u32> {
    path.file_stem()?.to_str()?.parse().ok()
}

#[test]
fn every_real_hero_skin_catalog_parses_and_reports_one_version() {
    let directory = corpus_dir();
    let paths = js_files(&directory);

    assert_eq!(
        paths.len(),
        HERO_COUNT,
        "the corpus should hold every hero once"
    );

    let expected = PatchVersion::parse(CORPUS_VERSION).expect("the corpus version parses");

    let mut failures = Vec::new();
    let mut skins_seen = 0;

    for path in &paths {
        let Some(hero_id) = hero_id_of(path) else {
            failures.push(format!("{path:?}: the file name carries no hero id"));
            continue;
        };

        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();

        let body = match fs::read_to_string(path) {
            Ok(body) => body,
            Err(error) => {
                failures.push(format!("{name}: unreadable, {error}"));
                continue;
            }
        };

        let catalog = match parse_hero_skins(&body) {
            Ok(catalog) => catalog,
            Err(error) => {
                failures.push(format!("{name}: {error}"));
                continue;
            }
        };

        if catalog.version != expected {
            failures.push(format!(
                "{name}: reports {} not {CORPUS_VERSION}",
                catalog.version
            ));
        }

        for skin in &catalog.skins {
            if skin.skin_id == 0 {
                failures.push(format!("{name}: a skin carries no id"));
            }

            if skin.hero_id == 0 {
                failures.push(format!("{name}: skin {} carries no hero", skin.skin_id));
            }

            if skin.hero_id != hero_id {
                failures.push(format!(
                    "{name}: skin {} claims hero {}",
                    skin.skin_id, skin.hero_id
                ));
            }
        }

        skins_seen += catalog.skins.len();
    }

    assert!(
        failures.is_empty(),
        "{} of {} catalogs failed:\n{}",
        failures.len(),
        paths.len(),
        failures.join("\n")
    );

    assert_eq!(skins_seen, SKIN_COUNT, "the corpus should hold every skin");
}
