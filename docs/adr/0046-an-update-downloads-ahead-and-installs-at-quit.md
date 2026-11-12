# ADR-0046: An update downloads ahead and installs at quit

- **Status:** Retired (2026-10-09)
- **Date:** 2026-09-21, retired 2026-10-09
- **Crates:** none. The backend was `src-tauri/src/updater.rs`, the frontend `src/modules/updater`
- **Related:** [ADR-0058](0058-the-manager-writes-a-skin-instead-of-patching-one.md), which took the
  patcher out of the launch this one's install waited on.

## Context and problem statement

The app checked for an update once, three seconds after launch, and the tray, the title bar and the
settings page all pointed at it. ADR-0058 then made this build a fork that writes skins into the
install rather than one that follows upstream, and that is what retired this decision.

## Decision (as accepted)

**The backend owned the update.** `check_update`, `download_update`, `install_update` and
`discard_update` held the plugin's `Update` and the verified installer in `UpdaterState`, and the
webview scheduled the checks and drew the dialog.

**A running app checked again** every four hours, and when the window came back into view after a
check older than an hour. **A release downloaded as soon as a check found it**, under the
`autoDownloadUpdates` setting, and **installed when the reader quit**. The tray named the release on
offer, and "start in tray unless update available" revealed the window for one the launch check had
found.

## Why it is retired

**The endpoint is upstream's, and so is the signature.** The check read
`https://github.com/LeagueToolkit/ltk-manager/releases/download/updater/latest.json` and verified
the installer against that project's minisign public key. Both are in `tauri.conf.json`, and neither
can describe a build made here: an update this app could install is an update to the _upstream_
build, which would replace this fork with one that patches.

That is not a defect to fix but the shape of a fork. A build whose whole point is that it does not
inject cannot take its updates from the build that does.

## What went

- `src-tauri/src/updater.rs` and `commands/updater.rs`, with `UpdaterState`, `PendingUpdate`,
  `check`, `download`, `install` and `install_on_quit`.
- The four commands leave `ipc.rs`, and `tauri-plugin-updater` leaves `Cargo.toml` and the plugin
  list.
- `tauri.conf.json` loses `plugins.updater` — the pubkey and the endpoint — and
  `createUpdaterArtifacts`.
- The tray loses its `Update to vX` entry and the quit-time install; `show_update` goes with it.
- The window-event hook that installed on close goes, and the tray's Quit is a plain quit again.
- `src/modules/updater` goes whole, with the title-bar `UpdateButton`, the update dialog, the
  settings page's check row and the two switches that configured it.
- `src-tauri/src/releases` and `commands/releases.rs` go with it: the changelog page existed to fill
  the update dialog, and nothing else read it. `GitHubFeed::Releases` goes too.
- The shell's `Settings` loses `auto_download_updates` and `start_in_tray_unless_update`, and the
  frontend's `Settings` binding loses those two and the already-retired `alwaysStartPatcher`.

## Consequences

- **Updating is by hand.** A newer build is installed by replacing the executable. There is no
  in-app path to one, and none is planned: this build's releases are not published anywhere the app
  knows about.
- A `settings.json` written before this still loads. `Settings` carries no `deny_unknown_fields`, so
  the two retired keys parse as the noise they now are — the same answer ADR-0024 gives a retired
  setting.
- `.github/workflows/release.yml` still maintains the pinned `updater` release and the `latest.json`
  manifest. That workflow belongs to upstream's releases and never runs here; leaving it alone is
  cheaper than rewriting a pipeline this build does not use.
