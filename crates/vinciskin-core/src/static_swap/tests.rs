//! Unit tests for the swap: the write into an archive, and the record around it.

use std::io::Cursor;
use std::io::Write as _;
use std::path::Path;

use fs_err as fs;
use ltk_hash::BinHash;
use ltk_meta::property::values;
use ltk_meta::{Bin, BinObject};
use ltk_wad::{Wad, WadBuilder, WadChunkBuilder, WadChunkCompression, WadHash};
use sha2::{Digest as _, Sha256};

use crate::events::{BackendEvent, CnPackStage, EventSink, NullEventSink};

use super::*;

/// `Characters/<character>/Skins/Skin<id><suffix>`.
fn object(character: &str, id: u32, suffix: &str) -> BinHash {
    BinHash::from(format!("Characters/{character}/Skins/Skin{id}{suffix}").as_str())
}

/// A `skin<id>.bin` for `character`, its two objects under skin `id`'s names.
///
/// `padding` is how much the properties repeat, which is what decides how much room the client
/// stores the chunk in. The game's own base skin is the smallest of a champion's skins, and the
/// skin a swap copies onto it is a later one, so a fixture that mirrors that is a fixture the
/// writer can satisfy.
fn skin_bin(character: &str, id: u32, padding: usize) -> Vec<u8> {
    let properties = BinObject::builder(object(character, id, ""), BinHash::from("SkinData"))
        .property(
            BinHash::from("mResourceResolver"),
            values::ObjectLink::new(object(character, id, "/Resources")),
        )
        .property(
            BinHash::from("mDescription"),
            values::String::new("a skin's own properties, repeated".repeat(padding)),
        )
        .build();
    let resources =
        BinObject::builder(object(character, id, "/Resources"), BinHash::from("Res")).build();

    let mut bin = Bin::default();
    bin.objects.insert(properties.path_hash, properties);
    bin.objects.insert(resources.path_hash, resources);

    let mut bytes = Cursor::new(Vec::new());
    bin.to_writer(&mut bytes).unwrap();
    bytes.into_inner()
}

/// The client's own base skin, which is the room a swap has to fit in.
fn base_skin(character: &str) -> Vec<u8> {
    skin_bin(character, 0, 256)
}

/// A skin a swap copies onto the base, which is smaller than the base's own room.
fn source_skin(character: &str, id: u32) -> Vec<u8> {
    skin_bin(character, id, 4)
}

/// An archive at `path` holding each `(chunk path, bytes)`.
///
/// Stored uncompressed, because the client's own compressor beats anything this crate can
/// produce on a chunk this size - which is the whole reason a write is bounded by the room it
/// finds. A fixture that let the builder compress would be a fixture no write could satisfy, so
/// the room here is the bytes themselves and the tests are about the writer rather than the
/// compressors.
fn build_archive(path: &Path, chunks: &[(String, Vec<u8>)]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut builder = WadBuilder::default();
    for (name, _) in chunks {
        builder = builder.with_chunk(
            WadChunkBuilder::default()
                .with_path(name.as_str())
                .with_force_compression(WadChunkCompression::None),
        );
    }

    let mut file = fs::File::create(path).unwrap();
    builder
        .build_to_writer(&mut file, |hash, cursor| {
            let (_, bytes) = chunks
                .iter()
                .find(|(name, _)| WadHash::from(name.as_str()) == hash)
                .expect("the chunk was submitted");
            cursor.write_all(bytes)?;
            Ok(())
        })
        .unwrap();
}

/// A champion archive holding a client's own `skin0.bin` and two skins to swap in.
fn champion_archive(character: &str, path: &Path) {
    build_archive(
        path,
        &[
            (skin_bin_path(character, 0), base_skin(character)),
            (skin_bin_path(character, 21), source_skin(character, 21)),
            (skin_bin_path(character, 22), source_skin(character, 22)),
            (
                "assets/characters/annie/skins/base/annie.skn".to_owned(),
                vec![7u8; 64],
            ),
        ],
    );
}

