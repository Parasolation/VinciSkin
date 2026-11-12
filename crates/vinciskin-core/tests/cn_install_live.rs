//! The Chinese client's own install, read end to end on a real machine.
//!
//! Ignored by default: it needs a Chinese client installed, which CI has not
//! got. Run it with
//! `cargo test -p vinciskin-core --test cn_install_live -- --ignored`.
//!
//! What it covers that no fixture can: the Tencent client's layout and archive
//! format, and the champion's own skin bins. Discovery is the app's own, so
//! nothing has to be configured for it beyond the client itself being installed.
//!
//! The things it settles, in the order they would break the manager: the
//! registry key names a root, that root holds `DATA/FINAL`, the archives under it
//! are mountable rather than packed some way `ltk_wad` cannot read, and a
//! champion's skins are held as the bins a swap is written from.

use std::collections::BTreeMap;
use std::io::BufReader;

use fs_err as fs;
use ltk_wad::Wad;
use vinciskin_core::config::Config;
use vinciskin_core::game_wads::GameArchives;
use vinciskin_core::hashtables::WadPathResolver;
use vinciskin_core::launcher::tencent_install_root;

#[test]
#[ignore = "needs an installed Chinese client"]
fn the_chinese_install_lists_its_archives_and_mounts_a_champion_one() {
    let root = tencent_install_root().expect("the Chinese client is installed");

    let config = Config {
        league_path: Some(root.clone()),
        ..Config::default()
    };

    let archives = GameArchives::resolve(&config)
        .unwrap_or_else(|e| panic!("DATA/FINAL does not resolve under {}: {e}", root.display()));
    let listed = archives.list().expect("the archives list");

    println!("install root    : {}", root.display());
    println!("archives listed : {}", listed.len());

    assert!(
        listed.len() > 100,
        "expected a full install under {}, found {} archives",
        root.display(),
        listed.len()
    );

    let champion = listed
        .iter()
        .find(|summary| {
            summary.name.starts_with("Champions/") && has_no_locale_suffix(&summary.name)
        })
        .expect("the install ships champion archives");

    println!(
        "mounted archive : {} ({} bytes)",
        champion.name, champion.size_bytes
    );

    let path = archives
        .archive_path(&champion.name)
        .expect("the archive path resolves");
    let wad = Wad::mount(BufReader::new(
        fs::File::open(&path).expect("the archive opens"),
    ))
    .expect("the archive mounts");

    let chunks = wad.chunks().as_slice();
    println!("chunks read     : {}", chunks.len());

    assert!(!chunks.is_empty(), "{} carries no chunks", champion.name);
}

/// Whether `name` carries no locale segment.
///
/// `Champions/Aatrox.wad.client` is the archive a skin overlay patches;
/// `Champions/Aatrox.en_US.wad.client` only holds that champion's strings.
fn has_no_locale_suffix(name: &str) -> bool {
    let Some(file) = name.rsplit('/').next() else {
        return false;
    };
    let Some(stem) = file.strip_suffix(".wad.client") else {
        return false;
    };

    !stem.contains('.')
}

#[test]
fn a_locale_variant_is_told_apart_from_the_archive_beside_it() {
    assert!(has_no_locale_suffix("Champions/Aatrox.wad.client"));
    assert!(!has_no_locale_suffix("Champions/Aatrox.en_US.wad.client"));
    assert!(!has_no_locale_suffix("Champions/Aatrox.zh_CN.wad.client"));
    assert!(!has_no_locale_suffix("Champions/Aatrox.wad"));
}

