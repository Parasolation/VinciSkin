//! Swapping a champion's skin by writing into the game's own archive.
//!
//! The manager's other route to the same effect is the overlay: a built-in mod hands the game a
//! different `skin0.bin` at runtime, and the install is never touched. This one writes the game
//! instead, which is what a machine whose anti-cheat refuses an injected DLL is left with. It is
//! also the route that can break an install, so everything here is built around two rules: a
//! backup exists before the first byte is written, and nothing is ever written into an archive
//! whose current state the record does not agree with.
//!
//! What a swap is made of is [`archive`], which rewrites one chunk in place and leaves the rest
//! of the file where it was. What this module adds is knowing whether that is safe to do now,
//! and being able to put the archive back.
//!
//! What each champion was last given is kept beside the records, in `history`. Putting an archive
//! back ends a swap, and the skin the reader had chosen is not a claim about the file, so it
//! survives that: a page can offer the same skin again without the reader picking it out of the
//! table a second time.
//!
//! A third-party skin pack is the other half of this: what it carries is whole champion archives,
//! which [`pack`] reads and [`apply_pack`] writes over the install's own. A pack has no catalog
//! skin behind it, so its records live in their own file and are keyed by the archive they
//! replaced rather than by a champion.

mod archive;
mod download;
mod pack;
#[cfg(test)]
mod tests;

pub use download::fetch_and_apply;

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;
use std::path::{Path, PathBuf};

use fs_err as fs;
use ltk_wad::WadHash;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::events::{BackendEvent, CnPackProgress, CnPackStage, EventSink};
use crate::overlay::{SkinContent, skin_bin_path, skin_number};
use crate::utils::game::GameDir;

/// Where the backups and the record live, under the manager's storage directory.
const DIR: &str = "static-swap";

/// The record of every champion this machine has written into.
const RECORD_FILE: &str = "records.json";

/// The record of every archive this machine has written a pack over.
const PACK_RECORD_FILE: &str = "pack-records.json";

/// Whether a champion's archive has been written into, as the record and the file agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS, specta::Type))]
#[cfg_attr(feature = "ts", ts(export))]
#[serde(rename_all = "camelCase")]
pub enum SwapState {
    /// No record, so the archive is the client's own.
    Untouched,
    /// The archive is what this manager wrote.
    Applied,
    /// The client put its own archive back, so the backup is what the game already has.
    ///
    /// A patch or a repair does this. The swap is gone, and writing another is safe.
    Reverted,
    /// Something changed the archive that was neither this manager nor a restore.
    ///
    /// The backup no longer describes anything on disk, so neither restoring nor writing over it
    /// is safe. This is the state a patch lands in when it moves a champion's chunks, and only
    /// dropping the record gets out of it.
    Drifted,
    /// The record exists and the backup it names is gone, so nothing can be put back.
    BackupMissing,
}

/// One champion's swap, as a page shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS, specta::Type))]
#[cfg_attr(feature = "ts", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct ChampionSwap {
    /// The character a champion archive is named after, lowercased.
    pub character: String,
    /// The archive's `DATA/FINAL`-relative name.
    pub archive: String,
    /// The state the record and the file agree on.
    pub state: SwapState,
    /// The game's own skin number the champion shows, where one was written.
    pub skin: Option<u32>,
    /// The skin the champion was last given, which outlives the archive being put back.
    ///
    /// A page reads it to offer the same skin again with one press, and it is `None` for a
    /// champion this machine has never written into. It stays set while `skin` does not: putting
    /// the client's archive back ends the swap, and the answer to "what did I have on this
    /// champion?" is still worth having.
    pub last_skin: Option<u32>,
}

/// What one file looked like when the record was written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fingerprint {
    size: u64,
    /// Nanoseconds since the epoch, which is what tells a rewrite of the same length apart.
    modified: i64,
    sha256: String,
}

/// One champion's record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    /// The archive's `DATA/FINAL`-relative name.
    archive: String,
    /// The game's own skin number the champion was given.
    skin: u32,
    /// What the client's archive looked like, which the backup holds.
    original: Fingerprint,
    /// What this manager's archive looks like, so a later run can tell whether it is still there.
    written: Fingerprint,
}

