use std::cell::RefCell;
use std::collections::HashMap;

use super::*;

/// A CDN holding bodies in a table rather than on a network.
#[derive(Default)]
struct FakeCdn {
    bodies: HashMap<String, String>,
    requested: RefCell<Vec<String>>,
}

impl FakeCdn {
    fn with(mut self, url: String, body: &str) -> Self {
        self.bodies.insert(url, body.to_string());
        self
    }

    fn urls(&self) -> Vec<String> {
        self.requested.borrow().clone()
    }
}

impl CdnTransport for FakeCdn {
    fn get(&self, url: &str) -> Result<String, CnError> {
        self.requested.borrow_mut().push(url.to_string());
        self.bodies
            .get(url)
            .cloned()
            .ok_or_else(|| CnError::Network(format!("no body for {url}")))
    }
}

/// Two heroes, the shape the live table answers with.
const HERO_LIST: &str = r#"{
    "hero": [
        {"heroId": "1", "name": "黑暗之女", "alias": "Annie"},
        {"heroId": "2", "name": "狂战士", "alias": "Olaf"}
    ],
    "version": "16.19",
    "fileName": "hero_list.js"
}"#;

/// One base skin and one chroma, the shape a per-hero table answers with.
const ANNIE_SKINS: &str = r#"{
    "hero": {"heroId": "1", "alias": "Annie", "name": "黑暗之女"},
    "skins": [
        {
            "skinId": "1000",
            "heroId": "1",
            "name": "黑暗之女",
            "isBase": true,
            "chromas": [],
            "chromaImg": "",
            "loadingImg": "https://game.gtimg.cn/images/lol/act/img/champion/Annie.png",
            "instanceId": "0b95894e-0df2-470e-b282-6c5f5cf41955"
        },
        {
            "skinId": "1001",
            "heroId": "1",
            "name": "哥特萝莉",
            "isBase": false,
            "chromaImg": "https://game.gtimg.cn/images/lol/act/img/skinloading/1/a.png",
            "loadingImg": "",
            "instanceId": "11111111-2222-3333-4444-555555555555"
        }
    ],
    "version": "16.19"
}"#;

#[test]
fn the_hero_table_comes_from_the_one_hero_list_url() {
    let cdn = FakeCdn::default().with(HERO_LIST_URL.to_string(), HERO_LIST);

    let catalog = fetch_heroes(&cdn).unwrap();

    assert_eq!(cdn.urls(), vec![HERO_LIST_URL.to_string()]);
    assert_eq!(catalog.heroes.len(), 2);
    assert_eq!(catalog.heroes[0].id, 1);
    assert_eq!(catalog.heroes[0].alias, "Annie");
    assert_eq!(catalog.heroes[0].name, "黑暗之女");
    assert_eq!((catalog.version.major, catalog.version.minor), (16, 19));
}

#[test]
fn a_skin_table_is_read_for_the_hero_that_was_asked_for() {
    let cdn = FakeCdn::default().with(hero_skin_url(1), ANNIE_SKINS);

    let catalog = fetch_hero_skins(&cdn, 1).unwrap();

    assert_eq!(
        cdn.urls(),
        vec!["https://game.gtimg.cn/images/lol/act/img/js/hero/1.js"]
    );
    assert_eq!(catalog.skins.len(), 2);
    assert!(catalog.skins[0].is_base);
    assert!(!catalog.skins[1].is_base);
}

#[test]
fn the_chroma_in_a_skin_table_is_told_apart_from_the_full_skin() {
    let cdn = FakeCdn::default().with(hero_skin_url(1), ANNIE_SKINS);

    let catalog = fetch_hero_skins(&cdn, 1).unwrap();

    assert!(!catalog.skins[0].is_chroma());
    assert!(catalog.skins[1].is_chroma());
}

#[test]
fn the_hero_id_in_the_url_is_the_one_the_caller_named() {
    let cdn = FakeCdn::default().with(hero_skin_url(64), ANNIE_SKINS);

    fetch_hero_skins(&cdn, 64).unwrap();

    assert_eq!(
        cdn.urls(),
        vec!["https://game.gtimg.cn/images/lol/act/img/js/hero/64.js"]
    );
}

#[test]
fn a_body_the_transport_cannot_read_surfaces_as_a_network_failure() {
    let error = fetch_heroes(&FakeCdn::default()).unwrap_err();

    assert!(matches!(error, CnError::Network(_)));
}

#[test]
fn a_body_that_is_not_the_hero_table_surfaces_as_a_parse_failure() {
    let cdn = FakeCdn::default().with(HERO_LIST_URL.to_string(), r#"{"version": "16.19"}"#);

    let error = fetch_heroes(&cdn).unwrap_err();

    assert!(matches!(error, CnError::CatalogParse(_)));
}

#[test]
fn a_patch_label_leaves_out_the_revision() {
    let version = PatchVersion::parse("16.19.1").unwrap();

    assert_eq!(patch_label(version), "16.19");
}
