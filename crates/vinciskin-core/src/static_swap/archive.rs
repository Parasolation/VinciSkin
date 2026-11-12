//! Writing one chunk of an installed game archive.
//!
//! A skin swap rewrites a champion's `skins/skin0.bin` inside that champion's own
//! `.wad.client`. Rebuilding the archive is not an option: two thirds of a real champion
//! archive's chunks are `ZstdMulti`, whose bytes only decode alongside the subchunk records
//! naming them, so a rebuild would have to carry that table over and understand Riot's own file
//! layout to place it again.
//!
//! What is written instead is the one entry that changed. A version-3 archive keeps its table of
//! contents at the head, one 32-byte entry per chunk in path-hash order, and the chunk bodies
//! after it; every body is found through its own entry, so moving one body moves nothing else.
//! A replacement that fits the room the chunk already had goes there and leaves even the file's
//! length alone. One that does not - which is the ordinary case, because the stand-in of a later
//! skin is a bigger payload than the base skin's own bin - is appended to the end of the file
//! and its entry is pointed at it. The archive grows by that much and nothing else changes.

use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::path::Path;

use fs_err as fs;
use ltk_wad::{EncodedChunk, Wad, WadChunkCompression, WadHash};

use crate::error::{AppError, AppResult};

/// A version-3 archive's header: magic (2), version (2), signature (256), checksum (8), count (4).
pub(super) const HEADER_LEN: u64 = 272;

/// One table-of-contents entry, whichever version-3 minor wrote it.
pub(super) const ENTRY_LEN: usize = 32;

/// The first version-3 minor whose entry carries no `is_duplicated` flag.
///
/// The two layouts are the same length; they differ in what the three bytes between the codec
/// byte and the checksum mean, and this is what tells them apart when one is written back.
const FIRST_FLAGLESS_MINOR: u8 = 4;

/// Zstd levels tried, strongest first, until the replacement fits the room the chunk had.
///
/// A chunk body of a few kilobytes compresses in well under a millisecond at any of these, so
/// the order costs nothing and the strongest that fits is the one that keeps the most headroom
/// for a replacement that grows.
pub(super) const LEVELS: [i32; 7] = [22, 19, 15, 12, 9, 6, 3];

/// What a write put in, and where it had to put it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Written {
    /// The codec the replacement went in under.
    pub(super) compression: WadChunkCompression,
    /// Bytes the encoded replacement occupies.
    pub(super) used: usize,
    /// Bytes the chunk held before.
    pub(super) room: usize,
    /// Whether the body had to go past the end of the file, which is the ordinary case.
    pub(super) appended: bool,
}

/// Write `bytes` over the chunk `target` of the archive at `path`.
///
/// The body goes into the room the chunk already had when it fits there, and is appended to the
/// end of the file when it does not. Either way the entry that names it is the only entry
/// rewritten, the table keeps its length, and every other chunk keeps its offset and its bytes.
/// The archive is left as it was where anything fails before the first write.
///
/// # Errors
///
/// Fails with [`AppError::ValidationFailed`] when the file is not a version-3 archive or holds
/// no chunk with `target`, and with an I/O error when the file cannot be written - which is what
/// a running game holding it looks like.
pub(super) fn write_chunk(path: &Path, target: WadHash, bytes: &[u8]) -> AppResult<Written> {
    let minor = version_minor(path)?;
    let (index, chunk) = locate(path, target)?;

    let room = chunk.compressed_size;
    let encoded = encode(bytes);
    let appended = encoded.compressed().len() > room;

    let entry_offset = HEADER_LEN + (index as u64) * (ENTRY_LEN as u64);
    let mut file = fs::OpenOptions::new().read(true).write(true).open(path)?;

    let length = file.metadata()?.len();
    let offset = if appended {
        length
    } else {
        chunk.data_offset as u64
    };

    /* The entry first, because it is one sector and the body may be many: a body that fails part
    way leaves an entry pointing at nothing, and a reader that stops now at least stops on a
    chunk it cannot read rather than on one whose checksum disagrees with its bytes. */
    let mut entry = [0u8; ENTRY_LEN];
    file.seek(SeekFrom::Start(entry_offset))?;
    file.read_exact(&mut entry)?;
    stamp_entry(&mut entry, minor, offset, &encoded);
    file.seek(SeekFrom::Start(entry_offset))?;
    file.write_all(&entry)?;

    file.seek(SeekFrom::Start(offset))?;
    file.write_all(encoded.compressed())?;
    /* Zeroed, so a body that shrank in place leaves nothing of the skin it replaced behind. */
    if !appended {
        file.write_all(&vec![0u8; room - encoded.compressed().len()])?;
    }
    file.sync_all()?;

    Ok(Written {
        compression: encoded.compression(),
        used: encoded.compressed().len(),
        room,
        appended,
    })
}

