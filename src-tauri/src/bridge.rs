//! Receives the champ-select pick the client plugin posts over loopback.
//!
//! The client page is HTTPS and its plugin runs inside it. A plain POST with a
//! query token needs no preflight, and `mode: 'no-cors'` means the plugin never
//! reads the answer, so the reply only has to be well formed.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use fs_err as fs;
use parking_lot::Mutex;
use serde::Deserialize;

use crate::lcu::Selection;

/// The port the plugin expects when it is already inside a running client.
///
/// A fixed port keeps the plugin valid across app restarts, so the client does
/// not have to be restarted to pick up a new endpoint. The fallback is an
/// ephemeral port, which does require the client to restart.
const PREFERRED_PORT: u16 = 47_821;

/// How long the accept loop sleeps between polls when nothing has arrived.
const ACCEPT_POLL: Duration = Duration::from_millis(100);

/// The running bridge: the port the plugin posts to, the last pick it sent, and
/// the flag that ends the accept loop.
pub struct Bridge {
    pub port: u16,
    pub token: String,
    pub state: Arc<Mutex<Option<Selection>>>,
    stop: Arc<AtomicBool>,
}

impl Bridge {
    /// Stop accepting. The port is released once the accept loop sees the flag.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// What the plugin posts.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Payload {
    champion_id: u32,
    skin_id: u32,
}

/// Start the bridge, preferring the port a loaded plugin already points at.
///
/// `token_path` holds the token between runs, so the plugin written last time
/// still authenticates when only the app restarted.
pub fn start(token_path: Option<&Path>) -> std::io::Result<Bridge> {
    let listener = TcpListener::bind(("127.0.0.1", PREFERRED_PORT))
        .or_else(|_| TcpListener::bind(("127.0.0.1", 0)))?;
    listener.set_nonblocking(true)?;

    let port = listener.local_addr()?.port();
    let token = load_token(token_path);
    let state = Arc::new(Mutex::new(None));
    let stop = Arc::new(AtomicBool::new(false));

    let shared = Arc::clone(&state);
    let expected = token.clone();
    let running = Arc::clone(&stop);

    std::thread::spawn(move || {
        while !running.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, _)) => {
                    let shared = Arc::clone(&shared);
                    let expected = expected.clone();
                    std::thread::spawn(move || {
                        if stream.set_nonblocking(false).is_ok() {
                            let _ = serve(stream, &expected, &shared);
                        }
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(ACCEPT_POLL);
                }
                Err(_) => break,
            }
        }
    });

    Ok(Bridge {
        port,
        token,
        state,
        stop,
    })
}

/// The token from a previous run, or a fresh one written for the next.
fn load_token(path: Option<&Path>) -> String {
    let Some(path) = path else {
        return token();
    };

    if let Ok(held) = fs::read_to_string(path) {
        let held = held.trim();
        if held.len() >= 16 {
            return held.to_owned();
        }
    }

    let fresh = token();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, &fresh);
    fresh
}

/// One request: the pick arrives as a small JSON body.
fn serve(
    mut stream: TcpStream,
    token: &str,
    state: &Arc<Mutex<Option<Selection>>>,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;

    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];

    let (head, body) = loop {
        let read = stream.read(&mut chunk)?;
        if read == 0 {
            break (None, Vec::new());
        }
        buffer.extend_from_slice(&chunk[..read]);

        if let Some(end) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buffer[..end]).into_owned();
            let length = content_length(&head);
            if buffer.len() >= end + 4 + length {
                break (Some(head), buffer[end + 4..end + 4 + length].to_vec());
            }
        }

        if buffer.len() > 64 * 1024 {
            break (None, Vec::new());
        }
    };

    let Some(head) = head else {
        return respond(&mut stream, "400 Bad Request");
    };

    let request = head.lines().next().unwrap_or_default();
    let authorised = request
        .split_whitespace()
        .nth(1)
        .and_then(|target| target.split_once("token="))
        .is_some_and(|(_, held)| held == token);

    if !request.starts_with("POST ") || !authorised {
        return respond(&mut stream, "403 Forbidden");
    }

    if let Ok(payload) = serde_json::from_slice::<Payload>(&body) {
        *state.lock() = Some(Selection {
            champion_id: payload.champion_id,
            skin_id: payload.skin_id,
        });
    }

    respond(&mut stream, "204 No Content")
}

/// The declared body length, or zero when the request carries none.
fn content_length(head: &str) -> usize {
    head.lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse().ok())
        .unwrap_or(0)
}

/// A minimal response: the plugin does not read it, only the status matters.
fn respond(stream: &mut TcpStream, status: &str) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
}

/// A token the plugin carries for this run only.
fn token() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    format!("{:032x}", nanos ^ (std::process::id() as u128))
}
