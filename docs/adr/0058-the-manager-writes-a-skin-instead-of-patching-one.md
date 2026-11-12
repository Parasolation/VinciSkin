# ADR-0058: The manager writes a skin instead of patching one

- **Status:** Superseded (2026-10-09) by the injection route: the manager builds an overlay
  under `<storage>/overlay` and the bundled `ltk_patcher_host.exe` injects
  `ltk_patcher_dll.dll`, which redirects the game's WAD reads. Nothing is written into the
  install. The static write this ADR chose is what broke the Chinese client's launch.
- **Date:** 2026-09-30
- **Crates:** the frontend, in `src/modules/{launcher,library,settings}`; `ltk-manager-core` and the
  Tauri shell for the retired Rust side
- **Related:** [ADR-0056](0056-a-skin-is-swapped-by-writing-the-champions-archive.md), the write this
  keeps. [ADR-0057](0057-the-manager-keeps-three-pages.md), the pages that went first.
  [ADR-0043](0043-a-built-in-mod-is-a-generated-project-above-every-mod.md), the built-in mods this
  removes.

## Context and problem statement

The manager's route to showing a champion a skin was the overlay: mods are merged into the game's
archives at build time, an injected DLL hands the game the built archive, and the launch waits for
that build before asking the Riot Client to start League. ADR-0056 replaced the effect with a write
into the game's own archive, and ADR-0057 took away the pages that reached the rest of it.

What was left after those two: a launch that still built an overlay nobody could turn anything on
in, four built-in mods in Settings that configured chunks no build would read, and a Play gate that
counted them. Worse, `alwaysStartPatcher` could still start the whole thing on launch — an
injection into a client whose anti-cheat is the reason this build exists.

## Decision

**The manager does not patch. A launch is a plain launch, and a skin is written into the install.**

The frontend half:

- `usePlay` exposes `launchOnly` alone. The `play` action — build the overlay, wait for the patcher,
  then launch — is gone, with the pending-rebuild handling and the patcher-status polling it needed.
- The four built-in mods go: `defaultWardSkins`, `baseSkins`, `mapSkin`/`forcedMapSkin` and
  `mapDecorations`, with their Settings card, their tab, their five index rows, their message
  catalog entries and the pickers that listed what the install could offer.
- `useGuardedStartPatcher` goes with `play`, which was its last caller. The skinhack pre-flight it
  wrapped was a mod-library concern, and nothing reaches it now.

The backend half, taken out in the same release:

- `ltk-manager-core`'s `patcher` module goes whole — the session thread, the injector, the host and
  its line protocol, the pipeline, the recorder and the elevation bridge. `PatcherError` goes with
  it, and `AppError` loses its `PATCHER` code.
- The shell's `patcher` module and `commands/patcher.rs` go, with `PatcherState`, `PatcherHostState`
  and the guard every mod write took off it. `start_patcher`, `stop_patcher`, `rebuild_overlay` and
  `get_patcher_status` are no longer commands.
- The overlay builder goes: `build_overlay`, the artifact cache, the blocked-WAD and string-override
  resolution, and the built-in-mod generator with every mod under it. What `overlay` keeps is what a
  swap reads — `skin_number`, `skin_bin_path`, `install_holds_skin` and `SkinContent` — in
  `overlay/skins.rs`.
- The patcher's own settings leave `Config`: `patchTft`, `blockScriptsWad`, `linkedBinCheckEnabled`,
  `wadBlocklist`, `elevateInjector`, `enforceSkinhackScan`, `applyStringOverridesToAllLocales`,
  `verbosePatcherLogging`, `fullWadScan`, `disableCrashReporting` and `builtinMods`. So do
  `alwaysStartPatcher`, `stopPatcherOnSessionEnd`, `reloadModsHotkey`, `killLeagueStopsPatcher` and
  `openOn` in the shell's `Settings`, and the `reload-mods` hotkey action with them.
- `ltk_patcher_dll.dll` and `ltk_patcher_host.exe` leave `src-tauri/resources`, the installer hook
  that killed the host on install goes, and the build script stops hashing them.

**The two settings the frontend strands are recorded, not migrated.** ADR-0024 allows a retired
setting to keep its id and its field, so a `settings.json` written before this release still loads:
every removed key parses as the noise it now is, which `config.rs` and `state.rs` both assert.

## Consequences

- The build's JavaScript drops, since the settings card, the pickers and the overlay-progress
  surfaces go with the feature.
- What a reader loses is the four built-in mods. They were the overlay's, and this build has no
  overlay.
- A `settings.json` with `alwaysStartPatcher: true` no longer starts anything. That is the point:
  the flag could inject.
- **`ltk_overlay` stays, and the overlay's build path is not what it is for.** The mod library's
  analysis still calls `OverlayBuilder::analyze_single_mod` through `mods/overlay_content.rs` to
  derive a mod's WAD footprint — that is what auto-categorization and the per-mod WAD report read.
  Dropping the dependency means dropping those, which is a mod-library decision rather than a
  patcher one.
- The per-mod badge caches the retired `record_overlay_build` filled — WAD reports, linked-bin
  offenders, checksum mismatches — keep their states and their storage, but nothing writes the last
  two any more. They are read by no command this build registers.
- `docs`, `CONTRIBUTING.md` and `LTK-PATCHER-LICENSE.md` still describe the shipped patcher
  binaries. The license file is left alone: it is a notice about binaries that are no longer
  distributed, and deleting a license is not this change's to make.
