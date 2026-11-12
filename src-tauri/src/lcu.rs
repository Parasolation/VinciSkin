//! Reads the running League client's local API (LCU).
//!
//! The client opens an HTTPS endpoint on a random loopback port and writes the
//! port and a one-time password into `LeagueClientUx.lockfile` at its install
//! root for as long as it runs. Reading that file and calling the API is the
//! whole of the discovery: no process is opened, no DLL is injected and nothing
//! outside the client's own documented endpoints is touched.
//!
//! The certificate is self-signed, which is why the client below skips TLS
//! verification. That is safe here because the request never leaves loopback.

use std::path::PathBuf;

use fs_err as fs;
use serde_json::Value;

use vinciskin_core::config::Config;

/// The lockfile names a client writes while it runs, relative to its install
/// root or to the client directory below it.
///
/// Riot's client writes `LeagueClientUx.lockfile` at the root. The Chinese
/// client has been seen with a bare `lockfile` under `LeagueClient/`, so both
/// spellings are tried rather than assuming one layout.
const LOCKFILES: [&str; 3] = [
    "LeagueClientUx.lockfile",
    "lockfile",
    "LeagueClient/lockfile",
];

/// The one champ-select endpoint this module reads, and the only scope it needs.
const SESSION: &str = "/lol-champ-select/v1/session";

/// The endpoint of a running client: the loopback port it answers on and the
/// password the lockfile hands out for this run.
#[derive(Debug, Clone)]
pub struct Endpoint {
    pub port: u16,
    pub password: String,
}

/// One champion's pick, as the client's own session reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// The champion on the local player's slot.
    pub champion_id: u32,
    /// The skin the local player has selected, `0` before one is hovered.
    pub skin_id: u32,
}

/// Every lockfile path the configured League path could name, root first.
///
/// The configured path may be the install root or the `Game` directory inside
/// it, so both are tried.
fn candidates(config: &Config) -> Vec<PathBuf> {
    let Some(path) = config.league_path.as_ref() else {
        return Vec::new();
    };

    let mut roots = vec![path.clone()];
    if let Some(parent) = path.parent() {
        roots.push(parent.to_path_buf());
    }

    let mut out = Vec::new();
    for root in roots {
        for name in LOCKFILES {
            out.push(root.join(name));
        }
    }
    out
}

/// The client's endpoint for this run, or none while the client is not up.
///
/// A lockfile that is missing, unreadable, empty or mid-write all read as "no
/// client", which is what the caller would do with a client that has just
/// closed.
pub fn endpoint(config: &Config) -> Option<Endpoint> {
    for path in candidates(config) {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        if let Some(endpoint) = parse_lockfile(&text) {
            tracing::debug!(lockfile = %path.display(), "using the client lockfile");
            return Some(endpoint);
        }
    }
    None
}

/// The lockfile's fields, of which the port and password are what the API needs.
///
/// Riot writes `name:pid:port:password:protocol`, and the two trailing fields
/// are swapped in some builds. The protocol is the field that names a scheme,
/// so whichever of the pair looks like one is read as the protocol.
fn parse_lockfile(text: &str) -> Option<Endpoint> {
    let mut fields = text.trim().split(':');
    fields.next()?; // the process name, which is not what the API is reached by
    fields.next()?; // the pid
    let port = fields.next()?.parse().ok()?;
    let fourth = fields.next()?;
    let fifth = fields.next()?;

    let password = if is_protocol(fifth) {
        fourth
    } else if is_protocol(fourth) {
        fifth
    } else {
        fourth
    };

    if password.is_empty() {
        return None;
    }

    Some(Endpoint {
        port,
        password: password.to_owned(),
    })
}

/// Whether `field` names the scheme the endpoint speaks.
fn is_protocol(field: &str) -> bool {
    field.eq_ignore_ascii_case("http") || field.eq_ignore_ascii_case("https")
}

/// The local player's champion and skin, read from the live champ-select session.
///
/// `None` while no session is open, which is everything outside a champion pick.
/// The local player is the slot the session names `localPlayerCellId`, found in
/// `myTeam`.
pub fn selection(endpoint: Endpoint) -> Option<Selection> {
    let session = get(endpoint, SESSION)?;
    let local = session.get("localPlayerCellId")?.as_i64()?;

    let player = session
        .get("myTeam")?
        .as_array()?
        .iter()
        .find(|entry| entry.get("cellId").and_then(Value::as_i64) == Some(local))?;

    Some(Selection {
        champion_id: player.get("championId")?.as_u64()? as u32,
        skin_id: player
            .get("selectedSkinId")
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32,
    })
}

/// One GET against the client, answered as parsed JSON.
///
/// The client may be between listens, so a failure to connect reads as no answer
/// rather than an error: the caller polls, and the next poll retries.
fn get(endpoint: Endpoint, path: &str) -> Option<Value> {
    let url = format!("https://127.0.0.1:{}{}", endpoint.port, path);
    let client = reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .ok()?;

    let response = client
        .get(url)
        .basic_auth("riot", Some(&endpoint.password))
        .send()
        .ok()?
        .error_for_status()
        .ok()?;

    serde_json::from_str(&response.text().ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lockfile_parses_its_port_and_password() {
        let endpoint = parse_lockfile("LeagueClientUx:1234:54321:abc123:https").expect("parses");
        assert_eq!(endpoint.port, 54321);
        assert_eq!(endpoint.password, "abc123");
    }

    #[test]
    fn a_lockfile_that_swaps_its_last_two_fields_still_parses() {
        let endpoint = parse_lockfile("LeagueClientUx:1234:54321:https:abc123").expect("parses");
        assert_eq!(endpoint.port, 54321);
        assert_eq!(endpoint.password, "abc123");
    }

    #[test]
    fn a_lockfile_without_a_password_is_no_endpoint() {
        assert!(parse_lockfile("LeagueClientUx:1234:54321").is_none());
    }

    #[test]
    fn a_lockfile_with_an_empty_password_is_no_endpoint() {
        assert!(parse_lockfile("LeagueClientUx:1234:54321::https").is_none());
    }

    #[test]
    fn a_lockfile_with_a_non_numeric_port_is_no_endpoint() {
        assert!(parse_lockfile("LeagueClientUx:1234:not-a-port:https:pw").is_none());
    }
}