/// A config pointing at a game directory rooted at `game`.
fn config(game: &Path) -> Config {
    Config {
        league_path: Some(game.to_path_buf()),
        ..Config::default()
    }
}

/// The chunk `name` of the archive at `path`, decompressed.
fn chunk(path: &Path, name: &str) -> Vec<u8> {
    let mut wad = Wad::mount(std::io::BufReader::new(fs::File::open(path).unwrap())).unwrap();
    let chunk = *wad
        .chunks()
        .get(WadHash::from(name))
        .expect("the chunk is in the archive");
    wad.load_chunk_decompressed(&chunk).unwrap().into_vec()
}

/// The objects a `skin0.bin` reads back as, which is what says a swap landed.
fn object_names(bytes: &[u8]) -> Vec<BinHash> {
    Bin::from_reader(&mut Cursor::new(bytes))
        .unwrap()
        .objects
        .keys()
        .copied()
        .collect()
}

#[test]
fn a_written_chunk_reads_back_and_nothing_else_moves() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("Annie.wad.client");
    champion_archive("annie", &archive);

    let before = fs::read(&archive).unwrap();
    let wad = Wad::mount(std::io::BufReader::new(fs::File::open(&archive).unwrap())).unwrap();
    let target = WadHash::from(skin_bin_path("annie", 0).as_str());
    let index = wad
        .chunks()
        .as_slice()
        .iter()
        .position(|chunk| chunk.path_hash == target)
        .expect("the chunk is in the archive");
    let entry = wad.chunks().as_slice()[index];

    let replacement = source_skin("annie", 22);
    let written = archive::write_chunk(&archive, target, &replacement).unwrap();

    /* The replacement fits the room the base skin had, so the file keeps its length and only the
    entry and the body moved. */
    assert_eq!(written.room, entry.compressed_size());
    assert!(written.used <= written.room);
    assert!(
        !written.appended,
        "the fixture leaves room for its own stand-in"
    );
    let after = fs::read(&archive).unwrap();
    assert_eq!(after.len(), before.len());

    assert_eq!(chunk(&archive, &skin_bin_path("annie", 0)), replacement);
    assert_eq!(
        object_names(&replacement),
        vec![object("annie", 22, ""), object("annie", 22, "/Resources")],
        "the fixture bytes are skin 22's own, which the swap renames"
    );

    /* Every other chunk is where it was, byte for byte. */
    assert_eq!(
        chunk(&archive, &skin_bin_path("annie", 22)),
        source_skin("annie", 22)
    );
    assert_eq!(
        chunk(&archive, "assets/characters/annie/skins/base/annie.skn"),
        vec![7u8; 64]
    );

    let entry_offset = archive::HEADER_LEN as usize + index * archive::ENTRY_LEN;
    let body = entry.data_offset()..entry.data_offset() + entry.compressed_size();
    let changed: Vec<usize> = (0..before.len())
        .filter(|at| before[*at] != after[*at])
        .collect();
    assert!(
        changed.iter().all(|at| {
            let site = *at;
            (entry_offset..entry_offset + archive::ENTRY_LEN).contains(&site)
                || body.contains(&site)
        }),
        "bytes moved outside the entry and the chunk body: {changed:?}"
    );
}

