//! Where a champion's skin lives, and the bin that gives one skin another's content.
//!
//! The catalog numbers a hero's skins from `heroId * 1000` upwards, and the game loads skin
//! `number` from `data/characters/<character>/skins/skin<number>.bin`. A static swap writes
//! that chunk (ADR-0056), so the three readers here are what turn a catalog id into a chunk
//! and answer whether an install holds it at all.

use crate::config::Config;
use crate::error::AppResult;
use crate::game_wads::GameArchives;
use crate::hashtables::LayeredHashDb;
use crate::utils::game::GameDir;
use ltk_hash::BinHash;
use ltk_meta::{Bin, BinObject, PropertyValueEnum};
use ltk_wad::WadHash;
use std::io::Cursor;

/// The game's own number for the skin a client names `skin_id` of `hero_id`, where that id is
/// the hero's.
///
/// A hero's catalog ids run from `heroId * 1000` upwards: `1000` is the base skin, and every
/// skin and chroma the hero has follows in one sequence, with the same gaps the catalog has. So
/// `0` is the hero's own base skin, and the number is what a swap writes into the archive.
///
/// `None` where the id belongs to another hero, which a caller must not apply.
#[must_use]
pub fn skin_number(hero_id: u32, skin_id: u32) -> Option<u32> {
    let base = hero_id.checked_mul(1000)?;
    (skin_id >= base).then(|| skin_id - base)
}

/// The chunk the game loads a champion's skin `number` from.
///
/// `data/characters/<character>/skins/skin<number>.bin`, the character being the one a champion
/// archive is named after, lowercased. The bin carries no padding where the assets under it do:
/// the base skin is `skin0.bin`, and its assets sit in `assets/characters/<character>/skins/base/`.
#[must_use]
pub fn skin_bin_path(character: &str, number: u32) -> String {
    format!(
        "data/characters/{}/skins/skin{number}.bin",
        character.to_ascii_lowercase()
    )
}

/// Whether the install `config` names holds the bin a champion's skin `number` is loaded from.
///
/// The catalog is the client's own table and the installed files are the other half of it: a skin
/// the client lists still has nothing to show on an install that predates it. The champion is
/// named by the character its archive is named after, matched without case.
///
/// # Errors
///
/// Fails when the install cannot be resolved, the champions directory cannot be listed, or the
/// champion's archive cannot be read.
pub fn install_holds_skin(config: &Config, character: &str, number: u32) -> AppResult<bool> {
    let game_dir = GameDir::resolve(config)?;
    let Some(name) = champion_wad(&game_dir, character)? else {
        return Ok(false);
    };

    /* No table is asked for a name: the chunk is found by hash, and an archive's table already
    holds every one of them. */
    let wanted = WadHash::from(skin_bin_path(character, number).as_str()).0;
    let mut held = false;
    GameArchives::at(game_dir.path()).for_each_chunk(
        &name,
        &LayeredHashDb::new(),
        |hash, _, _| {
            held |= hash == wanted;
        },
    )?;

    Ok(held)
}

/// The `DATA/FINAL`-relative name of the champion archive named after `character`, where the
/// install holds one.
fn champion_wad(game_dir: &GameDir, character: &str) -> AppResult<Option<String>> {
    Ok(game_dir
        .champion_archives()?
        .into_iter()
        .find_map(|(name, _)| {
            let stem = name.split('.').next()?;
            stem.eq_ignore_ascii_case(character)
                .then(|| format!("Champions/{name}"))
        }))
}

/// One skin's bin, the two objects a copy for another skin is built out of.
///
/// A skin is loaded out of two objects, `Characters/<character>/Skins/Skin<id>` and its
/// `/Resources`, and the properties hold a link to the resources. Renaming both and the link
/// between them is what turns one skin's content into another's.
#[derive(Debug, Clone)]
pub(crate) struct SkinContent {
    /// The character the objects belong to, lowercased.
    character: String,
    /// The skin the objects were read out of.
    source: u32,
    /// `Characters/<character>/Skins/Skin<source>`.
    properties: BinObject,
    /// `Characters/<character>/Skins/Skin<source>/Resources`.
    resources: BinObject,
    dependencies: Vec<String>,
}

impl SkinContent {
    /// Skin `id` of `character` out of the bytes of its `skin<id>.bin`, where they hold both of
    /// its objects.
    pub(crate) fn parse(character: &str, id: u32, bytes: &[u8]) -> Option<Self> {
        let mut bin = Bin::from_reader(&mut Cursor::new(bytes)).ok()?;
        let properties = bin.objects.shift_remove(&object_hash(character, id, ""))?;
        let resources = bin
            .objects
            .shift_remove(&object_hash(character, id, RESOURCES))?;
        Some(Self {
            character: character.to_ascii_lowercase(),
            source: id,
            properties,
            resources,
            dependencies: bin.dependencies,
        })
    }

    /// The `skin<id>.bin` holding this skin's objects under skin `id`'s names.
    ///
    /// It depends on the bin the objects were read out of, where the objects the two copies link
    /// to live, and on every bin that one depends on.
    ///
    /// # Errors
    ///
    /// Fails when the bin cannot be written.
    pub(crate) fn renamed_to(&self, id: u32) -> AppResult<Vec<u8>> {
        let source_resources = object_hash(&self.character, self.source, RESOURCES);
        let resources_hash = object_hash(&self.character, id, RESOURCES);

        let mut properties = self.properties.clone();
        properties.path_hash = object_hash(&self.character, id, "");
        for value in properties.properties.values_mut() {
            if let PropertyValueEnum::ObjectLink(link) = value
                && link.value == source_resources
            {
                link.value = resources_hash;
            }
        }
        let mut resources = self.resources.clone();
        resources.path_hash = resources_hash;

        let mut bin = Bin::default();
        bin.dependencies.clone_from(&self.dependencies);
        bin.dependencies.push(format!(
            "DATA/Characters/{}/Skins/Skin{}.bin",
            self.character, self.source
        ));
        bin.objects.insert(properties.path_hash, properties);
        bin.objects.insert(resources.path_hash, resources);

        let mut bytes = Cursor::new(Vec::new());
        bin.to_writer(&mut bytes)?;
        Ok(bytes.into_inner())
    }
}

const RESOURCES: &str = "/Resources";

/// `Characters/<character>/Skins/Skin<id><suffix>`.
fn object_hash(character: &str, id: u32, suffix: &str) -> BinHash {
    BinHash::from(format!("Characters/{character}/Skins/Skin{id}{suffix}").as_str())
}