/// What a skin swap needs before it can exist: the game's own path names, and
/// the skins they fall under.
///
/// A swap is built by naming the chunks of one champion's archive and grouping
/// them by skin, so a machine whose hashtable cache has never synced cannot
/// build one at all - every chunk keeps its hex name. That is what this settles
/// before any of it is written.
#[test]
#[ignore = "needs an installed Chinese client and a synced hashtable cache"]
fn a_champion_archive_can_be_named_and_grouped_by_skin() {
    let root = tencent_install_root().expect("the Chinese client is installed");
    let config = Config {
        league_path: Some(root),
        ..Config::default()
    };

    let resolver = WadPathResolver::discover();
    assert!(
        resolver.has_tables(),
        "no hashtable cache, so no chunk can be named"
    );

    let archives = GameArchives::resolve(&config).expect("DATA/FINAL resolves");
    let champion = "Champions/Annie.wad.client";
    let own_folder = "assets/characters/annie/";

    let mut paths: Vec<String> = Vec::new();
    let mut unnamed = 0usize;

    archives
        .for_each_chunk(
            champion,
            resolver.tables(),
            |_hash, path, _size| match path {
                Some(path) => paths.push(path.to_string()),
                None => unnamed += 1,
            },
        )
        .expect("the champion archive reads");

    assert!(
        !paths.is_empty(),
        "not one chunk of {champion} could be named"
    );
    assert_eq!(unnamed, 0, "the cache names every chunk of {champion}");

    let outside = paths
        .iter()
        .filter(|path| !path.starts_with(own_folder))
        .count();
    println!(
        "named chunks : {} ({outside} outside {own_folder})",
        paths.len()
    );

    /* Paths are lowercase, and a champion's own assets sit under
    `assets/characters/<champion>/skins/<segment>/`, `base` being the skin the
    others replace. Grouping by that segment is what a swap is built from. */
    let mut segments: BTreeMap<&str, usize> = BTreeMap::new();
    for path in &paths {
        if let Some((_, rest)) = path.split_once("/skins/") {
            *segments
                .entry(rest.split('/').next().unwrap_or(rest))
                .or_default() += 1;
        }
    }

    println!("skin segments: {}", segments.len());
    let mut ranked = segments.iter().collect::<Vec<_>>();
    ranked.sort_by(|a, b| b.1.cmp(a.1));
    for (segment, count) in &ranked {
        println!("   {segment:<12} {count}");
    }
    println!("   (of {} chunks)", paths.len());

    /* What the base skin is made of, by the folder under it. */
    let base_root = "assets/characters/annie/skins/base/";
    let mut base_dirs: BTreeMap<&str, usize> = BTreeMap::new();
    for path in paths.iter().filter(|path| path.starts_with(base_root)) {
        let rest = path.trim_start_matches(base_root);
        *base_dirs
            .entry(rest.split('/').next().unwrap_or(rest))
            .or_default() += 1;
    }
    println!("--- base folders ---");
    for (dir, count) in &base_dirs {
        println!("   {dir:<20} {count}");
    }

    /* Every file a few skins own, which is what a swap has to move. */
    for segment in ["skin01", "skin50", "skin59"] {
        let owned = paths
            .iter()
            .filter(|path| {
                path.contains(&format!("/skins/{segment}/"))
                    || path.ends_with(&format!("/skins/{segment}.bin"))
            })
            .collect::<Vec<_>>();
        println!("--- {segment}: {} chunks ---", owned.len());
        for path in owned.iter().take(12) {
            println!("   {path}");
        }
    }

    /* A skin that can be swapped has to carry its own payload - skeleton, mesh,
    textures, particles - rather than a pointer to the base skin's. Counting
    the substantial ones is what says the swap has something to move. */
    let substantial = ranked.iter().filter(|(_, count)| **count >= 100).count();
    println!("segments carrying 100+ chunks: {substantial}");

    assert!(
        segments.get("base").is_some_and(|count| *count > 0),
        "the base skin holds no assets under {own_folder}"
    );
    assert!(
        substantial >= 5,
        "expected several skins to carry a real payload, found {substantial}"
    );
}