/// The ordinary case on a real archive: the replacement is a bigger payload than the chunk it
/// goes over, and there is nowhere for it inside the room the chunk had.
#[test]
fn a_replacement_too_big_for_its_room_is_appended() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("Annie.wad.client");
    let name = "data/characters/annie/skins/skin0.bin".to_owned();
    build_archive(
        &archive,
        &[
            (name.clone(), vec![1u8; 4]),
            ("other.bin".to_owned(), vec![2u8; 64]),
        ],
    );
    let before = fs::read(&archive).unwrap();
    /* The entry that names the chunk is the one thing before the appended body that must change:
    it has to point at where the body went. */
    let target = WadHash::from(name.as_str());
    let index = {
        let wad = Wad::mount(std::io::BufReader::new(fs::File::open(&archive).unwrap())).unwrap();
        wad.chunks()
            .as_slice()
            .iter()
            .position(|chunk| chunk.path_hash == target)
            .expect("the chunk is in the archive")
    };
    let entry = archive::HEADER_LEN as usize + index * archive::ENTRY_LEN;

    /* Digest bytes, which do not compress, so nothing can make this fit four bytes. */
    let payload: Vec<u8> = (0..128u32)
        .flat_map(|at| Sha256::digest(at.to_le_bytes()).to_vec())
        .collect();
    let written = archive::write_chunk(&archive, target, &payload).unwrap();

    assert!(written.appended, "there is no room for this");
    assert_eq!(written.used, payload.len());
    let after = fs::read(&archive).unwrap();
    assert_eq!(after.len(), before.len() + payload.len());

    let moved: Vec<usize> = (0..before.len())
        .filter(|at| before[*at] != after[*at])
        .collect();
    assert!(
        moved
            .iter()
            .all(|at| (entry..entry + archive::ENTRY_LEN).contains(at)),
        "something before the appended body moved outside its own entry: {moved:?}"
    );

    assert_eq!(chunk(&archive, &name), payload);
    assert_eq!(chunk(&archive, "other.bin"), vec![2u8; 64]);
}

#[test]
fn a_file_that_is_not_an_archive_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Annie.wad.client");
    fs::write(&path, b"not an archive").unwrap();

    let refused = archive::write_chunk(&path, WadHash::from("x"), &[1, 2, 3]);
    assert!(matches!(refused, Err(AppError::ValidationFailed(_))));
}

#[test]
fn a_skin_the_install_does_not_hold_is_refused() {
    let game = tempfile::tempdir().unwrap();
    let storage = tempfile::tempdir().unwrap();
    let archive = game
        .path()
        .join("DATA")
        .join("FINAL")
        .join("Champions")
        .join("Annie.wad.client");
    champion_archive("annie", &archive);
    let config = config(game.path());

    assert!(matches!(
        overlay_apply(&config, storage.path(), "annie", 1031, 1),
        Err(AppError::ValidationFailed(_))
    ));
    assert!(swaps(&config, storage.path()).unwrap().is_empty());
}

#[test]
fn a_hero_that_is_not_the_skins_is_refused() {
    let game = tempfile::tempdir().unwrap();
    let storage = tempfile::tempdir().unwrap();
    let archive = game
        .path()
        .join("DATA")
        .join("FINAL")
        .join("Champions")
        .join("Annie.wad.client");
    champion_archive("annie", &archive);
    let config = config(game.path());

    assert!(matches!(
        overlay_apply(&config, storage.path(), "annie", 999, 1),
        Err(AppError::ValidationFailed(_))
    ));
}

/// The base skin's own id is the champion's own skin, which is a restore rather than a write.
#[test]
fn a_pick_back_on_the_base_skin_restores() {
    let game = tempfile::tempdir().unwrap();
    let storage = tempfile::tempdir().unwrap();
    let archive = game
        .path()
        .join("DATA")
        .join("FINAL")
        .join("Champions")
        .join("Annie.wad.client");
    champion_archive("annie", &archive);
    let client_bytes = fs::read(&archive).unwrap();
    let config = config(game.path());

    overlay_apply(&config, storage.path(), "annie", 1022, 1).unwrap();
    let restored = overlay_apply(&config, storage.path(), "annie", 1000, 1).unwrap();

    assert_eq!(restored.state, SwapState::Untouched);
    assert_eq!(fs::read(&archive).unwrap(), client_bytes);
}

