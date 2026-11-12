//! The Chinese catalog, the sources that supply it, and version alignment.

pub mod alignment;
pub mod gtimg;

use serde::{Deserialize, Serialize};

use crate::error::CnError;
use crate::version::PatchVersion;

/// A hero, as the Chinese hero table lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hero {
    /// Riot's numeric hero id, read from the string `heroId`.
    pub id: u32,
    /// The English name, `Annie`.
    pub alias: String,
    /// The Chinese title, `黑暗之女`.
    pub name: String,
    /// The `keywords` field, every search term the client itself indexes the champion by.
    ///
    /// `安妮,黑暗之女,火女,Annie,anni,heianzhinv,huonv,an,hazn,hn`: the nickname first
    /// (`安妮`), then the title, then the full and initial pinyin spellings. Split on the
    /// comma, with blanks dropped. `name` and `alias` are already in there, so the frontend
    /// needs only this one list.
    pub keywords: Vec<String>,
}

/// A skin, as a per-hero skin table lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skin {
    /// Riot's numeric skin id, `1000` for a base skin.
    pub skin_id: u32,
    /// The hero this skin belongs to.
    pub hero_id: u32,
    /// The Chinese skin name.
    pub name: String,
    /// Whether this is the hero's base skin.
    pub is_base: bool,
    /// The `chromaImg`, present only on a chroma entry.
    pub chroma_img: Option<String>,
    /// The loading art, absent on a chroma entry.
    pub loading_img: Option<String>,
    /// The art's UUID, which the image URLs carry in place of the skin id.
    pub instance_id: Option<String>,
    /// The `emblemsName` the table labels the entry with, which is the skin's collection
    /// rather than a sentence, `Epic`, `Legend`, `Year of the Snake 2025`.
    pub emblems_name: Option<String>,
    /// The `chromasBelongId` of a chroma, naming the skin it is a variant of.
    ///
    /// Present only on a chroma entry, and `0` where the table has no parent to name.
    pub chromas_belong_id: Option<u32>,
}

impl Skin {
    /// Whether this entry is a chroma rather than a full skin.
    pub fn is_chroma(&self) -> bool {
        self.chroma_img.is_some()
    }

    /// The skin this entry sets the look of, which is itself unless it is a chroma.
    ///
    /// A chroma carries the id of the skin it varies, so a page can fold the variants of one
    /// skin under it rather than listing a hero's chromas as skins of their own.
    #[must_use]
    pub fn parent_id(&self) -> u32 {
        match self.chromas_belong_id {
            Some(parent) if parent != 0 => parent,
            _ => self.skin_id,
        }
    }

    /// The quality the client labels this entry with, as the tier a page filters by.
    ///
    /// The table's `emblemsName` is the collection a skin belongs to, and the tier words sit
    /// inside it: `Epic` is one, `Hextech Limited` is a limited one, and a skin line such as
    /// `Year of the Snake 2025` names no tier at all. Measured over 173 heroes' tables, where
    /// these are the only tier words the field carries.
    #[must_use]
    pub fn quality(&self) -> Quality {
        let label = self.emblems_name.as_deref().unwrap_or_default();
        Quality::of(label)
    }

    /// The one image the catalog serves for this entry, or none.
    ///
    /// A full skin carries its own `loading_img`, used as it stands. A chroma's
    /// `chroma_img` is deliberately not used: the URL it holds carries a `/1/`
    /// segment that answers 404. The art is built from `instance_id` instead,
    /// which is the same UUID the field names - see
    /// [`gtimg::chroma_image_url`].
    pub fn art_url(&self) -> Option<String> {
        if let Some(url) = &self.loading_img {
            return Some(url.clone());
        }

        self.instance_id.as_deref().map(gtimg::chroma_image_url)
    }
}