/// The bins a skin swap is written from, and the gate that decides whether one can be.
///
/// A swap names a champion's skin bin by the game's own number and copies the two objects that
/// bin holds onto another skin, so three things have to hold on the real install or the feature
/// writes a champion nothing: the bin is named without zero padding, its objects are named the
/// way the copy looks for them, and the gate agrees with the archive about which numbers exist.
#[test]
#[ignore = "needs an installed Chinese client and a synced hashtable cache"]
fn a_champions_skins_are_held_as_bins_a_swap_can_copy() {
    use std::collections::BTreeSet;
    use std::io::Cursor;

    use ltk_hash::BinHash;
    use ltk_meta::{Bin, PropertyValueEnum};
    use ltk_wad::WadHash;
    use vinciskin_core::game_wads::WadCache;
    use vinciskin_core::hashtables::LayeredHashDb;
    use vinciskin_core::overlay::{install_holds_skin, skin_bin_path};

    let root = tencent_install_root().expect("the Chinese client is installed");
    let config = Config {
        league_path: Some(root),
        ..Config::default()
    };
    let archives = GameArchives::resolve(&config).expect("DATA/FINAL resolves");

    let champion = "Champions/Annie.wad.client";
    let character = "annie";

    /* Every chunk the archive holds, found by hash rather than by name, so nothing about the
    naming under test is assumed to read it. */
    let mut held: BTreeSet<u64> = BTreeSet::new();
    archives
        .for_each_chunk(champion, &LayeredHashDb::new(), |hash, _, _| {
            held.insert(hash);
        })
        .expect("the archive reads");
    let holds = |name: &str| held.contains(&WadHash::from(name).0);

    /* The naming the whole feature is built on: the bin of skin N is `skin<N>.bin`, and a
    zero-padded one is not a chunk the game ships. */
    let bins: Vec<u32> = (0..=511)
        .filter(|id| holds(&skin_bin_path(character, *id)))
        .collect();
    println!("skin bins: {bins:?}");

    assert!(bins.contains(&0), "the champion has no base skin bin");
    assert!(
        bins.len() > 5,
        "expected a champion with several skins, found {bins:?}"
    );
    assert!(
        !holds(&format!("data/characters/{character}/skins/skin01.bin")),
        "the install ships a zero-padded bin, so the numbering is not what this assumes"
    );

    let cache = WadCache::default();
    let read = |id: u32| {
        cache
            .read_chunk(
                &archives,
                champion,
                WadHash::from(skin_bin_path(character, id).as_str()),
            )
            .unwrap_or_else(|e| panic!("skin{id}.bin reads: {e}"))
    };

    /* The two objects every skin is loaded out of, and the link between them, which is what a
    swap renames. */
    for id in [0, 1, 22] {
        let bytes = read(id);
        let parsed = Bin::from_reader(&mut Cursor::new(bytes.as_slice()))
            .unwrap_or_else(|e| panic!("skin{id}.bin parses: {e}"));

        let properties = BinHash::from(format!("Characters/{character}/Skins/Skin{id}").as_str());
        let resources =
            BinHash::from(format!("Characters/{character}/Skins/Skin{id}/Resources").as_str());

        let properties = parsed
            .objects
            .get(&properties)
            .unwrap_or_else(|| panic!("skin{id}.bin holds no Skin{id} object"));
        let resources = parsed
            .objects
            .get(&resources)
            .unwrap_or_else(|| panic!("skin{id}.bin holds no Skin{id}/Resources object"));

        assert!(
            properties.properties.values().any(|value| matches!(
                value,
                PropertyValueEnum::ObjectLink(link) if link.value == resources.path_hash
            )),
            "Skin{id}'s properties do not link to its own resolver, so a copy would not rewrite it"
        );
    }

    /* The gate the skin pick is refused on, checked against the archive's own chunk table: the
    manager must not accept a number the install cannot serve, nor refuse one it can. */
    let absent = (0..=120).find(|id| !bins.contains(id));
    for id in [0, 1, 22] {
        assert!(
            install_holds_skin(&config, character, id).expect("the install reads"),
            "skin {id} is in the archive but the gate refused it"
        );
    }
    if let Some(absent) = absent {
        println!("absent skin number: {absent}");
        assert!(
            !install_holds_skin(&config, character, absent).expect("the install reads"),
            "skin {absent} is not in the archive but the gate accepted it"
        );
    }
    assert!(
        !install_holds_skin(&config, "notachampionatall", 0).expect("the install reads"),
        "a character with no champion archive was accepted"
    );
}