/// Whether a real skin's stand-in fits the room a real base skin was stored in.
///
/// The writer bounds itself by the room the chunk already has, and the client stores a chunk as
/// small as its own compressor can make it. Whether that leaves room for a stand-in is the one
/// thing about this route that no fixture can answer, so it is measured on the install.
#[test]
#[ignore = "needs an installed Chinese client"]
fn a_real_stand_in_reports_whether_it_fits_its_room() {
    let root = crate::launcher::tencent_install_root().expect("the Chinese client is installed");
    let game_dir = GameDir::resolve(&config(&root)).expect("the install resolves");
    let (_, archive) = super::champion_archive(&game_dir, "annie").expect("annie has an archive");

    let mut wad = Wad::mount(std::io::BufReader::new(fs::File::open(&archive).unwrap())).unwrap();
    let target = WadHash::from(skin_bin_path("annie", 0).as_str());
    let chunk = *wad.chunks().get(target).expect("the base skin is there");
    let (room, uncompressed) = (chunk.compressed_size(), chunk.uncompressed_size());
    let codec = chunk.compression_type();
    let own = wad.load_chunk_decompressed(&chunk).unwrap().into_vec();
    drop(wad);

    /* The question the room raises is whether this crate can store the client's own bytes as
    small as the client did. Compressing what the client stored settles it without a fixture. */
    let mine = archive::LEVELS
        .iter()
        .filter_map(|level| zstd::bulk::compress(&own, *level).ok())
        .map(|compressed| compressed.len())
        .min();

    println!("skin0.bin: {uncompressed} bytes, codec {codec}, stored in {room}");
    println!("  this crate's best for the same bytes: {mine:?}");

    let mut fits = 0;
    for skin in [1u32, 22, 50, 59] {
        let Ok(bytes) = super::stand_in(&archive, "annie", skin) else {
            println!("skin{skin}: no stand-in to build");
            continue;
        };
        let best = archive::LEVELS
            .iter()
            .filter_map(|level| zstd::bulk::compress(&bytes, *level).ok())
            .map(|compressed| compressed.len())
            .min();

        let ok = best.is_some_and(|best| best <= room);
        if ok {
            fits += 1;
        }
        println!(
            "skin{skin}: stand-in {} bytes, compresses to {best:?}, room {room}, fits {ok}",
            bytes.len()
        );
    }

    println!("{fits} of 4 stand-ins fit");
    assert!(room > 0, "the base skin holds no bytes");
}

/// A write into a copy of a real champion archive, with every other chunk read back after it.
///
/// The fixtures prove the writer moves what it says it moves; this proves it on the file the
/// game actually ships, where two thirds of the chunks are `ZstdMulti` and only the client's own
/// layout is in play. It works on a copy, so the install is untouched either way.
#[test]
#[ignore = "needs an installed Chinese client"]
fn a_real_archive_survives_a_swap_with_every_other_chunk_intact() {
    let root = crate::launcher::tencent_install_root().expect("the Chinese client is installed");
    let game_dir = GameDir::resolve(&config(&root)).expect("the install resolves");
    let (_, source) = super::champion_archive(&game_dir, "annie").expect("annie has an archive");

    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("Annie.wad.client");
    fs::copy(&source, &archive).unwrap();
    let before = fs::read(&archive).unwrap();

    let target = WadHash::from(skin_bin_path("annie", 0).as_str());
    let stand_in = super::stand_in(&source, "annie", 22).expect("skin 22 builds a stand-in");
    let written = archive::write_chunk(&archive, target, &stand_in).unwrap();
    println!(
        "wrote {} bytes over a chunk that had {}: appended {}",
        written.used, written.room, written.appended
    );

    /* The whole archive still mounts, keeps its chunk count, and every chunk it holds still
    decodes - which is what says the other 4082 were left where they were. */
    let mut wad = Wad::mount(std::io::BufReader::new(fs::File::open(&archive).unwrap())).unwrap();
    let chunks: Vec<_> = wad.chunks().as_slice().to_vec();
    assert_eq!(wad.chunks().len(), 4083, "the chunk count is the client's");

    let mut read = 0usize;
    for chunk in &chunks {
        if let Err(e) = wad.load_chunk_decompressed(chunk) {
            panic!("chunk {:016x} no longer decodes: {e}", chunk.path_hash.0);
        }
        read += 1;
    }
    println!("read back {read} chunks");

    let patched = *wad
        .chunks()
        .get(target)
        .expect("the base skin is still there");
    let bytes = wad.load_chunk_decompressed(&patched).unwrap().into_vec();
    assert_eq!(bytes, stand_in, "the base skin holds the stand-in");
    assert_eq!(
        object_names(&bytes),
        vec![object("annie", 0, ""), object("annie", 0, "/Resources")]
    );
    drop(wad);

    /* The file grew by exactly the body, and nothing the client wrote moved except the one entry
    that had to point at it. */
    let after = fs::read(&archive).unwrap();
    assert_eq!(after.len(), before.len() + written.used);
    let index = chunks
        .iter()
        .position(|chunk| chunk.path_hash == target)
        .expect("the base skin has an entry");
    let entry = archive::HEADER_LEN as usize + index * archive::ENTRY_LEN;
    let moved: Vec<usize> = (0..before.len())
        .filter(|at| before[*at] != after[*at])
        .collect();
    assert!(
        moved
            .iter()
            .all(|at| (entry..entry + archive::ENTRY_LEN).contains(at)),
        "{} bytes moved outside the entry: {moved:?}",
        moved.len()
    );
}

