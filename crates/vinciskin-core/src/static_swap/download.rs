//! Fetching a skin pack from an allowed host, before anything is written into the game.
//!
//! A pack is a file this manager does not own and did not build, so a fetch is bounded twice
//! over: the address is one of a short list of code-hosting hosts, checked on every hop of a
//! redirect, and the body is refused past the size a pack may be. What lands lands inside the
//! directory the caller names, under the name the address ends in, and a fetch that fails
//! takes its own partial file away with it.
//!
//! Nothing here decides whether the bytes are a pack. That is the pack reader's to say, and it
//! says it after this returns, before any archive is touched.

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use fs_err as fs;
use reqwest::Url;
use reqwest::blocking::Client;
use reqwest::redirect::{Attempt, Policy};

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::events::{BackendEvent, CnPackProgress, CnPackStage, EventSink};

use super::PackSwap;

/// The hosts a pack may be fetched from.
///
/// The two GitHub pages are what a reader copies out of a browser, and the two the download
/// actually comes from once GitHub has answered a page with a redirect.
const HOSTS: [&str; 4] = [
    "github.com",
    "raw.githubusercontent.com",
    "objects.githubusercontent.com",
    "codeload.github.com",
];

/// The largest body a fetch will write, matching what a pack may carry.
const MAX_BYTES: u64 = 512 * 1024 * 1024;

/// How many redirects a fetch will follow before it gives up.
const MAX_REDIRECTS: usize = 5;

const TIMEOUT: Duration = Duration::from_secs(300);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// The one fetch this process runs at a time.
///
/// A pack is hundreds of megabytes over a link nobody is watching, so a second press while one
/// is in flight is refused rather than raced.
static IN_FLIGHT: Mutex<()> = Mutex::new(());

/// Fetch the pack at `url` into `scratch` and write it over the install's own archives.
///
/// The fetch is removed once the write is over, whichever way it went, because what a pack is
/// worth keeping is the archives it left in the game rather than the file it arrived in.
///
/// # Errors
///
/// Fails when the address is not one a pack is fetched from, when the fetch fails or is larger
/// than a pack may be, when the bytes are not a pack, and for everything `apply_pack` refuses.
pub fn fetch_and_apply(
    config: &Config,
    storage_dir: &Path,
    url: &str,
    scratch: &Path,
    events: &dyn EventSink,
) -> AppResult<Vec<PackSwap>> {
    events.emit(BackendEvent::CnPackProgress(CnPackProgress {
        stage: CnPackStage::Fetching,
        current_archive: None,
        pack: None,
        current: 0,
        total: 0,
    }));

    let file = fetch(url, scratch)?;
    let applied = super::apply_pack(config, storage_dir, &file, events);
    let _ = fs::remove_file(&file);
    applied
}

/// Fetch `url` into `scratch` and answer the file it wrote.
///
/// # Errors
///
/// Fails when the address is not https on an allowed host, when its last path segment is not a
/// file name, when a fetch is already running, when the request fails or answers with a status
/// nothing can be read from, and when the body is larger than [`MAX_BYTES`].
pub fn fetch(url: &str, scratch: &Path) -> AppResult<PathBuf> {
    let _in_flight = IN_FLIGHT
        .try_lock()
        .map_err(|_| AppError::Other("a skin pack is already being fetched".to_owned()))?;

    let url = Url::parse(url).map_err(|error| refused(url, &error.to_string()))?;
    if url.scheme() != "https" {
        return Err(refused(url.as_str(), "it is not an https address"));
    }
    if !allowed(url.host_str()) {
        return Err(refused(url.as_str(), "it is not a host a pack is fetched from"));
    }

    let name = file_name(&url)?;
    fs::create_dir_all(scratch)?;
    let path = scratch.join(&name);

    let client = Client::builder()
        .user_agent(concat!("vinciskin/", env!("CARGO_PKG_VERSION")))
        .timeout(TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .redirect(Policy::custom(hop))
        .build()
        .map_err(|error| AppError::Other(error.to_string()))?;

    let mut response = client
        .get(url.clone())
        .send()
        .map_err(|error| AppError::Other(error.to_string()))?;
    if !response.status().is_success() {
        return Err(refused(url.as_str(), &format!("it answered {}", response.status())));
    }
    if response.content_length().is_some_and(|size| size > MAX_BYTES) {
        return Err(too_big(url.as_str()));
    }

    let written = write_body(&mut response, &path);
    match written {
        Ok(()) => Ok(path),
        Err(error) => {
            let _ = fs::remove_file(&path);
            Err(error)
        }
    }
}

/// Stream the body into `path`, refusing one past the cap.
fn write_body(response: &mut reqwest::blocking::Response, path: &Path) -> AppResult<()> {
    let mut file = fs::File::create(path)?;
    let mut kept = 0u64;
    let mut buffer = vec![0u8; 1 << 20];

    loop {
        let read = response.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        kept += read as u64;
        if kept > MAX_BYTES {
            return Err(AppError::ValidationFailed(format!(
                "{} is past the {MAX_BYTES} bytes a pack may be",
                path.display()
            )));
        }
        file.write_all(&buffer[..read])?;
    }

    file.flush()?;
    Ok(())
}

/// Whether a redirect to `host` is one a fetch follows.
///
/// A redirect that leaves the allowed hosts is refused here as well as at the address, because
/// a `github.com` page is free to point anywhere and the body is what matters.
fn hop(attempt: Attempt<'_>) -> Action {
    if attempt.previous().len() >= MAX_REDIRECTS {
        return attempt.error("too many redirects");
    }
    match allowed(attempt.url().host_str()) {
        true => attempt.follow(),
        false => attempt.error("that host is not one a pack is fetched from"),
    }
}

/// The `Action` a [`Policy`]'s closure answers with.
type Action = reqwest::redirect::Action;

/// Whether `host` is one of the hosts a pack is fetched from.
fn allowed(host: Option<&str>) -> bool {
    host.is_some_and(|host| {
        let host = host.to_ascii_lowercase();
        HOSTS.contains(&host.as_str())
    })
}

/// The file name an address ends in, which is what the fetch is written as.
///
/// The name travels into a record a page shows, and a pack's name is what decides whether a
/// bare `.wad.client` is read at all, so it is kept as the address spells it and only checked:
/// a segment carrying anything but a file name's own characters names nowhere here. A segment
/// of dots alone is refused as well, because a name that is `..` would put the write outside
/// the directory the caller named.
fn file_name(url: &Url) -> AppResult<String> {
    let name = url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .unwrap_or_default()
        .to_owned();

    let plain = !name.is_empty()
        && !name.bytes().all(|byte| byte == b'.')
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'));
    if !plain {
        return Err(refused(url.as_str(), "it does not end in a file name"));
    }
    Ok(name)
}

fn refused(url: &str, why: &str) -> AppError {
    AppError::ValidationFailed(format!("{url} is not a skin pack source: {why}"))
}

fn too_big(url: &str) -> AppError {
    AppError::ValidationFailed(format!(
        "{url} is larger than the {MAX_BYTES} bytes a pack may be"
    ))
}