/// How rare the client calls a skin, which is what a page's quality filter narrows by.
///
/// Riot's own tier words, and nothing guessed: an entry the table labels with a collection
/// instead of a tier is [`Quality::Common`], which is what such a skin is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Quality {
    /// No tier word on the entry.
    Common,
    /// `Epic`.
    Epic,
    /// `Legend` and `Legendary`.
    Legend,
    /// `Mythic`.
    Mythic,
    /// `Ultimate`.
    Ultimate,
    /// `Prestige`.
    Prestige,
    /// `Exalted`.
    Exalted,
    /// `Limited`, and every collection whose name carries it.
    ///
    /// Last rather than ranked, because being limited says when a skin was sold rather than
    /// how rare it is: a limited epic and a limited ultimate are both limited.
    Limited,
}

impl Quality {
    /// Every tier, in the order a picker lists them.
    pub const ALL: [Self; 8] = [
        Self::Common,
        Self::Epic,
        Self::Legend,
        Self::Mythic,
        Self::Ultimate,
        Self::Prestige,
        Self::Exalted,
        Self::Limited,
    ];

    /// The tier `label` names, where it names one.
    ///
    /// The match is without case and over the whole word, so a collection that merely contains
    /// a tier's letters is not one: `Ultimate` is a tier and `Ultimatum` is not.
    #[must_use]
    pub fn of(label: &str) -> Self {
        let words: Vec<String> = label
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(str::to_ascii_lowercase)
            .collect();

        for word in &words {
            let tier = match word.as_str() {
                "epic" => Self::Epic,
                "legend" | "legendary" => Self::Legend,
                "mythic" => Self::Mythic,
                "prestige" => Self::Prestige,
                "exalted" => Self::Exalted,
                "ultimate" => Self::Ultimate,
                _ => continue,
            };
            return tier;
        }

        if words.iter().any(|word| word == "limited") {
            return Self::Limited;
        }

        Self::Common
    }
}

/// A whole Chinese catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    /// The hero table.
    pub heroes: Vec<Hero>,
    /// The skin table.
    pub skins: Vec<Skin>,
}

/// The hero table with the version its own body reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeroCatalog {
    /// The body's `version`, which carries no revision.
    pub version: PatchVersion,
    /// The heroes.
    pub heroes: Vec<Hero>,
}

/// One hero's skin table with the version its own body reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkinCatalog {
    /// The body's `version`.
    pub version: PatchVersion,
    /// The skins, chromas included.
    pub skins: Vec<Skin>,
}

/// A source of the Chinese catalog.
pub trait CatalogSource {
    /// The authoritative version, which is Riot's Data Dragon version.
    fn authoritative_version(&self) -> Result<PatchVersion, CnError>;

    /// The Chinese catalog for `version`.
    fn fetch(&self, version: &PatchVersion) -> Result<Catalog, CnError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skin(chroma_img: Option<&str>) -> Skin {
        Skin {
            skin_id: 1014,
            hero_id: 1,
            name: "福牛守护者 安妮 贺岁".into(),
            is_base: false,
            chroma_img: chroma_img.map(str::to_string),
            loading_img: None,
            instance_id: None,
            emblems_name: None,
            chromas_belong_id: None,
        }
    }

    fn labelled(emblems_name: &str) -> Skin {
        Skin {
            emblems_name: Some(emblems_name.into()),
            ..skin(None)
        }
    }

    #[test]
    fn a_collection_the_client_labels_a_tier_with_reads_as_that_tier() {
        assert_eq!(Quality::of("Epic"), Quality::Epic);
        assert_eq!(Quality::of("Legend"), Quality::Legend);
        assert_eq!(Quality::of("Legendary"), Quality::Legend);
        assert_eq!(Quality::of("Mythic"), Quality::Mythic);
        assert_eq!(Quality::of("Ultimate"), Quality::Ultimate);
        assert_eq!(Quality::of("Prestige"), Quality::Prestige);
        assert_eq!(Quality::of("Exalted"), Quality::Exalted);
        assert_eq!(Quality::of("common"), Quality::Common);
    }

