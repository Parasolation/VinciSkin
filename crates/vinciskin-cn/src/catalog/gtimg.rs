//! URL construction and body parsing for the Tencent CDN.

use serde_json::Value;

use crate::catalog::{Hero, HeroCatalog, Skin, SkinCatalog};
use crate::error::CnError;
use crate::version::PatchVersion;

/// The Tencent CDN root.
pub const BASE: &str = "https://game.gtimg.cn";

/// The prefix every trusted image URL starts with.
pub const IMAGE_PREFIX: &str = "https://game.gtimg.cn/images/lol/act/img/";

/// The hero table, checked against the live CDN at 112 KB.
pub const HERO_LIST_URL: &str = "https://game.gtimg.cn/images/lol/act/img/js/heroList/hero_list.js";

/// One hero's skin table, confirmed against the live CDN at 60 KB.
pub fn hero_skin_url(hero_id: u32) -> String {
    format!("{BASE}/images/lol/act/img/js/hero/{hero_id}.js")
}

/// One chroma's art, addressed by the UUID the table carries as `instanceId`.
///
/// Built rather than read: the `chromaImg` field points at
/// `img/skinloading/1/<uuid>.png`, and that `/1/` segment answers 404. The art
/// sits at `img/skinloading/<uuid>.png`, which is where a full skin's loading
/// art lives too. Measured against the live CDN on 40 of one hero's chromas,
/// all of which the field 404s and this path serves.
pub fn chroma_image_url(instance_id: &str) -> String {
    format!("{IMAGE_PREFIX}skinloading/{instance_id}.png")
}

/// One hero's portrait, the small square the client lists a champion by.
///
/// `images/lol/act/img/champion/<alias>.png`, the alias spelled as the hero table spells it.
/// Measured against the live CDN on twelve aliases, the four newest among them: `120x120` for
/// the champions that predate the newest batch and `128x128` since, about 20 KB either way.
///
/// `None` for an alias that is not a plain identifier: the path is built out of a field a body
/// supplies, so a name carrying a separator must not become part of it.
#[must_use]
pub fn hero_portrait_url(alias: &str) -> Option<String> {
    let plain = !alias.is_empty() && alias.bytes().all(|byte| byte.is_ascii_alphanumeric());
    plain.then(|| format!("{IMAGE_PREFIX}champion/{alias}.png"))
}

/// Whether `url` is an official image on the Tencent CDN.
///
/// The character set is the one the client's own check allows, so `?`, `&`,
/// `=`, `%` and `#` are rejected along with any other host. A UUID path segment
/// and the nested `skinloading/1/` directory stay inside it.
pub fn is_trusted_image_url(url: &str) -> bool {
    if !url.starts_with(IMAGE_PREFIX) {
        return false;
    }

    if url.split('/').any(|segment| segment == "..") {
        return false;
    }

    url.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'/' | b'_' | b'.' | b'-')
    })
}

/// The hero table and the version its body reports.
///
/// The shape was confirmed against a live response, which is the fixture under
/// `tests/fixtures/hero_list.js`. A renamed field comes back as
/// [`CnError::CatalogParse`] rather than a default.
pub fn parse_hero_list(json: &str) -> Result<HeroCatalog, CnError> {
    let body = parse_body(json)?;

    let version = body
        .get("version")
        .and_then(Value::as_str)
        .ok_or_else(|| detail("hero list has no version string"))?;

    let entries = body
        .get("hero")
        .and_then(Value::as_array)
        .ok_or_else(|| detail("hero list has no hero array"))?;

    let heroes = entries
        .iter()
        .map(parse_hero)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(HeroCatalog {
        version: PatchVersion::parse(version)?,
        heroes,
    })
}

/// One hero's skins and the version its body reports.
///
/// The body is `hero/{heroId}.js`, confirmed against a live response, which is
/// the fixture under `tests/fixtures/hero_1.js`. Chromas arrive in `skins`
/// beside the full skins rather than in a table of their own.
///
/// ASSUMPTION: this per-hero body is the only skin table seen. Whether the CDN
/// also serves one combined skin list is unknown, and `chromasBelongId` was
/// populated on the one hero checked, which is not enough to call it reliable.
pub fn parse_hero_skins(json: &str) -> Result<SkinCatalog, CnError> {
    let body = parse_body(json)?;

    let version = body
        .get("version")
        .and_then(Value::as_str)
        .ok_or_else(|| detail("skin table has no version string"))?;

    let entries = body
        .get("skins")
        .and_then(Value::as_array)
        .ok_or_else(|| detail("skin table has no skins array"))?;

    let skins = entries
        .iter()
        .map(parse_skin)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SkinCatalog {
        version: PatchVersion::parse(version)?,
        skins,
    })
}