/// Every champion this machine has written into, and what each was last given.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Records {
    #[serde(default)]
    champions: BTreeMap<String, Record>,
    /// The skin each champion was last given, which no operation here clears.
    ///
    /// A record is a claim about a file, so putting the archive back or letting go of a record
    /// removes one. Which skin the reader had chosen is not a claim about anything, so it is
    /// kept: without it, putting a champion back leaves a page with nothing to offer, and the
    /// next swap has to be picked out of the table again. Defaulted, so a file written before
    /// this existed still reads.
    #[serde(default)]
    history: BTreeMap<String, u32>,
}

/// Where a manager's storage directory keeps the backups.
fn root(storage_dir: &Path) -> PathBuf {
    storage_dir.join(DIR)
}

fn record_path(storage_dir: &Path) -> PathBuf {
    root(storage_dir).join(RECORD_FILE)
}

fn read_records(storage_dir: &Path) -> AppResult<Records> {
    let path = record_path(storage_dir);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Records::default()),
        Err(e) => return Err(e.into()),
    };

    serde_json::from_slice(&bytes).map_err(|e| {
        AppError::ValidationFailed(format!(
            "the swap record at {} is unreadable: {e}",
            path.display()
        ))
    })
}

fn write_records(storage_dir: &Path, records: &Records) -> AppResult<()> {
    let path = record_path(storage_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_vec_pretty(records)?)?;
    Ok(())
}

/// The archive named after `character` in `installed`: its `DATA/FINAL`-relative name and where
/// it is.
///
/// A champion is named by the character its archive is named after, matched without case, which
/// is the whole of what the two have in common.
fn archive_named<'a>(
    installed: &'a [(String, PathBuf)],
    character: &str,
) -> Option<(String, &'a Path)> {
    installed.iter().find_map(|(name, path)| {
        let stem = name.split('.').next()?;
        stem.eq_ignore_ascii_case(character)
            .then(|| (format!("Champions/{name}"), path.as_path()))
    })
}

/// The archive a champion's skins live in, and its name relative to `DATA/FINAL`.
fn champion_archive(game_dir: &GameDir, character: &str) -> AppResult<(String, PathBuf)> {
    let installed = game_dir.champion_archives()?;
    archive_named(&installed, character)
        .map(|(name, path)| (name, path.to_path_buf()))
        .ok_or_else(|| {
            AppError::ValidationFailed(format!("no champion archive is named after {character}"))
        })
}

/// Size, modification time and digest of `path`, as one fingerprint.
fn fingerprint(path: &Path) -> AppResult<Fingerprint> {
    let metadata = fs::metadata(path)?;
    let modified = metadata
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos() as i64)
        .unwrap_or(0);

    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(Fingerprint {
        size: metadata.len(),
        modified,
        sha256: format!("{:x}", hasher.finalize()),
    })
}

/// Every champion this machine has written into, with what the record and the file agree on.
///
/// A champion whose record is gone is still listed while a skin is remembered for it, because
/// that is what a page offers back. Such a row carries no skin of its own, since nothing was
/// written: the archive is the client's own.
///
/// # Errors
///
/// Fails when the configured install cannot be resolved or the record cannot be read.
pub fn swaps(config: &Config, storage_dir: &Path) -> AppResult<Vec<ChampionSwap>> {
    let game_dir = GameDir::resolve(config)?;
    let records = read_records(storage_dir)?;

    let named: BTreeSet<&String> = records
        .champions
        .keys()
        .chain(records.history.keys())
        .collect();

    /* Only a row with no record needs the listing to name its archive, and a champion whose
    archive was put back is the rare one, so a library that has never had one does not pay for it. */
    let installed = match named
        .iter()
        .any(|character| !records.champions.contains_key(*character))
    {
        true => game_dir.champion_archives()?,
        false => Vec::new(),
    };

    Ok(named
        .into_iter()
        .map(|character| {
            let last_skin = records.history.get(character).copied();
            let Some(record) = records.champions.get(character) else {
                return ChampionSwap {
                    character: character.clone(),
                    archive: archive_named(&installed, character)
                        .map(|(name, _)| name)
                        .unwrap_or_default(),
                    state: SwapState::Untouched,
                    skin: None,
                    last_skin,
                };
            };

            ChampionSwap {
                character: character.clone(),
                archive: record.archive.clone(),
                state: overlay_state(storage_dir, record).unwrap_or(SwapState::Untouched),
                skin: Some(record.skin),
                last_skin,
            }
        })
        .collect())
}

