//! The Chinese client's hero and skin tables, read from the Tencent CDN.
//!
//! `vinciskin_cn` owns the URLs and the body shapes; this module is only the transport
//! that carries them over HTTP, so a catalog read can be exercised without a
//! network. Nothing here is written to disk - the tables are small and a caller
//! that wants them again can ask the CDN again.

use std::time::Duration;

use vinciskin_cn::catalog::gtimg::{HERO_LIST_URL, hero_skin_url, parse_hero_list, parse_hero_skins};

pub use vinciskin_cn::catalog::gtimg::{hero_portrait_url, is_trusted_image_url};
pub use vinciskin_cn::catalog::{Hero, HeroCatalog, Quality, Skin, SkinCatalog};
pub use vinciskin_cn::error::CnError;
pub use vinciskin_cn::version::PatchVersion;

/// Whole-request budget. The hero table is about 112 KB and one hero's skin
/// table about 60 KB, so this is generous for one body and still bounded.
const FETCH_TIMEOUT: Duration = Duration::from_secs(30);

/// Budget for reaching the CDN at all.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// Reads a catalog body from the CDN.
///
/// The seam a test replaces with a table of bodies. A real implementation is
/// [`GtimgCdn`].
pub trait CdnTransport {
    /// The body at `url`, or why it could not be read.
    ///
    /// # Errors
    ///
    /// [`CnError::Network`] when the request fails, times out, or answers with a
    /// status that is not success.
    fn get(&self, url: &str) -> Result<String, CnError>;
}

/// The live Tencent CDN.
#[derive(Debug)]
pub struct GtimgCdn {
    client: reqwest::blocking::Client,
}

impl GtimgCdn {
    /// Talk to the CDN as `user_agent`.
    ///
    /// # Errors
    ///
    /// Fails when the HTTP client cannot be built, which is a broken TLS backend
    /// rather than anything a user can fix.
    pub fn new(user_agent: &str) -> Result<Self, reqwest::Error> {
        Ok(Self {
            client: reqwest::blocking::Client::builder()
                .user_agent(user_agent)
                .timeout(FETCH_TIMEOUT)
                .connect_timeout(CONNECT_TIMEOUT)
                .build()?,
        })
    }
}

impl CdnTransport for GtimgCdn {
    fn get(&self, url: &str) -> Result<String, CnError> {
        self.client
            .get(url)
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .and_then(reqwest::blocking::Response::text)
            .map_err(|error| CnError::Network(error.to_string()))
    }
}

/// Every hero the Chinese client ships, with the version its table reports.
///
/// # Errors
///
/// Fails when the transport cannot read the table, or when its body does not
/// match the shape [`parse_hero_list`] expects.
pub fn fetch_heroes<T: CdnTransport>(cdn: &T) -> Result<HeroCatalog, CnError> {
    let body = cdn.get(HERO_LIST_URL)?;
    parse_hero_list(&body)
}

/// One hero's skins, chromas included, with the version its table reports.
///
/// The CDN serves one table per hero and no combined list, so a whole roster is
/// one request per hero. Reading a hero at a time is what keeps opening the
/// screen to a single request.
///
/// # Errors
///
/// Fails when the transport cannot read the table, or when its body does not
/// match the shape [`parse_hero_skins`] expects.
pub fn fetch_hero_skins<T: CdnTransport>(cdn: &T, hero_id: u32) -> Result<SkinCatalog, CnError> {
    let body = cdn.get(&hero_skin_url(hero_id))?;
    parse_hero_skins(&body)
}

/// A patch as a player names it, `16.19`.
///
/// The tables carry no revision, and a revision is not part of what a patch is
/// called, so the third component the version type prints is left out.
pub fn patch_label(version: PatchVersion) -> String {
    format!("{}.{}", version.major, version.minor)
}

#[cfg(test)]
mod tests;
