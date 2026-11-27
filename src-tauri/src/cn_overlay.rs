//! Champ-select floating skin picker window, docked beside the League client.
//!
//! Shown automatically when champ select detects a picked champion, positioned
//! next to the League client window on the configured side, and hidden as soon
//! as champ select ends or the game starts.

use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::state::{CnOverlayAlign, CnOverlaySide, SettingsState};

const OVERLAY_LABEL: &str = "cn-overlay";
const OVERLAY_WIDTH: f64 = 360.0;
const OVERLAY_HEIGHT: f64 = 560.0;
const DOCK_GAP_LOGICAL: f64 = 8.0;

#[derive(Default)]
pub struct CnOverlayState {
    inner: Mutex<OverlayTracker>,
}

#[derive(Default)]
struct OverlayTracker {
    visible: bool,
    champion_id: Option<u32>,
    dismissed: bool,
    last_client_rect: Option<(i32, i32, i32, i32)>,
    last_side: Option<CnOverlaySide>,
    last_align: Option<CnOverlayAlign>,
}

/// Synchronize the overlay window visibility, position, and active champion
/// with the current champ-select state.
pub fn sync(app_handle: &AppHandle, champion_id: Option<u32>) {
    let (side, align) = {
        let settings = app_handle.state::<SettingsState>();
        let guard = settings.0.lock();
        (guard.cn_overlay_side, guard.cn_overlay_align)
    };

    let overlay_state = app_handle.state::<CnOverlayState>();
    let mut tracker = overlay_state.inner.lock();

    let Some(champ_id) = champion_id else {
        tracker.champion_id = None;
        tracker.dismissed = false;
        tracker.last_client_rect = None;
        if tracker.visible {
            tracker.visible = false;
            if let Some(window) = app_handle.get_webview_window(OVERLAY_LABEL) {
                let _ = window.hide();
            }
        }
        return;
    };

    if tracker.dismissed {
        return;
    }

    let champion_changed = tracker.champion_id != Some(champ_id);
    tracker.champion_id = Some(champ_id);

    let needs_show = !tracker.visible;
    let client_rect = find_league_client_rect();
    let needs_reposition = needs_show
        || tracker.last_side != Some(side)
        || tracker.last_align != Some(align)
        || (client_rect.is_some() && client_rect != tracker.last_client_rect);

    tracker.visible = true;
    tracker.last_side = Some(side);
    tracker.last_align = Some(align);
    if client_rect.is_some() {
        tracker.last_client_rect = client_rect;
    }
    drop(tracker);

    let handle = app_handle.clone();
    let _ = app_handle.run_on_main_thread(move || {
        let Some(window) = ensure_window(&handle) else {
            return;
        };

        if needs_reposition {
            if let Some(rect) = client_rect {
                dock_beside_rect(&window, rect, side, align);
            }
        }

        if needs_show {
            let _ = window.show();
        }

        if champion_changed || needs_show {
            let _ = window.emit("cn-overlay-champion", champ_id);
        }
    });
}

/// Mark the overlay as dismissed for the remainder of the current champ-select
/// session so it does not immediately pop back up on the next poll.
pub fn dismiss(app_handle: &AppHandle) {
    let overlay_state = app_handle.state::<CnOverlayState>();
    let mut tracker = overlay_state.inner.lock();
    tracker.dismissed = true;
    tracker.visible = false;
    drop(tracker);

    if let Some(window) = app_handle.get_webview_window(OVERLAY_LABEL) {
        let _ = window.hide();
    }
}

/// Re-open the overlay if the user dismissed it during the current champ select.
pub fn reveal(app_handle: &AppHandle) {
    let (side, align) = {
        let settings = app_handle.state::<SettingsState>();
        let guard = settings.0.lock();
        (guard.cn_overlay_side, guard.cn_overlay_align)
    };

    let overlay_state = app_handle.state::<CnOverlayState>();
    let mut tracker = overlay_state.inner.lock();
    tracker.dismissed = false;
    let champ_id = tracker.champion_id;
    let client_rect = find_league_client_rect();
    tracker.visible = true;
    tracker.last_side = Some(side);
    tracker.last_align = Some(align);
    if client_rect.is_some() {
        tracker.last_client_rect = client_rect;
    }
    drop(tracker);

    let handle = app_handle.clone();
    let _ = app_handle.run_on_main_thread(move || {
        let Some(window) = ensure_window(&handle) else {
            return;
        };
        if let Some(rect) = client_rect {
            dock_beside_rect(&window, rect, side, align);
        }
        let _ = window.show();
        let _ = window.set_focus();
        if let Some(id) = champ_id {
            let _ = window.emit("cn-overlay-champion", id);
        }
    });
}

