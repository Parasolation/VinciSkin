//! The live Tencent CDN, read end to end.
//!
//! Ignored by default. It needs the network, and a CDN that drifted between
//! patches would fail a suite with nothing to do with the drift. Run it with
//! `cargo test -p vinciskin-core --test cn_catalog_live -- --ignored`.
//!
//! What it covers that the unit tests cannot: the bytes the CDN actually serves
//! today, against the shapes `vinciskin_cn` parses. The fixtures under
//! `vinciskin-cn/tests/fixtures` are one capture of that, so this is what notices a
//! rename the capture predates.

use vinciskin_core::cn_catalog::{
    GtimgCdn, fetch_hero_skins, fetch_heroes, is_trusted_image_url,
};

/// The CDN is not ours, so the request says who is asking.
const USER_AGENT: &str = concat!("vinciskin/", env!("CARGO_PKG_VERSION"));

#[test]
#[ignore = "reads the live Tencent CDN"]
fn the_live_hero_table_parses_and_names_a_patch() {
    let cdn = GtimgCdn::new(USER_AGENT).expect("the HTTP client builds");

    let catalog = fetch_heroes(&cdn).expect("the live hero table parses");

    assert!(
        catalog.heroes.len() > 100,
        "expected a full roster, got {} champions",
        catalog.heroes.len()
    );
    assert!(catalog.version.major > 0, "the table names no patch");
    assert!(!catalog.heroes[0].alias.is_empty());
}

#[test]
#[ignore = "reads the live Tencent CDN"]
fn the_live_skin_table_of_one_champion_parses() {
    let cdn = GtimgCdn::new(USER_AGENT).expect("the HTTP client builds");

    let roster = fetch_heroes(&cdn).expect("the live hero table parses");
    let champion = roster.heroes.first().expect("the roster is not empty");

    let skins = fetch_hero_skins(&cdn, champion.id).expect("the live skin table parses");

    assert!(!skins.skins.is_empty(), "{} has no skins", champion.alias);
    assert!(
        skins.skins.iter().any(|skin| skin.is_base),
        "{} has no base skin",
        champion.alias
    );
}

/// The art the screen draws, against the CDN that serves it.
///
/// This is the check that would have caught the field `chromaImg` being wrong:
/// it points at `skinloading/1/<uuid>.png`, which answers 404, so the URL is
/// built from the instance id instead. A chroma whose art stopped resolving is
/// a row of broken images on the screen, which no fixture can show.
#[test]
#[ignore = "reads the live Tencent CDN"]
fn the_art_urls_the_screen_draws_answer() {
    let cdn = GtimgCdn::new(USER_AGENT).expect("the HTTP client builds");
    let roster = fetch_heroes(&cdn).expect("the live hero table parses");
    let champion = roster.heroes.first().expect("the roster is not empty");
    let catalogue = fetch_hero_skins(&cdn, champion.id).expect("the live skin table parses");

    let client = reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .expect("the HTTP client builds");

    let chromas = catalogue
        .skins
        .iter()
        .filter(|skin| skin.is_chroma())
        .take(5)
        .collect::<Vec<_>>();
    assert!(!chromas.is_empty(), "{} lists no chromas", champion.alias);

    for chroma in chromas {
        let url = chroma.art_url().expect("a chroma has art");
        assert!(is_trusted_image_url(&url), "{url} is not on the CDN");

        let status = client
            .head(&url)
            .send()
            .expect("the art request answers")
            .status();
        assert!(status.is_success(), "{} answered {status}", url);
    }
}