/// What a record says about the overlay file it wrote, or `None` when it wrote none.
///
/// The overlay is what the injection host serves, so a record whose overlay file is
/// still there is the applied one; the game's own archive is untouched under it.
fn overlay_state(storage_dir: &Path, record: &Record) -> Option<SwapState> {
    let path = overlay_path(storage_dir, &record.archive);
    if !path.is_file() {
        return None;
    }
    Some(match digest(&path) {
        Ok(digest) if digest == record.written.sha256 => SwapState::Applied,
        _ => SwapState::Drifted,
    })
}

/// The digest of one file, which is what settles a question a clock cannot.
fn digest(path: &Path) -> AppResult<String> {
    Ok(fingerprint(path)?.sha256)
}

/// Where an archive lives inside the overlay, mirrored the way the game reads it.
fn overlay_path(storage_dir: &Path, archive: &str) -> PathBuf {
    storage_dir
        .join("overlay")
        .join("DATA")
        .join("FINAL")
        .join(archive)
}

/// Show `character` the hero's catalog skin `skin_id` by writing the stand-in
/// into the overlay the injection host serves.
///
/// The game's own archive is never touched: the host injects a DLL that
/// redirects the game's WAD reads into the overlay. The base skin drops the
/// overlay entry, which is the game's own archive again.
///
/// # Errors
///
/// Fails when the install cannot be resolved, when the champion has no archive,
/// when the pick is the install's to refuse, or when the overlay cannot be
/// written.
pub fn overlay_apply(
    config: &Config,
    storage_dir: &Path,
    character: &str,
    skin_id: u32,
    hero_id: u32,
) -> AppResult<ChampionSwap> {
    let game_dir = GameDir::resolve(config)?;
    let character = character.to_ascii_lowercase();
    let (archive_name, archive) = champion_archive(&game_dir, &character)?;

    let skin = skin_number(hero_id, skin_id).ok_or_else(|| {
        AppError::ValidationFailed(format!("skin {skin_id} is not hero {hero_id}'s"))
    })?;

    let target = overlay_path(storage_dir, &archive_name);

    if skin == 0 {
        if target.is_file() {
            fs::remove_file(&target)?;
        }
        return Ok(ChampionSwap {
            character,
            archive: archive_name,
            state: SwapState::Untouched,
            skin: None,
            last_skin: None,
        });
    }

    if !crate::overlay::install_holds_skin(config, &character, skin)? {
        return Err(AppError::ValidationFailed(format!(
            "the install holds no skin {skin} of {character}"
        )));
    }

    let bytes = stand_in(&archive, &character, skin)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    /* Seeded from the client's own archive, so the overlay file is the whole
    archive the game reads, with only the classic skin slot replaced. */
    fs::copy(&archive, &target)?;
    archive::write_chunk(
        &target,
        WadHash::from(skin_bin_path(&character, 0).as_str()),
        &bytes,
    )?;

    let mut records = read_records(storage_dir)?;
    records.champions.insert(
        character.clone(),
        Record {
            archive: archive_name.clone(),
            skin,
            original: fingerprint(&archive)?,
            written: fingerprint(&target)?,
        },
    );
    records.history.insert(character.clone(), skin);
    write_records(storage_dir, &records)?;

    Ok(ChampionSwap {
        character,
        archive: archive_name,
        state: SwapState::Applied,
        skin: Some(skin),
        last_skin: Some(skin),
    })
}