fn ensure_window(app_handle: &AppHandle) -> Option<WebviewWindow> {
    if let Some(existing) = app_handle.get_webview_window(OVERLAY_LABEL) {
        return Some(existing);
    }

    let window = WebviewWindowBuilder::new(
        app_handle,
        OVERLAY_LABEL,
        WebviewUrl::App("cn-overlay".into()),
    )
    .title("VinciSkin")
    .inner_size(OVERLAY_WIDTH, OVERLAY_HEIGHT)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .visible(false)
    .build()
    .map_err(|error| tracing::warn!("failed to build cn-overlay window: {error}"))
    .ok()?;

    let handle = app_handle.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            dismiss(&handle);
        }
    });

    Some(window)
}

fn dock_beside_rect(
    window: &WebviewWindow,
    rect: (i32, i32, i32, i32),
    side: CnOverlaySide,
    align: CnOverlayAlign,
) {
    let (left, top, right, bottom) = rect;
    let center_x = (left + right) / 2;
    let center_y = (top + bottom) / 2;

    let monitor = window
        .available_monitors()
        .ok()
        .and_then(|monitors| {
            monitors.into_iter().find(|m| {
                let pos = m.position();
                let size = m.size();
                center_x >= pos.x
                    && center_x < pos.x + size.width as i32
                    && center_y >= pos.y
                    && center_y < pos.y + size.height as i32
            })
        })
        .or_else(|| window.primary_monitor().ok().flatten());

    let scale = monitor
        .as_ref()
        .map(|m| m.scale_factor())
        .unwrap_or_else(|| window.scale_factor().unwrap_or(1.0));

    let width = (OVERLAY_WIDTH * scale).round() as i32;
    let height = (OVERLAY_HEIGHT * scale).round() as i32;
    let gap = (DOCK_GAP_LOGICAL * scale).round() as i32;

    let (mon_left, mon_top, mon_right, mon_bottom) = match monitor {
        Some(m) => {
            let pos = m.position();
            let size = m.size();
            (
                pos.x,
                pos.y,
                pos.x + size.width as i32,
                pos.y + size.height as i32,
            )
        }
        None => (0, 0, i32::MAX / 2, i32::MAX / 2),
    };

    let right_x = right + gap;
    let left_x = left - gap - width;

    let x = match side {
        CnOverlaySide::Right => {
            if right_x + width <= mon_right {
                right_x
            } else if left_x >= mon_left {
                left_x
            } else {
                (mon_right - width).max(mon_left)
            }
        }
        CnOverlaySide::Left => {
            if left_x >= mon_left {
                left_x
            } else if right_x + width <= mon_right {
                right_x
            } else {
                mon_left
            }
        }
    };

    let max_y = (mon_bottom - height).max(mon_top);
    let target_y = match align {
        CnOverlayAlign::Top => top,
        CnOverlayAlign::Bottom => bottom - height,
    };
    let y = target_y.clamp(mon_top, max_y);

    let _ = window.set_position(PhysicalPosition::new(x, y));
}

#[cfg(windows)]
fn find_league_client_rect() -> Option<(i32, i32, i32, i32)> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowRect, IsWindowVisible};

    fn wide(text: &str) -> Vec<u16> {
        OsStr::new(text)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    let class = wide("RCLIENT");
    let title = wide("League of Legends");
    let mut hwnd = unsafe { FindWindowW(class.as_ptr(), title.as_ptr()) };
    if hwnd.is_null() {
        hwnd = unsafe { FindWindowW(class.as_ptr(), std::ptr::null()) };
    }
    if hwnd.is_null() || unsafe { IsWindowVisible(hwnd) } == 0 {
        return None;
    }

    let mut rect: RECT = unsafe { std::mem::zeroed() };
    if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
        return None;
    }

    if rect.right <= rect.left || rect.bottom <= rect.top {
        return None;
    }

    Some((rect.left, rect.top, rect.right, rect.bottom))
}

#[cfg(not(windows))]
fn find_league_client_rect() -> Option<(i32, i32, i32, i32)> {
    None
}
