//! Reading a third-party skin pack, and everything that has to be true before its bytes go over
//! a game archive.
//!
//! A pack is a `.fantome` archive or a bare `.wad.client`, and what it carries is the champion
//! WADs an overlay would have mounted in place of the game's own. Writing them to disk is the
//! static way to the same effect, so this is the reading half; [`super`] writes what it returns.
//!
//! Everything here is a refusal first. The archive format's own placement rules already refuse
//! an entry name that would land outside the archive, so that class is
//! [`FantomeReader`](ltk_fantome::FantomeReader)'s to answer and not restated here. What is left
//! for a caller about to overwrite game files is how much it is asked to write, what shape that
//! is, and whether the result is a game archive at all.

use std::io::{BufReader, Cursor, Read as _};
use std::path::Path;

use fs_err as fs;
use ltk_wad::Wad;

use crate::error::{AppError, AppResult};

/// The most WADs one pack may carry.
///
/// A skin is a handful of archives, so a number past this is not one and is refused before
/// anything is decompressed.
const MAX_WADS: usize = 8;

/// The largest single WAD a pack may carry.
const MAX_WAD_BYTES: u64 = 512 * 1024 * 1024;

/// The largest total a pack may carry.
const MAX_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;

/// The extensions a WAD an install can hold is named with, longest first.
const WAD_EXTENSIONS: [&str; 3] = [".wad.client", ".wad.mobile", ".wad"];

/// One champion WAD a pack carries: the name it installs under, and its bytes.
pub(super) struct PackWad {
    /// The archive's file name, which is what it is matched to the install by.
    pub(super) name: String,
    /// The whole archive, as the pack stores it.
    pub(super) bytes: Vec<u8>,
}

/* Hand-written, because the derived one would print an archive's every byte into whatever
message a failure carries. */
impl std::fmt::Debug for PackWad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PackWad")
            .field("name", &self.name)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

/// The WADs `path` carries, refusing anything that is not a pack of game archives.
///
/// # Errors
///
/// Fails when the file is neither a fantome nor a `.wad.client`, when the archive carries more
/// or larger WADs than the caps allow, when a WAD travels as a directory of files rather than
/// one entry, and when a WAD's bytes are not a version-3 archive.
pub(super) fn read(path: &Path) -> AppResult<Vec<PackWad>> {
    let wads = match starts_with_zip(path)? {
        true => read_fantome(path)?,
        false => read_bare(path)?,
    };
    if wads.is_empty() {
        return Err(refused(path, "it carries no WADs"));
    }
    Ok(wads)
}