/// Whether the client's own archives carry a signature over their table of contents.
///
/// A swap that writes into an archive rewrites that table, so an archive whose header carries
/// Riot's signature can only be written back with a signature that no longer verifies - we have
/// no private key. What the game does about that decides whether a swap can be written into the
/// game at all, so this measures the install rather than assuming it.
#[test]
#[ignore = "needs an installed Chinese client"]
fn a_champion_archive_reports_whether_it_is_signed() {
    use ltk_wad::RITO_PKEY;
    use ltk_wad::rsa::RsaPublicKey;
    use ltk_wad::rsa::pkcs8::DecodePublicKey as _;

    let root = tencent_install_root().expect("the Chinese client is installed");
    let config = Config {
        league_path: Some(root),
        ..Config::default()
    };
    let archives = GameArchives::resolve(&config).expect("DATA/FINAL resolves");

    let key = RsaPublicKey::from_public_key_der(RITO_PKEY).expect("Riot's key parses");
    let mut signed = 0usize;
    let mut zeroed = 0usize;
    let mut checked = 0usize;

    for summary in archives.list().expect("the archives list") {
        if !summary.name.starts_with("Champions/") || !has_no_locale_suffix(&summary.name) {
            continue;
        }

        let path = archives
            .archive_path(&summary.name)
            .expect("the archive path resolves");
        let Ok(wad) = Wad::mount(BufReader::new(fs::File::open(&path).expect("it opens"))) else {
            continue;
        };

        let (valid, _) = wad
            .verify_signature(&key)
            .expect("the signature is checkable");
        if wad.signature().iter().all(|byte| *byte == 0) {
            zeroed += 1;
        }
        if valid {
            signed += 1;
        }
        checked += 1;

        if checked <= 3 {
            println!(
                "{}: signature {} valid, checksum {:016x}",
                summary.name,
                if valid { "IS" } else { "is NOT" },
                wad.checksum()
            );
        }
        if checked >= 20 {
            break;
        }
    }

    println!("checked {checked} archives: {signed} signed, {zeroed} with no signature");
    assert!(checked > 0, "no champion archive could be read");
}

/// Which codecs a champion archive's chunks are stored under.
///
/// A swap that writes into an archive re-emits its table of contents. Every chunk can be carried
/// over verbatim except a [`ZstdMulti`](ltk_wad::WadChunkCompression::ZstdMulti) one, whose bytes
/// only decode alongside the subchunk records naming them - so how many of those an archive holds
/// decides whether a write is a copy or a re-encode. Measured rather than assumed, because the
/// codec mix is what the client ships.
#[test]
#[ignore = "needs an installed Chinese client"]
fn a_champion_archive_reports_its_chunk_codecs() {
    use std::collections::BTreeMap;

    let root = tencent_install_root().expect("the Chinese client is installed");
    let config = Config {
        league_path: Some(root),
        ..Config::default()
    };
    let archives = GameArchives::resolve(&config).expect("DATA/FINAL resolves");

    for champion in [
        "Champions/Annie.wad.client",
        "Champions/Ahri.wad.client",
        "Champions/Aatrox.wad.client",
    ] {
        let path = archives
            .archive_path(champion)
            .expect("the archive path resolves");
        let wad = Wad::mount(BufReader::new(
            fs::File::open(&path).expect("the archive opens"),
        ))
        .expect("the archive mounts");

        let mut codecs: BTreeMap<String, usize> = BTreeMap::new();
        for chunk in wad.chunks().as_slice() {
            *codecs
                .entry(chunk.compression_type().to_string())
                .or_default() += 1;
        }

        println!(
            "{champion}: {} chunks, codecs {codecs:?}, subchunk table {}",
            wad.chunks().len(),
            if wad.subchunk_toc().is_some() {
                "present"
            } else {
                "absent"
            }
        );
    }
}
