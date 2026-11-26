//! Applies the manager's configured skin onto whichever skin slot the client's
//! champ-select currently has equipped (default skin, owned skin, or chroma).
//!
//! By default (`swap_all_champions == false`), only the local player's currently
//! picked champion is kept in the overlay directory so other players' champions
//! are not swapped. When `swap_all_champions == true`, all configured champions
//! remain active in the overlay simultaneously.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::error::{AppError, AppResult};
use crate::lcu;
use crate::mods::ModLibraryState;
use crate::state::SettingsState;
use vinciskin_core::cn_catalog::{fetch_heroes, GtimgCdn, HeroCatalog};
use vinciskin_core::static_swap::{
    configured_skin, get_swap_all_champions, overlay_apply_to_slot, overlay_keep_only,
};

/// How long between session reads. The client answers in milliseconds; this is
/// the floor, and what keeps a pick that changes every second from queueing
/// more than one write at a time.
const POLL: Duration = Duration::from_secs(2);

/// The client's user agent, as the CDN's own commands send it.
const USER_AGENT: &str = concat!("vinciskin/", env!("CARGO_PKG_VERSION"));

/// One auto-applied skin, as the page reports it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoApplied {
    /// The champion whose overlay was rebuilt.
    pub champion_id: u32,
    /// The desired skin that was applied onto the equipped slot.
    pub skin_id: u32,
    /// The champion archive the overlay was written for, as the page names it.
    pub character: String,
}

/// What the watcher is doing now.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoStatus {
    /// Whether a watcher thread is running.
    pub running: bool,
    /// The champion the client's champ-select shows, so the page can follow it.
    pub current_champion: Option<u32>,
    /// The last skin it applied, while one is still what the client shows.
    pub applied: Option<AutoApplied>,
    /// The last failure, when there was one.
    pub error: Option<String>,
}

/// The managed watcher, held by Tauri for the commands.
#[derive(Default)]
pub struct AutoSwapState {
    running: Mutex<Option<Arc<AtomicBool>>>,
    status: Arc<Mutex<AutoStatus>>,
}

impl AutoSwapState {
    /// Start following the client's champ-select session.
    pub fn start(&self, app_handle: &AppHandle) -> AppResult<()> {
        let token = {
            let mut current = self.running.lock();
            if current
                .as_ref()
                .is_some_and(|token| token.load(Ordering::SeqCst))
            {
                return Ok(());
            }

            let token = Arc::new(AtomicBool::new(true));
            *current = Some(Arc::clone(&token));
            token
        };

        let running = Arc::clone(&token);
        let status = Arc::clone(&self.status);
        let handle = app_handle.clone();

        std::thread::spawn(move || follow(&handle, running, status));
        Ok(())
    }

    /// Stop following the session.
    pub fn stop(&self) {
        if let Some(token) = self.running.lock().take() {
            token.store(false, Ordering::SeqCst);
        }
    }

    /// What the watcher is doing now.
    pub fn status(&self) -> AutoStatus {
        let mut status = self.status.lock();
        status.running = self
            .running
            .lock()
            .as_ref()
            .is_some_and(|token| token.load(Ordering::SeqCst));
        status.clone()
    }
}

/// The watcher's loop: read the session, and rebuild the overlay on a change.
fn follow(
    app_handle: &AppHandle,
    running: Arc<AtomicBool>,
    status: Arc<Mutex<AutoStatus>>,
) {
    tracing::info!("auto-swap watcher started");

    let mut last: Option<(u32, u32, Option<u32>, bool)> = None;

    while running.load(Ordering::SeqCst) {
        match tick(app_handle, &mut last) {
            Ok((current, applied)) => set(&status, |s| {
                s.error = None;
                s.current_champion = current;
                s.applied = applied.or_else(|| s.applied.take());
            }),
            Err(error) => {
                tracing::debug!("auto-swap tick: {error}");
                set(&status, |s| s.error = Some(error.to_string()));
            }
        }

        std::thread::sleep(POLL);
    }

    tracing::info!("auto-swap watcher stopped");
}

