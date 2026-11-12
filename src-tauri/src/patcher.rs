//! Drives the cslol injection loader (`cslol-inj.exe`).
//!
//! The Chinese client's game runs at high integrity, and its module layout
//! breaks the bundled ltk patcher's integrity hook (`.rdata section not found`),
//! which aborts that DLL's init before its overlay hook is installed. cslol's
//! legacy loader has no such hook: it hooks `CreateFileA` and redirects the
//! game's WAD reads into the overlay, so it is the loader used here.
//!
//! It is launched elevated (`runas`), because a process at a lower integrity
//! level cannot hook the game (`Win32=5`). The process handle is kept so the
//! session can be stopped again, which a lower-integrity process could not do
//! with a fresh handle.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use fs_err as fs;
use parking_lot::Mutex;
use serde::Serialize;

use crate::error::{AppError, AppResult};

/// The bundled loader host.
pub const LOADER_EXE_NAME: &str = "cslol-inj.exe";
/// The bundled loader DLL, loaded from beside the host.
pub const LOADER_DLL_NAME: &str = "cslol-dll-legacy.dll";

/// The loader pair, embedded so a release build is one file.
const EMBEDDED_LOADER_EXE: &[u8] = include_bytes!("../resources/cslol-inj.exe");
const EMBEDDED_LOADER_DLL: &[u8] = include_bytes!("../resources/cslol-dll-legacy.dll");

/// Write the embedded loader into `dir`, answering the host's path.
///
/// A release build carries no `resources/` beside it, so the pair is unpacked
/// here on first use. Both files land together, because the host loads its DLL
/// from the directory it runs in.
pub fn unpack_loader(dir: &Path) -> AppResult<PathBuf> {
    fs::create_dir_all(dir)?;
    let host = dir.join(LOADER_EXE_NAME);

    write_if_changed(&host, EMBEDDED_LOADER_EXE)?;
    write_if_changed(&dir.join(LOADER_DLL_NAME), EMBEDDED_LOADER_DLL)?;

    Ok(host)
}

/// Write `bytes` to `path`, unless it already holds them.
fn write_if_changed(path: &Path, bytes: &[u8]) -> AppResult<()> {
    if fs::read(path).is_ok_and(|held| held == bytes) {
        return Ok(());
    }
    fs::write(path, bytes)?;
    Ok(())
}

/// The state the patcher is in.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatcherStatus {
    /// Whether the loader process is alive.
    pub running: bool,
    /// What the session is doing, when there is one.
    pub state: Option<String>,
    /// The last failure, when there was one.
    pub error: Option<String>,
}

/// A process handle, kept so the session can be stopped again.
#[derive(Clone, Copy)]
struct Handle(isize);

// SAFETY: the value is an opaque Windows handle, only ever used through the
// Win32 calls that take it.
unsafe impl Send for Handle {}

#[cfg(windows)]
mod elevated {
    use super::{AppError, AppResult, Handle};
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, TerminateProcess};

    /// The exit code a process still running reports (`STILL_ACTIVE`).
    const STILL_ACTIVE: u32 = 259;
    use windows_sys::Win32::UI::Shell::{
        ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
    };

    fn wide(text: &str) -> Vec<u16> {
        std::ffi::OsStr::new(text)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    /// Launch `exe` with `args` through the `runas` verb, answering its handle.
    pub fn spawn_runas(exe: &str, args: &str) -> AppResult<Handle> {
        let verb = wide("runas");
        let file = wide(exe);
        let params = wide(args);

        // SAFETY: every pointer below outlives the call, and the struct is
        // zeroed then filled the way `ShellExecuteExW` documents.
        let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        /* `SW_HIDE`: the loader is a console program, so a normal show opens a
        black window over the game. Its console is never read — the session's
        state comes from the game's own process, not the loader's stdout — so
        hiding the window costs nothing. */
        info.nShow = 0;

        // SAFETY: `info` is a fully initialized SHELLEXECUTEINFOW.
        let started = unsafe { ShellExecuteExW(&mut info) };
        if started == 0 {
            return Err(AppError::Other(format!(
                "could not start the loader elevated: {}",
                std::io::Error::last_os_error()
            )));
        }
        if info.hProcess.is_null() {
            return Err(AppError::Other(
                "the loader started but gave no process handle".into(),
            ));
        }
        Ok(Handle(info.hProcess as isize))
    }

    /// Whether the process behind `handle` is still running.
    pub fn alive(handle: Handle) -> bool {
        let mut code: u32 = 0;
        // SAFETY: `handle` is a live process handle.
        let ok = unsafe { GetExitCodeProcess(handle.0 as HANDLE, &mut code) };
        ok != 0 && code == STILL_ACTIVE as u32
    }

    /// Stop the process behind `handle`.
    pub fn kill(handle: Handle) {
        // SAFETY: `handle` is a live process handle with terminate access, which
        // is what `ShellExecuteExW` hands back.
        unsafe {
            TerminateProcess(handle.0 as HANDLE, 0);
            CloseHandle(handle.0 as HANDLE);
        }
    }
}