/// Drop `character`'s overlay entry, so the game reads its own archive again.
///
/// # Errors
///
/// Fails when the install cannot be resolved or the champion has no archive.
pub fn overlay_restore(
    config: &Config,
    storage_dir: &Path,
    character: &str,
) -> AppResult<ChampionSwap> {
    let game_dir = GameDir::resolve(config)?;
    let character = character.to_ascii_lowercase();
    let (archive_name, _) = champion_archive(&game_dir, &character)?;

    let target = overlay_path(storage_dir, &archive_name);
    if target.is_file() {
        fs::remove_file(&target)?;
    }

    let mut records = read_records(storage_dir)?;
    if records.champions.remove(&character).is_some() {
        write_records(storage_dir, &records)?;
    }

    Ok(ChampionSwap {
        character,
        archive: archive_name,
        state: SwapState::Untouched,
        skin: None,
        last_skin: None,
    })
}

/// Remove overlay files built from an archive the install has since replaced.
///
/// A game patch rewrites the champion archives an overlay was seeded from. Serving such a file
/// makes the game parse a chunk table its own archive no longer agrees with, which crashes it, so
/// every overlay whose base archive changed is removed rather than kept. The next apply seeds it
/// from the current archive again.
///
/// Overlay files with no record are removed too: nothing names what skin they hold.
///
/// # Errors
///
/// Fails when the install cannot be resolved or a file cannot be removed.
pub fn overlay_purge_stale(config: &Config, storage_dir: &Path) -> AppResult<usize> {
    let game_dir = GameDir::resolve(config)?;
    let installed = game_dir.champion_archives()?;
    let mut records = read_records(storage_dir)?;
    let mut removed = 0;

    let stale: Vec<String> = records
        .champions
        .iter()
        .filter(|(character, record)| {
            archive_named(&installed, character)
                .map(|(_, path)| archive_changed(path, &record.original))
                .unwrap_or(true)
        })
        .map(|(character, _)| character.clone())
        .collect();

    for character in stale {
        if let Some(record) = records.champions.remove(&character) {
            remove_overlay(storage_dir, &record.archive, &mut removed)?;
        }
    }

    /* A file a previous version wrote, or one whose record was let go of, is served just the
    same, so it goes too. */
    let known: Vec<String> = records
        .champions
        .values()
        .map(|record| record.archive.clone())
        .collect();

    for archive in overlay_archives(storage_dir)? {
        if !known.contains(&archive) {
            remove_overlay(storage_dir, &archive, &mut removed)?;
        }
    }

    write_records(storage_dir, &records)?;
    Ok(removed)
}

/// Whether the install's archive no longer looks like the one a record was seeded from.
fn archive_changed(path: &Path, original: &Fingerprint) -> bool {
    match fs::metadata(path) {
        Ok(metadata) => {
            let modified = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|since| since.as_nanos() as i64)
                .unwrap_or(0);

            metadata.len() != original.size || modified != original.modified
        }
        Err(_) => true,
    }
}

/// Every archive name the overlay directory holds.
fn overlay_archives(storage_dir: &Path) -> AppResult<Vec<String>> {
    let dir = storage_dir
        .join("overlay")
        .join("DATA")
        .join("FINAL")
        .join("Champions");

    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };

    let mut out = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            out.push(format!("Champions/{}", entry.file_name().to_string_lossy()));
        }
    }
    Ok(out)
}

/// Remove one overlay file, counting it when it was there.
fn remove_overlay(storage_dir: &Path, archive: &str, removed: &mut usize) -> AppResult<()> {
    let path = overlay_path(storage_dir, archive);
    if path.is_file() {
        fs::remove_file(&path)?;
        *removed += 1;
    }
    Ok(())
}

/// Forget a champion's record without touching the overlay.
///
/// The way out of a record the overlay no longer agrees with, which is where a hand-made change
/// to the overlay file leaves one.
///
/// What the champion was last given is kept: dropping a record is letting go of a claim about a
/// file, and the memory is not one.
///
/// # Errors
///
/// Fails when the record cannot be written.
pub fn forget(storage_dir: &Path, character: &str) -> AppResult<()> {
    let mut records = read_records(storage_dir)?;
    records.champions.remove(&character.to_ascii_lowercase());
    write_records(storage_dir, &records)
}

/// One archive a skin pack replaced, as a page shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS, specta::Type))]
#[cfg_attr(feature = "ts", ts(export))]
#[serde(rename_all = "camelCase")]
pub struct PackSwap {
    /// The archive's `DATA/FINAL`-relative name.
    pub archive: String,
    /// The pack it was written from, by file name, which is what a page shows.
    pub pack: String,
    /// The state the record and the file agree on.
    pub state: SwapState,
    /// The bytes the record left on disk, so a page can say how much is written.
    pub bytes: u64,
}