    /* Limited is what the collection says about when a skin was sold, so it is read wherever it
    stands rather than only as a word of its own. */
    #[test]
    fn a_collection_naming_a_limited_skin_is_limited() {
        assert_eq!(Quality::of("Limited"), Quality::Limited);
        assert_eq!(Quality::of("Hextech Limited"), Quality::Limited);
        assert_eq!(Quality::of("Champion Limited"), Quality::Limited);
    }

    /* A skin line is not a tier, and a word that merely contains one is not that tier. */
    #[test]
    fn a_collection_that_names_no_tier_is_common() {
        assert_eq!(Quality::of("Year of the Snake 2025"), Quality::Common);
        assert_eq!(Quality::of("Snowdown"), Quality::Common);
        assert_eq!(Quality::of("Ultimatum"), Quality::Common);
        assert_eq!(Quality::of(""), Quality::Common);
    }

    /* Epic outranks a collection that also happens to say limited, because the tier the entry
    carries is what it is sold as. */
    #[test]
    fn a_tier_word_outranks_the_limited_word_beside_it() {
        assert_eq!(Quality::of("Limited Epic"), Quality::Epic);
    }

    #[test]
    fn a_skin_reads_its_quality_off_the_label_the_table_gave_it() {
        assert_eq!(
            labelled("Epic").quality(),
            Quality::Epic,
            "the table's own label is what the tier comes from"
        );
        assert_eq!(skin(None).quality(), Quality::Common);
    }

    #[test]
    fn a_chroma_names_the_skin_it_varies() {
        let chroma = Skin {
            chromas_belong_id: Some(1013),
            ..skin(Some(
                "https://game.gtimg.cn/images/lol/act/img/skinloading/1/a.png",
            ))
        };

        assert_eq!(chroma.parent_id(), 1013);
    }

    #[test]
    fn a_skin_with_no_parent_of_its_own_is_its_own_parent() {
        assert_eq!(skin(None).parent_id(), 1014);

        let named_nothing = Skin {
            chromas_belong_id: Some(0),
            ..skin(None)
        };
        assert_eq!(named_nothing.parent_id(), 1014);
    }

    #[test]
    fn a_skin_with_chroma_art_is_a_chroma() {
        assert!(skin(Some(
            "https://game.gtimg.cn/images/lol/act/img/skinloading/1/a.png"
        ))
        .is_chroma());
    }

    #[test]
    fn a_skin_without_chroma_art_is_not_a_chroma() {
        assert!(!skin(None).is_chroma());
    }

    #[test]
    fn a_full_skin_draws_the_loading_art_the_table_gave_it() {
        const ART: &str = "https://game.gtimg.cn/images/lol/act/img/skinloading/46358cd4.jpg";
        let full = Skin {
            loading_img: Some(ART.into()),
            ..skin(None)
        };

        assert_eq!(full.art_url().as_deref(), Some(ART));
    }

    /// The `chromaImg` the table carries holds a `/1/` segment the CDN answers
    /// 404 for, so the URL is built from the instance id rather than read off
    /// the field that names it.
    #[test]
    fn a_chroma_draws_art_built_from_its_instance_id() {
        const UUID: &str = "b2949367-d078-4e01-b52c-2e5e101bfe79";
        let chroma = Skin {
            chroma_img: Some(format!(
                "https://game.gtimg.cn/images/lol/act/img/skinloading/1/{UUID}.png"
            )),
            instance_id: Some(UUID.into()),
            ..skin(None)
        };

        assert_eq!(
            chroma.art_url().as_deref(),
            Some("https://game.gtimg.cn/images/lol/act/img/skinloading/b2949367-d078-4e01-b52c-2e5e101bfe79.png")
        );
        assert_ne!(chroma.art_url(), chroma.chroma_img);
    }

    #[test]
    fn an_entry_with_neither_art_field_has_no_art() {
        assert_eq!(skin(None).art_url(), None);
    }
}