#[cfg(not(windows))]
mod elevated {
    use super::{AppError, AppResult, Handle};

    pub fn spawn_runas(_exe: &str, _args: &str) -> AppResult<Handle> {
        Err(AppError::Other(
            "the injection loader is Windows only".into(),
        ))
    }
    pub fn alive(_handle: Handle) -> bool {
        false
    }
    pub fn kill(_handle: Handle) {}
}

/// The game the loader waits for.
#[cfg(windows)]
mod probe {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    /// The game the loader hooks, as the process list names it.
    const GAME: &str = "League of Legends.exe";

    /// The running game's process id, which is what tells a session from a scan.
    pub fn game_pid() -> Option<u32> {
        find(GAME)
    }

    /// A loader process, whether or not this app started it.
    ///
    /// A loader left behind by a crashed or replaced app still injects, so a
    /// second one beside it would hook the game twice.
    pub fn loader_pid() -> Option<u32> {
        find(super::LOADER_EXE_NAME)
    }

    /// The first process whose executable name is `name`.
    fn find(name: &str) -> Option<u32> {
        // SAFETY: the snapshot is closed before returning, and the entry is
        // sized the way the API documents.
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return None;
            }

            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

            let mut found = None;
            if Process32FirstW(snapshot, &mut entry) != 0 {
                loop {
                    let listed: String = entry
                        .szExeFile
                        .iter()
                        .take_while(|unit| **unit != 0)
                        .map(|unit| char::from_u32(u32::from(*unit)).unwrap_or('\u{fffd}'))
                        .collect();
                    if listed.eq_ignore_ascii_case(name) {
                        found = Some(entry.th32ProcessID);
                        break;
                    }
                    if Process32NextW(snapshot, &mut entry) == 0 {
                        break;
                    }
                }
            }

            CloseHandle(snapshot);
            found
        }
    }
}

#[cfg(not(windows))]
mod probe {
    pub fn game_pid() -> Option<u32> {
        None
    }

    pub fn loader_pid() -> Option<u32> {
        None
    }
}

struct Inner {
    handle: Option<Handle>,
    status: PatcherStatus,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            handle: None,
            status: PatcherStatus::default(),
        }
    }
}

/// The managed patcher state, held by Tauri for the commands.
#[derive(Default)]
pub struct PatcherState {
    inner: Arc<Mutex<Inner>>,
}

impl PatcherState {
    /// What the patcher is doing now, reaping a loader that has exited.
    pub fn status(&self) -> PatcherStatus {
        let mut inner = self.inner.lock();

        if let Some(handle) = inner.handle {
            if elevated::alive(handle) {
                /* The loader has no stdio of its own, so the game's own process
                is what tells a session from a scan. */
                inner.status.state = Some(
                    if probe::game_pid().is_some() {
                        "injected"
                    } else {
                        "scanning"
                    }
                    .to_string(),
                );
            } else {
                inner.handle = None;
                inner.status = PatcherStatus::default();
            }
        }

        inner.status.clone()
    }

    /// Stop the loader.
    pub fn stop(&self) -> AppResult<()> {
        let mut inner = self.inner.lock();
        let Some(handle) = inner.handle else {
            return Err(AppError::Other("the patcher is not running".into()));
        };

        /* The loader cannot unload what it injected, and a running game is one
        it either hooked or is about to, so ending it mid-game leaves the hook
        in that process until the game closes. */
        if probe::game_pid().is_some() {
            return Err(AppError::Other(
                "the game is still running - close it before stopping the loader".into(),
            ));
        }

        inner.handle = None;
        elevated::kill(handle);
        inner.status = PatcherStatus::default();
        Ok(())
    }

    /// Start the loader over `overlay`, elevated.
    pub fn start(&self, loader_exe: &Path, overlay: &Path) -> AppResult<()> {
        let mut inner = self.inner.lock();
        if inner.handle.is_some() {
            return Err(AppError::Other("the patcher is already running".into()));
        }

        /* A loader this app does not hold a handle on still hooks whatever game
        it finds, so one left by an earlier run has to be cleared first. */
        if probe::loader_pid().is_some() {
            return Err(AppError::Other(format!(
                "{} is already running - end it before starting another",
                LOADER_EXE_NAME
            )));
        }

        /* A game that is already up has opened its WADs, so a late injection
        either misses them or stacks a second hook in a process nothing ever
        unloads one from. The loader therefore only starts ahead of the game. */
        if probe::game_pid().is_some() {
            return Err(AppError::Other(
                "the game is already running - start the loader before launching it".into(),
            ));
        }

        tracing::info!(
            loader = %loader_exe.display(),
            overlay = %overlay.display(),
            "starting the injection loader (elevated)"
        );

        let handle = elevated::spawn_runas(
            &loader_exe.to_string_lossy(),
            &overlay.to_string_lossy(),
        )?;

        inner.status = PatcherStatus {
            running: true,
            state: Some("scanning".to_string()),
            error: None,
        };
        inner.handle = Some(handle);
        Ok(())
    }
}