/// The archive's version-3 minor, which is what says how a table entry is laid out.
fn version_minor(path: &Path) -> AppResult<u8> {
    let mut file = fs::File::open(path)?;
    let mut header = [0u8; 4];
    file.read_exact(&mut header)?;

    if u16::from_le_bytes([header[0], header[1]]) != 0x5752 {
        return Err(not_our_archive(path, "it does not start with `RW`"));
    }

    let major = header[2];
    let minor = header[3];
    if major != 3 {
        return Err(not_our_archive(
            path,
            &format!("it is version {major}.{minor}, not 3.x"),
        ));
    }

    Ok(minor)
}

/// The index and entry of the chunk `target` in the archive at `path`.
///
/// The mounted table is in the file's own order, so an index into it is the index the entry
/// sits at.
fn locate(path: &Path, target: WadHash) -> AppResult<(usize, ltk_wad::WadChunk)> {
    let wad = Wad::mount(std::io::BufReader::new(fs::File::open(path)?))
        .map_err(|e| not_our_archive_raw(path, e))?;

    wad.chunks()
        .as_slice()
        .iter()
        .position(|chunk| chunk.path_hash == target)
        .map(|index| (index, wad.chunks().as_slice()[index]))
        .ok_or_else(|| {
            AppError::ValidationFailed(format!("{} holds no chunk {target:016x}", path.display()))
        })
}

/// `bytes` under the smallest codec this crate can produce for them.
///
/// Stored as they stand when compressing makes them bigger, which is what the client does with
/// a chunk of its own that will not compress - measured against the client's storage, this
/// crate's zstd reaches the same size it did on the same bytes, so there is nothing here it
/// knows that this does not.
fn encode(bytes: &[u8]) -> EncodedChunk {
    let stored = bytes.to_vec();
    let mut best: Option<Vec<u8>> = None;

    for level in LEVELS {
        let Ok(compressed) = zstd::bulk::compress(bytes, level) else {
            continue;
        };
        if best
            .as_ref()
            .is_none_or(|held| compressed.len() < held.len())
        {
            best = Some(compressed);
        }
    }

    match best {
        Some(compressed) if compressed.len() < stored.len() => {
            EncodedChunk::new(compressed, bytes.len() as u32, WadChunkCompression::Zstd)
        }
        _ => EncodedChunk::new(stored, bytes.len() as u32, WadChunkCompression::None),
    }
}

/// Rewrite the entry `entry` so it describes the body just written at `offset`.
///
/// Everything but the offset, the sizes, the codec and the checksum is left as the client wrote
/// it, and the flag a version-3.1 entry carries between the codec byte and the start frame is
/// the client's business rather than ours.
fn stamp_entry(entry: &mut [u8; ENTRY_LEN], minor: u8, offset: u64, encoded: &EncodedChunk) {
    entry[8..12].copy_from_slice(&(offset as u32).to_le_bytes());
    entry[12..16].copy_from_slice(&(encoded.compressed().len() as u32).to_le_bytes());
    entry[16..20].copy_from_slice(&encoded.uncompressed_size().to_le_bytes());
    /* Frame count zero, which is what a chunk that is not multi-frame carries. */
    entry[20] = encoded.compression() as u8;

    if minor >= FIRST_FLAGLESS_MINOR {
        entry[21..24].fill(0);
    } else {
        /* A version-3.1 entry keeps `is_duplicated` here, then a 16-bit start frame. */
        entry[22..24].fill(0);
    }

    entry[24..32].copy_from_slice(&encoded.checksum().to_le_bytes());
}

fn not_our_archive(path: &Path, why: &str) -> AppError {
    AppError::ValidationFailed(format!("{} is not a game archive: {why}", path.display()))
}

fn not_our_archive_raw(path: &Path, error: ltk_wad::WadError) -> AppError {
    not_our_archive(path, &error.to_string())
}