/// A pack's own archive, so a test can tell the install's bytes from the pack's.
fn pack_archive(path: &Path, character: &str, marker: u8) {
    build_archive(path, &[(skin_bin_path(character, 0), vec![marker; 96])]);
}

/// The install's `Annie.wad.client`, under a game directory of its own.
fn install_with_annie(game: &Path) -> std::path::PathBuf {
    let archive = game
        .join("DATA")
        .join("FINAL")
        .join("Champions")
        .join("Annie.wad.client");
    champion_archive("annie", &archive);
    archive
}

#[test]
fn a_pack_writes_its_archive_into_the_overlay_and_takes_it_back() {
    let game = tempfile::tempdir().unwrap();
    let archive = install_with_annie(game.path());
    let client_bytes = fs::read(&archive).unwrap();

    let storage = tempfile::tempdir().unwrap();
    let packs = tempfile::tempdir().unwrap();
    let pack = packs.path().join("Annie.wad.client");
    pack_archive(&pack, "annie", 9);
    let pack_bytes = fs::read(&pack).unwrap();
    let config = config(game.path());

    let overlay = storage
        .path()
        .join("overlay")
        .join("DATA")
        .join("FINAL")
        .join("Champions")
        .join("Annie.wad.client");

    let applied = apply_pack(&config, storage.path(), &pack, &NullEventSink).unwrap();
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0].archive, "Champions/Annie.wad.client");
    assert_eq!(applied[0].state, SwapState::Applied);
    assert_eq!(applied[0].pack, "Annie.wad.client");
    assert_eq!(
        fs::read(&overlay).unwrap(),
        pack_bytes,
        "the pack's bytes landed in the overlay"
    );
    assert_eq!(
        fs::read(&archive).unwrap(),
        client_bytes,
        "the game's own archive is untouched"
    );

    let listed = pack_swaps(&config, storage.path()).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].state, SwapState::Applied);

    let restored = restore_pack(&config, storage.path(), "Champions/Annie.wad.client").unwrap();
    assert_eq!(restored.state, SwapState::Untouched);
    assert!(!overlay.exists(), "the overlay entry is gone");
    assert_eq!(
        fs::read(&archive).unwrap(),
        client_bytes,
        "byte for byte the client's"
    );
    assert!(pack_swaps(&config, storage.path()).unwrap().is_empty());
}