fn parse_hero(value: &Value) -> Result<Hero, CnError> {
    Ok(Hero {
        id: number_field(value, "heroId")?,
        alias: string_field(value, "alias")?,
        name: string_field(value, "name")?,
        keywords: keyword_list(value),
    })
}

/// The `keywords` field, split on the commas that separate its terms.
///
/// Missing or empty reads as no terms rather than an error: the field is a search aid, so a
/// client that stops shipping it costs search breadth rather than the whole catalog.
fn keyword_list(value: &Value) -> Vec<String> {
    let Some(text) = optional_string(value, "keywords") else {
        return Vec::new();
    };

    text.split(',')
        .map(str::trim)
        .filter(|term| !term.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_skin(value: &Value) -> Result<Skin, CnError> {
    Ok(Skin {
        skin_id: number_field(value, "skinId")?,
        hero_id: number_field(value, "heroId")?,
        name: string_field(value, "name")?,
        is_base: bool_field(value, "isBase").unwrap_or(false),
        chroma_img: optional_string(value, "chromaImg"),
        loading_img: optional_string(value, "loadingImg"),
        instance_id: optional_string(value, "instanceId"),
        emblems_name: optional_string(value, "emblemsName"),
        chromas_belong_id: optional_number(value, "chromasBelongId"),
    })
}

fn parse_body(json: &str) -> Result<Value, CnError> {
    serde_json::from_str(strip_bom(json)).map_err(|error| CnError::CatalogParse(error.to_string()))
}

fn strip_bom(json: &str) -> &str {
    json.strip_prefix('\u{feff}').unwrap_or(json)
}

fn number_field(value: &Value, key: &str) -> Result<u32, CnError> {
    let Some(raw) = value.get(key) else {
        return Err(detail(&format!("missing {key}")));
    };

    if let Some(number) = raw.as_u64() {
        return u32::try_from(number).map_err(|_| detail(&format!("{key} is out of range")));
    }

    if let Some(text) = raw.as_str() {
        return text
            .trim()
            .parse::<u32>()
            .map_err(|_| detail(&format!("{key} is not a number")));
    }

    Err(detail(&format!("{key} is not a number")))
}

fn string_field(value: &Value, key: &str) -> Result<String, CnError> {
    text_of(value.get(key)).ok_or_else(|| detail(&format!("missing {key} string")))
}

/// A field the API may send as a string or a number, absent or empty read as none.
fn optional_string(value: &Value, key: &str) -> Option<String> {
    let text = text_of(value.get(key))?;
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.to_string())
}

/// A field the API may send as a number or a numeric string, absent or unparsable as none.
fn optional_number(value: &Value, key: &str) -> Option<u32> {
    let text = optional_string(value, key)?;

    text.parse().ok()
}

fn bool_field(value: &Value, key: &str) -> Option<bool> {
    match value.get(key)? {
        Value::Bool(flag) => Some(*flag),
        Value::Number(number) => number.as_u64().map(|count| count != 0),
        Value::String(text) => match text.trim() {
            "1" | "true" => Some(true),
            "0" | "false" | "" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn text_of(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn detail(message: &str) -> CnError {
    CnError::CatalogParse(message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::Quality;

    const HERO_LIST_FIXTURE: &str = include_str!("../../tests/fixtures/hero_list.js");
    const HERO_SKINS_FIXTURE: &str = include_str!("../../tests/fixtures/hero_1.js");

    const BASE_SKIN_LOADING: &str =
        "https://game.gtimg.cn/images/lol/act/img/skinloading/0b95894e-0df2-470e-b282-6c5f5cf41955.jpg";

    fn fixture_skins() -> Vec<Skin> {
        parse_hero_skins(HERO_SKINS_FIXTURE)
            .expect("fixture parses")
            .skins
    }

    fn collect_image_urls(value: &Value, urls: &mut Vec<String>) {
        match value {
            Value::String(text) if text.starts_with(IMAGE_PREFIX) => urls.push(text.clone()),
            Value::Array(items) => items.iter().for_each(|item| collect_image_urls(item, urls)),
            Value::Object(fields) => fields
                .values()
                .for_each(|field| collect_image_urls(field, urls)),
            _ => {}
        }
    }

    fn skin_of(skin_id: u32) -> Skin {
        fixture_skins()
            .into_iter()
            .find(|skin| skin.skin_id == skin_id)
            .unwrap_or_else(|| panic!("skin {skin_id} is in the fixture"))
    }

    #[test]
    fn the_confirmed_urls_match_the_cdn_paths() {
        assert_eq!(
            HERO_LIST_URL,
            "https://game.gtimg.cn/images/lol/act/img/js/heroList/hero_list.js"
        );
        assert_eq!(
            hero_skin_url(1),
            "https://game.gtimg.cn/images/lol/act/img/js/hero/1.js"
        );
    }

    /* The portrait sits under `champion/`, spelled as the table spells the alias, and the URL it
    builds is one the client's own trust check accepts. */
    #[test]
    fn a_hero_portrait_is_built_from_its_alias() {
        assert_eq!(
            hero_portrait_url("MonkeyKing").as_deref(),
            Some("https://game.gtimg.cn/images/lol/act/img/champion/MonkeyKing.png")
        );

        let url = hero_portrait_url("Annie").expect("a plain alias has a portrait");
        assert!(is_trusted_image_url(&url), "{url}");
    }

    /* The alias is a field a body supplies, so anything that is not a plain identifier names no
    portrait rather than becoming part of a path. */
    #[test]
    fn an_alias_that_is_not_a_plain_identifier_names_no_portrait() {
        for alias in ["", "..", "../Annie", "Annie/../Olaf", "Annie.png", "安妮", "A nnie"] {
            assert_eq!(hero_portrait_url(alias), None, "{alias}");
        }
    }

    #[test]
    fn the_real_hero_list_fixture_parses() {
        let catalog = parse_hero_list(HERO_LIST_FIXTURE).expect("fixture parses");

        assert_eq!(catalog.heroes.len(), 173);
        assert_eq!(catalog.version, PatchVersion::parse("16.19").unwrap());

        let annie = catalog
            .heroes
            .iter()
            .find(|hero| hero.id == 1)
            .expect("hero 1");
        assert_eq!(annie.alias, "Annie");
        assert_eq!(annie.name, "黑暗之女");
    }

    #[test]
    fn a_hero_list_parses_its_version_and_heroes() {
        let json =
            r#"{"hero":[{"heroId":"1","name":"黑暗之女","alias":"Annie"}],"version":"16.19"}"#;
        let catalog = parse_hero_list(json).unwrap();

        assert_eq!(catalog.version, PatchVersion::parse("16.19").unwrap());
        assert_eq!(
            catalog.heroes,
            vec![Hero {
                id: 1,
                alias: "Annie".into(),
                name: "黑暗之女".into(),
                keywords: Vec::new()
            }]
        );
    }

    /* The keywords are what a player searches by, so a body that ships them reaches the
    catalog whole: the nickname, the title, and the pinyin spellings, blanks dropped. */
    #[test]
    fn a_hero_list_splits_its_keywords_into_search_terms() {
        let json = r#"{"hero":[{"heroId":"157","name":"疾风剑豪","alias":"Yasuo","keywords":"疾风剑豪,剑豪,亚索,Yasuo,ys,jh, jfjh ,yaso u"}],"version":"16.19"}"#;
        let catalog = parse_hero_list(json).unwrap();

        assert_eq!(
            catalog.heroes[0].keywords,
            vec![
                "疾风剑豪", "剑豪", "亚索", "Yasuo", "ys", "jh", "jfjh", "yaso u"
            ]
        );
    }

    /* A client that stops shipping keywords costs search breadth, not the catalog, so a hero
    without them still parses. */
    #[test]
    fn a_hero_without_keywords_parses_with_none() {
        let json = r#"{"hero":[{"heroId":"1","name":"黑暗之女","alias":"Annie"}],"version":"16.19"}"#;
        let catalog = parse_hero_list(json).unwrap();

        assert!(catalog.heroes[0].keywords.is_empty());
    }

    /* The real table ships the field on every champion, and its terms are what `亚索` reaches. */
    #[test]
    fn the_real_hero_list_ships_keywords_on_every_hero() {
        let catalog = parse_hero_list(HERO_LIST_FIXTURE).expect("fixture parses");

        for hero in &catalog.heroes {
            assert!(!hero.keywords.is_empty(), "{} carries no keywords", hero.alias);
        }

        let yasuo = catalog
            .heroes
            .iter()
            .find(|hero| hero.id == 157)
            .expect("hero 157");
        assert!(yasuo.keywords.iter().any(|term| term == "亚索"));
        assert!(yasuo.keywords.iter().any(|term| term == "ys"));
    }

    #[test]
    fn a_leading_byte_order_mark_is_ignored() {
        let json = r#"{"hero":[],"version":"16.19"}"#;
        assert!(parse_hero_list(&format!("\u{feff}{json}")).is_ok());
    }

    #[test]
    fn a_hero_without_an_alias_is_a_parse_error() {
        let json = r#"{"hero":[{"heroId":"1","name":"x"}],"version":"16.19"}"#;
        assert!(matches!(
            parse_hero_list(json),
            Err(CnError::CatalogParse(_))
        ));
    }

    #[test]
    fn a_hero_list_on_a_broken_body_is_a_parse_error() {
        assert!(matches!(
            parse_hero_list("<html>"),
            Err(CnError::CatalogParse(_))
        ));
    }

    #[test]
    fn a_hero_list_without_a_version_is_a_parse_error() {
        assert!(matches!(
            parse_hero_list(r#"{"hero":[]}"#),
            Err(CnError::CatalogParse(_))
        ));
    }

    #[test]
    fn the_real_hero_skins_fixture_parses_sixty_skins() {
        let catalog = parse_hero_skins(HERO_SKINS_FIXTURE).expect("fixture parses");

        assert_eq!(catalog.skins.len(), 60);
        assert_eq!(catalog.version, PatchVersion::parse("16.19").unwrap());
    }

    #[test]
    fn the_real_base_skin_is_hero_ones_skin_one_thousand() {
        let base = skin_of(1000);

        assert_eq!(base.hero_id, 1);
        assert!(base.is_base);
        assert_eq!(base.name, "黑暗之女");
        assert!(!base.is_chroma());
    }

    #[test]
    fn the_real_base_skin_url_comes_from_the_body_not_the_skin_id() {
        assert_eq!(
            skin_of(1000).loading_img.as_deref(),
            Some(BASE_SKIN_LOADING)
        );
    }

    #[test]
    fn a_real_skin_whose_chromas_are_separate_entries_is_not_a_chroma() {
        let parent = skin_of(1013);

        assert_eq!(parent.name, "福牛守护者 安妮");
        assert!(!parent.is_chroma());
        assert!(parent.chroma_img.is_none());
        assert!(parent.loading_img.is_some());
    }

    #[test]
    fn a_real_chroma_entry_is_marked_by_its_chroma_art() {
        let chroma = skin_of(1014);

        assert_eq!(chroma.name, "福牛守护者 安妮 贺岁");
        assert!(chroma.is_chroma());
        assert!(chroma.chroma_img.is_some());
        assert!(chroma.loading_img.is_none());
    }

    /* The two fields a page's filters are built on: the collection the client labels an entry
    with, and the skin a chroma varies. */
    #[test]
    fn the_real_table_labels_each_entry_with_its_collection() {
        assert_eq!(skin_of(1000).emblems_name.as_deref(), Some("base"));
        assert_eq!(skin_of(1005).emblems_name.as_deref(), Some("Epic"));
        assert_eq!(skin_of(1013).emblems_name.as_deref(), Some("Year of Ox"));

        assert_eq!(skin_of(1005).quality(), Quality::Epic);
        assert_eq!(skin_of(1013).quality(), Quality::Common);
    }

    #[test]
    fn the_real_table_names_the_skin_a_chroma_varies() {
        let parent = skin_of(1013);
        assert_eq!(parent.parent_id(), 1013, "a skin is its own parent");

        let chroma = skin_of(1014);
        assert_eq!(chroma.chromas_belong_id, Some(1013));
        assert_eq!(chroma.parent_id(), 1013);
    }

    /* The body names the parent as a string, and a page folds chromas under it by that id. */
    #[test]
    fn a_parent_named_as_a_string_still_reads() {
        let json = r#"{"skins":[{"skinId":"1014","heroId":"1","name":"x","chromas":"1","chromasBelongId":"1013","emblemsName":"Epic"}],"version":"16.19"}"#;
        let skin = parse_hero_skins(json).unwrap().skins.remove(0);

        assert_eq!(skin.chromas_belong_id, Some(1013));
        assert_eq!(skin.emblems_name.as_deref(), Some("Epic"));
    }

    #[test]
    fn a_parent_of_zero_reads_as_absent_and_parses() {
        let json = r#"{"skins":[{"skinId":"1000","heroId":"1","name":"x","chromasBelongId":"0"}],"version":"16.19"}"#;
        let skin = parse_hero_skins(json).unwrap().skins.remove(0);

        assert_eq!(skin.chromas_belong_id, Some(0));
        assert_eq!(skin.parent_id(), 1000);
    }

    #[test]
    fn every_real_image_url_is_trusted() {
        let body: Value = serde_json::from_str(strip_bom(HERO_SKINS_FIXTURE)).unwrap();
        let mut urls = Vec::new();
        collect_image_urls(&body, &mut urls);

        urls.sort();
        urls.dedup();

        assert!(
            urls.len() > 100,
            "the fixture should hold a real spread of image URLs"
        );

        for url in &urls {
            assert!(is_trusted_image_url(url), "{url} should be trusted");
        }

        for directory in [
            "/img/skin/",
            "/img/skinloading/",
            "/img/skinvideo/",
            "/img/center/",
            "/img/guidetop/",
            "/img/sourceImg/",
            "/img/vo/",
            "/img/zmheropage/",
        ] {
            assert!(
                urls.iter().any(|url| url.contains(directory)),
                "{directory} should appear in the fixture"
            );
        }
    }

    #[test]
    fn a_real_video_page_under_a_foreign_host_is_not_an_image() {
        assert!(!is_trusted_image_url(
            "https://v.qq.com/x/page/i356387coc5.html"
        ));
    }

    #[test]
    fn every_real_hero_audio_url_carries_the_trusted_prefix() {
        for path in [
            "/images/lol/act/img/vo/choose/1.ogg",
            "/images/lol/act/img/vo/ban/1.ogg",
        ] {
            assert!(is_trusted_image_url(&format!("{BASE}{path}")));
        }
    }

    #[test]
    fn a_foreign_host_is_rejected() {
        assert!(!is_trusted_image_url(
            "https://evil.example.com/images/lol/act/img/skinloading/x.jpg"
        ));
    }

    #[test]
    fn a_traversal_under_the_trusted_prefix_is_rejected() {
        assert!(!is_trusted_image_url(
            "https://game.gtimg.cn/images/lol/act/img/skinloading/../../secret.jpg"
        ));
    }

    #[test]
    fn a_query_escape_or_fragment_is_rejected() {
        for url in [
            "https://game.gtimg.cn/images/lol/act/img/skinloading/1001.jpg?token=abc",
            "https://game.gtimg.cn/images/lol/act/img/skinloading/1001%2ejpg",
            "https://game.gtimg.cn/images/lol/act/img/skinloading/1001.jpg#frag",
        ] {
            assert!(!is_trusted_image_url(url), "{url} should be rejected");
        }
    }

    #[test]
    fn a_skin_table_without_skins_is_a_parse_error() {
        assert!(matches!(
            parse_hero_skins(r#"{"version":"16.19"}"#),
            Err(CnError::CatalogParse(_))
        ));
    }

    #[test]
    fn a_skin_without_a_numeric_skin_id_is_a_parse_error() {
        let json = r#"{"skins":[{"heroId":1,"name":"x"}],"version":"16.19"}"#;
        assert!(matches!(
            parse_hero_skins(json),
            Err(CnError::CatalogParse(_))
        ));
    }

    #[test]
    fn a_number_where_a_string_is_expected_still_parses() {
        let json = r#"{"skins":[{"skinId":1000,"heroId":1,"name":"x","isBase":1,"loadingImg":"https://game.gtimg.cn/images/lol/act/img/skinloading/x.jpg"}],"version":"16.19"}"#;
        let skin = parse_hero_skins(json).unwrap().skins.remove(0);

        assert_eq!(skin.skin_id, 1000);
        assert!(skin.is_base);
        assert!(skin.loading_img.is_some());
    }

    #[test]
    fn an_empty_option_field_reads_as_absent() {
        let json = r#"{"skins":[{"skinId":1,"heroId":1,"name":"x","chromaImg":"","loadingImg":"","instanceId":""}],"version":"16.19"}"#;
        let skin = parse_hero_skins(json).unwrap().skins.remove(0);

        assert!(!skin.is_chroma());
        assert!(skin.loading_img.is_none());
        assert!(skin.instance_id.is_none());
    }
}