/// Whether `path` begins with a zip's local file header, which is what a fantome is.
fn starts_with_zip(path: &Path) -> AppResult<bool> {
    let mut file = fs::File::open(path)?;
    let mut magic = [0u8; 4];
    match file.read_exact(&mut magic) {
        Ok(()) => Ok(&magic == b"PK\x03\x04"),
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// The WADs a `.wad.client` file is, which is one under its own name.
fn read_bare(path: &Path) -> AppResult<Vec<PackWad>> {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return Err(refused(path, "its file name is not readable"));
    };
    if !is_wad_name(name) {
        return Err(refused(
            path,
            "it is neither a fantome archive nor a `.wad.client`",
        ));
    }

    let name = name.to_owned();
    check_size(path, fs::metadata(path)?.len())?;
    let bytes = fs::read(path)?;
    ensure_wad(path, &name, &bytes)?;
    Ok(vec![PackWad { name, bytes }])
}

/// The WADs a `.fantome` carries, each read out of its `WAD/` entry.
fn read_fantome(path: &Path) -> AppResult<Vec<PackWad>> {
    /* The declared sizes first, so an archive promising more than the caps allow is refused
    before a byte of it is decompressed. */
    let declared = declared_wads(path)?;

    let mut reader = ltk_fantome::FantomeReader::new(BufReader::new(fs::File::open(path)?))
        .map_err(|e| refused(path, &e.to_string()))?;

    let mut wads = Vec::with_capacity(declared.len());
    for name in declared {
        let bytes = reader
            .read_packed_wad(ltk_fantome::BASE_LAYER, &name)
            .map_err(|e| refused(path, &e.to_string()))?
            .ok_or_else(|| {
                refused(
                    path,
                    &format!("{name} travels as loose files, which cannot be written over one"),
                )
            })?;
        ensure_wad(path, &name, &bytes)?;
        wads.push(PackWad { name, bytes });
    }
    Ok(wads)
}

/// The WAD names a fantome's `WAD/` entries describe, with the caps checked against their
/// declared sizes.
///
/// Only the entry table is read, so this costs no decompression and a bomb is refused on what it
/// claims rather than on what it costs to find out.
fn declared_wads(path: &Path) -> AppResult<Vec<String>> {
    let mut zip = zip::ZipArchive::new(BufReader::new(fs::File::open(path)?))
        .map_err(|e| refused(path, &e.to_string()))?;

    let mut names: Vec<String> = Vec::new();
    let mut total: u64 = 0;
    for index in 0..zip.len() {
        let entry = zip
            .by_index(index)
            .map_err(|e| refused(path, &e.to_string()))?;
        let Some(ltk_fantome::FantomeEntry::PackedWad { name, .. }) =
            ltk_fantome::classify_entry(entry.name())
        else {
            continue;
        };
        let name = name.to_owned();
        if names.contains(&name) {
            continue;
        }
        if !is_wad_name(&name) {
            return Err(refused(path, &format!("{name} is not a WAD name")));
        }

        let size = entry.size();
        check_size(path, size)?;
        total = total.saturating_add(size);
        if total > MAX_TOTAL_BYTES {
            return Err(refused(
                path,
                &format!("it declares more than {MAX_TOTAL_BYTES} bytes"),
            ));
        }

        names.push(name);
        if names.len() > MAX_WADS {
            return Err(refused(path, &format!("it carries more than {MAX_WADS} WADs")));
        }
    }
    Ok(names)
}

/// Refuse a WAD past the per-file cap.
fn check_size(path: &Path, size: u64) -> AppResult<()> {
    if size > MAX_WAD_BYTES {
        return Err(refused(
            path,
            &format!("it carries a WAD of {size} bytes, past {MAX_WAD_BYTES}"),
        ));
    }
    Ok(())
}

/// Refuse bytes that are not a version-3 WAD, so nothing but an archive goes over one.
fn ensure_wad(pack: &Path, name: &str, bytes: &[u8]) -> AppResult<()> {
    let mut cursor = Cursor::new(bytes);
    let mut header = [0u8; 4];
    let framed = cursor.read_exact(&mut header).is_ok()
        && u16::from_le_bytes([header[0], header[1]]) == 0x5752
        && header[2] == 3;
    if !framed {
        return Err(refused(
            pack,
            &format!("{name} is not a version-3 game archive"),
        ));
    }

    /* The head is one thing and the table another: a name that starts like an archive and holds
    a table nothing can mount is a file that would break the game rather than a pack. */
    Wad::mount(Cursor::new(bytes))
        .map_err(|e| refused(pack, &format!("{name} does not mount: {e}")))?;
    Ok(())
}

/// Whether `name` is a WAD file name this manager will write.
///
/// A pack names the archive it installs over, so the name is a file name and nothing else: it
/// carries no directory, and holds only the characters an archive's own name has.
fn is_wad_name(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    let Some(stem) = WAD_EXTENSIONS
        .iter()
        .find_map(|extension| lowered.strip_suffix(extension))
    else {
        return false;
    };

    !stem.is_empty()
        && stem
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn refused(path: &Path, why: &str) -> AppError {
    AppError::ValidationFailed(format!("{} is not a skin pack: {why}", path.display()))
}