/// One pack-written archive's record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackRecord {
    /// The pack's file name, for a page to name what is on the champion.
    pack: String,
    /// What the client's archive looked like, which the backup holds.
    original: Fingerprint,
    /// What the pack left, so a later run can tell whether it is still there.
    written: Fingerprint,
}

/// Every archive this machine has written a pack over.
#[derive(Debug, Default, Serialize, Deserialize)]
struct PackRecords {
    #[serde(default)]
    packs: BTreeMap<String, PackRecord>,
}

fn pack_record_path(storage_dir: &Path) -> PathBuf {
    root(storage_dir).join(PACK_RECORD_FILE)
}

fn read_pack_records(storage_dir: &Path) -> AppResult<PackRecords> {
    let path = pack_record_path(storage_dir);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(PackRecords::default()),
        Err(e) => return Err(e.into()),
    };

    serde_json::from_slice(&bytes).map_err(|e| {
        AppError::ValidationFailed(format!(
            "the pack record at {} is unreadable: {e}",
            path.display()
        ))
    })
}

fn write_pack_records(storage_dir: &Path, records: &PackRecords) -> AppResult<()> {
    let path = pack_record_path(storage_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_vec_pretty(records)?)?;
    Ok(())
}

/// Every archive this machine has written a pack over, with what the record and the file agree on.
///
/// # Errors
///
/// Fails when the configured install cannot be resolved or the record cannot be read.
pub fn pack_swaps(config: &Config, storage_dir: &Path) -> AppResult<Vec<PackSwap>> {
    let _game_dir = GameDir::resolve(config)?;
    let records = read_pack_records(storage_dir)?;

    Ok(records
        .packs
        .into_iter()
        .map(|(archive, record)| {
            let path = overlay_path(storage_dir, &archive);
            PackSwap {
                state: if !path.is_file() {
                    SwapState::Untouched
                } else if digest(&path).is_ok_and(|digest| digest == record.written.sha256) {
                    SwapState::Applied
                } else {
                    SwapState::Drifted
                },
                bytes: record.written.size,
                pack: record.pack,
                archive,
            }
        })
        .collect())
}

/// Write every WAD `pack` carries over the install's own, backing each one up first.
///
/// A pack is a skin, so its WADs are the champion archives an overlay would have mounted. This
/// writes them to disk instead: the archive the pack names is replaced whole, and what was there
/// is kept under the storage directory until it is put back or the record is forgotten.
///
/// # Errors
///
/// Fails when the install cannot be resolved, when the pack cannot be read (see
/// [`pack::read`]), when the install holds no archive the pack names, when an archive was written
/// before and its backup is gone, when an archive changed since the record (a patch or a
/// repair), or when a file cannot be written.
pub fn apply_pack(
    config: &Config,
    storage_dir: &Path,
    pack_path: &Path,
    events: &dyn EventSink,
) -> AppResult<Vec<PackSwap>> {
    let game_dir = GameDir::resolve(config)?;
    let pack_name = pack_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("pack")
        .to_owned();

    events.emit(BackendEvent::CnPackProgress(CnPackProgress {
        stage: CnPackStage::Reading,
        current_archive: None,
        pack: Some(pack_name.clone()),
        current: 0,
        total: 0,
    }));

    let wads = pack::read(pack_path)?;
    let installed = game_dir.champion_archives()?;
    let total = wads.len() as u32;

    let mut records = read_pack_records(storage_dir)?;
    let mut applied = Vec::with_capacity(wads.len());

    for (index, wad) in wads.into_iter().enumerate() {
        let character = wad.name.split('.').next().unwrap_or(wad.name.as_str());
        let (archive_name, archive) = archive_named(&installed, character).ok_or_else(|| {
            AppError::ValidationFailed(format!("the install holds no {}", wad.name))
        })?;
        let target = overlay_path(storage_dir, &archive_name);

        events.emit(BackendEvent::CnPackProgress(CnPackProgress {
            stage: CnPackStage::Writing,
            current_archive: Some(archive_name.clone()),
            pack: Some(pack_name.clone()),
            current: index as u32 + 1,
            total,
        }));

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&target, &wad.bytes)?;
        let written = fingerprint(&target)?;
        records.packs.insert(
            archive_name.clone(),
            PackRecord {
                pack: pack_name.clone(),
                original: fingerprint(archive)?,
                written,
            },
        );
        applied.push(PackSwap {
            archive: archive_name,
            pack: pack_name.clone(),
            state: SwapState::Applied,
            bytes: wad.bytes.len() as u64,
        });
    }

    write_pack_records(storage_dir, &records)?;
    events.emit(BackendEvent::CnPackProgress(CnPackProgress {
        stage: CnPackStage::Complete,
        current_archive: None,
        pack: Some(pack_name),
        current: total,
        total,
    }));
    Ok(applied)
}