/// One session read: detect the currently equipped skin in champ-select,
/// apply the configured skin onto that slot, and prune other champions when
/// `swap_all_champions` is disabled.
fn tick(
    app_handle: &AppHandle,
    last: &mut Option<(u32, u32, Option<u32>, bool)>,
) -> AppResult<(Option<u32>, Option<AutoApplied>)> {
    let settings = app_handle.state::<SettingsState>();
    let config = settings.config();

    let Some(endpoint) = lcu::endpoint(&config) else {
        // No client is up, so the last pick is out of date for the next session.
        *last = None;
        crate::cn_overlay::sync(app_handle, None);
        return Ok((None, None));
    };

    let phase = lcu::gameflow_phase(endpoint.clone());
    let selection = lcu::selection(endpoint);

    let in_game = matches!(
        phase.as_deref(),
        Some("GameStart" | "InProgress" | "Reconnect")
    );
    let overlay_champion = if in_game {
        None
    } else {
        selection
            .as_ref()
            .filter(|s| s.champion_id != 0)
            .map(|s| s.champion_id)
    };
    crate::cn_overlay::sync(app_handle, overlay_champion);

    let Some(selection) = selection else {
        // No champ-select session is open.
        *last = None;
        return Ok((None, None));
    };

    let current = Some(selection.champion_id);
    if selection.champion_id == 0 || selection.skin_id == 0 {
        return Ok((current, None));
    }

    let character = character_for(app_handle, selection.champion_id)?;
    let library = app_handle.state::<ModLibraryState>();
    let storage = library.inner().0.storage_dir(&config)?;

    let swap_all = get_swap_all_champions(&storage)?;
    let desired_skin = configured_skin(&storage, &character)?;

    let key = (selection.champion_id, selection.skin_id, desired_skin, swap_all);
    if *last == Some(key) {
        return Ok((current, None));
    }

    let Some(desired_skin) = desired_skin else {
        // Current champion has no configured skin: in single-champion mode,
        // clear any leftover overlay files from previously played champions.
        if !swap_all {
            overlay_keep_only(&storage, None)?;
        }
        *last = Some(key);
        return Ok((current, None));
    };

    let desired_skin_id = selection
        .champion_id
        .saturating_mul(1000)
        .saturating_add(desired_skin);

    overlay_apply_to_slot(
        &config,
        &storage,
        &character,
        desired_skin_id,
        selection.champion_id,
        selection.skin_id,
    )?;

    if !swap_all {
        overlay_keep_only(&storage, Some(&character))?;
    }

    *last = Some(key);

    let applied = AutoApplied {
        champion_id: selection.champion_id,
        skin_id: desired_skin_id,
        character,
    };
    let _ = app_handle.emit("cn-auto-swap-applied", &applied);
    tracing::info!(
        champion = applied.champion_id,
        desired_skin = applied.skin_id,
        equipped_slot = selection.skin_id,
        swap_all,
        "auto-applied configured skin onto equipped slot"
    );
    Ok((current, Some(applied)))
}

/// The champion archive a champion id is named after, from the CDN's own table.
fn character_for(app_handle: &AppHandle, champion_id: u32) -> AppResult<String> {
    let catalog = app_handle.state::<AutoCatalog>();
    let mut heroes = catalog.0.lock();

    if heroes.is_none() {
        let cdn = GtimgCdn::new(USER_AGENT).map_err(|error| AppError::Other(error.to_string()))?;
        *heroes = Some(fetch_heroes(&cdn).map_err(|error| AppError::Other(error.to_string()))?);
    }

    let hero = heroes
        .as_ref()
        .and_then(|catalog| catalog.heroes.iter().find(|hero| hero.id == champion_id))
        .ok_or_else(|| {
            AppError::ValidationFailed(format!("no hero in the table is id {champion_id}"))
        })?;

    Ok(hero.alias.clone())
}

/// The hero table, cached for as long as the app runs.
#[derive(Default)]
pub struct AutoCatalog(pub Mutex<Option<HeroCatalog>>);

/// Run `change` against the shared status.
fn set(status: &Arc<Mutex<AutoStatus>>, change: impl FnOnce(&mut AutoStatus)) {
    change(&mut status.lock());
}
