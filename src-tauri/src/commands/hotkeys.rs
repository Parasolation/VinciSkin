use crate::error::{AppResult, IpcResult};
use crate::hotkeys::{HotkeyAction, HotkeyManager};
use crate::state::{persist_settings, SettingsState};
use vinciskin_core::launcher::wait_for_game_exit;
use std::process::Command;
use tauri::{AppHandle, State};

use super::off_thread;

// ── Hotkey action implementations (called from shortcut callbacks) ──

/// Execute kill-league action.
pub(crate) fn execute_kill_league(_app_handle: &AppHandle) -> AppResult<()> {
    kill_league_process();
    Ok(())
}

// ── IPC commands (called from frontend) ──

/// Temporarily unregister all hotkeys (e.g. while capturing a new binding).
#[tauri::command]
pub fn pause_hotkeys(
    hotkeys: State<HotkeyManager>,
    settings: State<SettingsState>,
) -> IpcResult<()> {
    hotkeys.pause(&settings.0.lock());
    IpcResult::ok(())
}

/// Re-register all hotkeys after capture mode ends.
#[tauri::command]
pub fn resume_hotkeys(
    hotkeys: State<HotkeyManager>,
    settings: State<SettingsState>,
) -> IpcResult<()> {
    hotkeys.resume(&settings.0.lock());
    IpcResult::ok(())
}

/// Set (or clear) a global hotkey for the given action.
#[tauri::command]
pub fn set_hotkey(
    action: HotkeyAction,
    accelerator: Option<String>,
    app_handle: AppHandle,
    hotkeys: State<HotkeyManager>,
    settings: State<SettingsState>,
) -> IpcResult<()> {
    set_hotkey_inner(action, accelerator, &app_handle, &hotkeys, &settings).into()
}

fn set_hotkey_inner(
    action: HotkeyAction,
    accelerator: Option<String>,
    app_handle: &AppHandle,
    hotkeys: &State<HotkeyManager>,
    settings: &State<SettingsState>,
) -> AppResult<()> {
    let mut s = settings.0.lock();
    let old_hotkey = action.get_accelerator(&s).map(str::to_string);

    match accelerator {
        Some(ref accel) if !accel.trim().is_empty() => {
            let trimmed = accel.trim().to_string();
            action.check_no_conflict(&s, &trimmed)?;
            hotkeys.register(action, &trimmed)?;
            if let Some(ref old) = old_hotkey {
                hotkeys.unregister(old);
            }
            action.set_accelerator(&mut s, Some(trimmed));
        }
        _ => {
            if let Some(ref old) = old_hotkey {
                hotkeys.unregister(old);
            }
            action.set_accelerator(&mut s, None);
        }
    }

    persist_settings(app_handle, &s)?;
    Ok(())
}

/// Kill the League of Legends process.
///
/// Runs on a blocking thread rather than inline, because the kill waits for the
/// game to release its archives, which would otherwise hold the IPC handler.
#[tauri::command]
pub async fn kill_league(app_handle: AppHandle) -> IpcResult<()> {
    off_thread(move || execute_kill_league(&app_handle)).await
}

/// How long a killed game gets to release its archives.
const GAME_EXIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Kill the League of Legends game process and wait for it to exit.
fn kill_league_process() {
    tracing::info!("Killing League of Legends process");

    #[cfg(target_os = "windows")]
    let result = Command::new("taskkill")
        .args(["/F", "/IM", "League of Legends.exe"])
        .spawn();

    #[cfg(target_os = "macos")]
    let result = Command::new("pkill")
        .args(["-f", "League of Legends"])
        .spawn();

    #[cfg(target_os = "linux")]
    let result = Command::new("pkill")
        .args(["-f", "League of Legends"])
        .spawn();

    match result {
        Ok(mut child) => {
            let timeout = std::time::Duration::from_secs(3);
            let start = std::time::Instant::now();
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) => {
                        if start.elapsed() > timeout {
                            tracing::warn!("kill command timed out");
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    Err(e) => {
                        tracing::warn!("Error waiting for kill command: {}", e);
                        break;
                    }
                }
            }
        }
        Err(e) => {
            tracing::warn!("Failed to spawn kill command: {}", e);
        }
    }

    if !wait_for_game_exit(GAME_EXIT_TIMEOUT) {
        tracing::warn!(
            "League of Legends still running {}s after the kill",
            GAME_EXIT_TIMEOUT.as_secs()
        );
    }
}