/// The archive a page named, resolved through the install rather than trusted as a path.
///
/// A page hands back the `archive` a record carried, and that string is not one this manager
/// built on this call: it arrives over IPC. It feeds a write into the game's own files, so it
/// is matched against what the install actually lists rather than joined onto `DATA/FINAL` -
/// a name carrying `..`, a drive or an absolute root resolves to nothing here instead of
/// somewhere else on the machine.
fn guarded_archive(game_dir: &GameDir, archive: &str) -> AppResult<(String, PathBuf)> {
    game_dir
        .champion_archives()?
        .into_iter()
        .find(|(file, _)| format!("Champions/{file}") == archive)
        .map(|(file, path)| (format!("Champions/{file}"), path))
        .ok_or_else(|| AppError::ValidationFailed(format!("the install holds no {archive}")))
}

/// Put an archive a pack replaced back to the one the client shipped.
///
/// # Errors
///
/// Fails when the install cannot be resolved, when the archive was not written from a pack, when
/// its backup is gone, and when the archive, the backup and the record do not agree about what
/// may be put back.
pub fn restore_pack(config: &Config, storage_dir: &Path, archive: &str) -> AppResult<PackSwap> {
    let game_dir = GameDir::resolve(config)?;

    let mut records = read_pack_records(storage_dir)?;
    let Some(record) = records.packs.get(archive).cloned() else {
        return Err(AppError::ValidationFailed(format!(
            "{archive} was not written from a pack"
        )));
    };

    let (archive, _path) = guarded_archive(&game_dir, archive)?;
    let target = overlay_path(storage_dir, &archive);
    if target.is_file() {
        fs::remove_file(&target)?;
    }

    records.packs.remove(&archive);
    write_pack_records(storage_dir, &records)?;

    Ok(PackSwap {
        archive,
        pack: record.pack,
        state: SwapState::Untouched,
        bytes: record.original.size,
    })
}

/// Forget a pack-written archive's record without touching the archive.
///
/// The way out of [`SwapState::Drifted`], where neither writing nor restoring is safe.
///
/// # Errors
///
/// Fails when the record cannot be written.
pub fn forget_pack(storage_dir: &Path, archive: &str) -> AppResult<()> {
    let mut records = read_pack_records(storage_dir)?;
    records.packs.remove(archive);
    write_pack_records(storage_dir, &records)
}

/// The bytes a champion's `skin0.bin` should hold to show skin `skin` of that same champion.
///
/// The two objects that skin is loaded out of, renamed onto the base skin, which is what the
/// overlay's built-in mod writes and what a swap writes here.
fn stand_in(archive: &Path, character: &str, skin: u32) -> AppResult<Vec<u8>> {
    let mut wad = ltk_wad::Wad::mount(std::io::BufReader::new(fs::File::open(archive)?))?;
    let hash = WadHash::from(skin_bin_path(character, skin).as_str());
    let chunk = *wad
        .chunks()
        .get(hash)
        .ok_or_else(|| AppError::ValidationFailed(format!("{character} holds no skin {skin}")))?;
    let bytes = wad.load_chunk_decompressed(&chunk)?;

    let content = SkinContent::parse(character, skin, &bytes).ok_or_else(|| {
        AppError::ValidationFailed(format!("skin {skin} of {character} holds no skin to copy"))
    })?;
    content.renamed_to(0)
}