#[test]
fn a_pack_naming_an_archive_the_install_does_not_hold_is_refused() {
    let game = tempfile::tempdir().unwrap();
    /* The install is laid out the way the other tests lay one out, so the refusal is about the
    pack's name rather than about the directory not looking like an install. */
    install_with_annie(game.path());

    let storage = tempfile::tempdir().unwrap();
    let packs = tempfile::tempdir().unwrap();
    let pack = packs.path().join("Yuumi.wad.client");
    build_archive(
        &pack,
        &[("data/no/such/chunk.bin".to_owned(), vec![1u8; 32])],
    );

    let error =
        apply_pack(&config(game.path()), storage.path(), &pack, &NullEventSink).unwrap_err();
    assert!(
        error.to_string().contains("holds no Yuumi.wad.client"),
        "{error}"
    );
}

#[test]
fn a_pack_that_is_neither_a_fantome_nor_a_wad_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a-skin.txt");
    fs::write(&path, b"not an archive at all").unwrap();

    let error = pack::read(&path).unwrap_err();
    assert!(error.to_string().contains("not a skin pack"), "{error}");
}

#[test]
fn a_wad_named_pack_whose_bytes_are_not_a_wad_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Annie.wad.client");
    fs::write(&path, b"NOTAWAD, whatever else it is").unwrap();

    let error = pack::read(&path).unwrap_err();
    assert!(
        error.to_string().contains("not a version-3 game archive"),
        "{error}"
    );
}

#[test]
fn a_restore_naming_a_path_outside_the_install_is_refused() {
    let game = tempfile::tempdir().unwrap();
    install_with_annie(game.path());
    let storage = tempfile::tempdir().unwrap();
    let config = config(game.path());

    /* A record naming somewhere else, which only a hand-edited record file holds. The write it
    would feed has to resolve through the install rather than through the string, or a page
    could aim it anywhere on the machine. */
    let records = r#"{"packs":{"../../../escape.wad.client":{"pack":"x.fantome",
        "original":{"size":1,"modified":0,"sha256":"aa"},
        "written":{"size":1,"modified":0,"sha256":"bb"}}}}"#;
    let path = storage.path().join("static-swap").join("pack-records.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, records).unwrap();

    let error = restore_pack(&config, storage.path(), "../../../escape.wad.client").unwrap_err();
    assert!(
        error.to_string().contains("the install holds no"),
        "{error}"
    );
}

/// A sink that keeps every event it is handed, for a test about what was announced.
#[derive(Default)]
struct RecordingSink(parking_lot::Mutex<Vec<BackendEvent>>);

impl EventSink for RecordingSink {
    fn emit(&self, event: BackendEvent) {
        self.0.lock().push(event);
    }
}

#[test]
fn a_pack_write_reports_the_stages_it_goes_through() {
    let game = tempfile::tempdir().unwrap();
    install_with_annie(game.path());
    let storage = tempfile::tempdir().unwrap();
    let packs = tempfile::tempdir().unwrap();
    let pack = packs.path().join("Annie.wad.client");
    pack_archive(&pack, "annie", 9);

    let sink = RecordingSink::default();
    apply_pack(&config(game.path()), storage.path(), &pack, &sink).unwrap();

    let announced: Vec<String> = {
        let events = sink.0.lock();
        events
            .iter()
            .filter_map(|event| match event {
                BackendEvent::CnPackProgress(progress) => Some(match progress.stage {
                    CnPackStage::Fetching => "fetching",
                    CnPackStage::Reading => "reading",
                    CnPackStage::Writing => "writing",
                    CnPackStage::Complete => "complete",
                }),
                _ => None,
            })
            .map(str::to_owned)
            .collect()
    };

    assert_eq!(announced, ["reading", "writing", "complete"]);
}

#[test]
fn a_pack_source_that_is_not_an_allowed_host_is_refused() {
    let dir = tempfile::tempdir().unwrap();

    for url in [
        "not a url at all",
        "http://github.com/a/b.fantome",
        "https://example.com/b.fantome",
        "https://raw.githubusercontent.com/a/b/",
        "https://raw.githubusercontent.com/a/..",
    ] {
        let error = download::fetch(url, dir.path()).unwrap_err();
        assert!(
            error.to_string().contains("not a skin pack source"),
            "{url} should be refused: {error}"
        );
    }
}
