# ADR-0057: The manager keeps three pages

- **Status:** Superseded (2026-10-09) by the injection route: the manager builds an overlay under `<storage>/overlay` and the bundled `ltk_patcher_host.exe` injects `ltk_patcher_dll.dll`, which redirects the game's WAD reads. Nothing is written into the install.
- **Date:** 2026-09-30
- **Crates:** the frontend, in `src/routes` and `src/modules/shell`
- **Related:** [ADR-0056](0056-a-skin-is-swapped-by-writing-the-champions-archive.md), what the catalog
  page does. [ADR-0038](0038-the-rail-fills-the-primary-side-panel.md), the rail this trims.
  [ADR-0043](0043-a-built-in-mod-is-a-generated-project-above-every-mod.md), the built-in mods the
  library's surfaces were for.

## Context and problem statement

The manager is a mod library: a library page, a workshop with a bin editor and a VFX graph, a home
page that reports on the library, and the settings and diagnostics that serve all of it.

This build exists to show a champion a skin the Chinese client already holds and to write it into
the install (ADR-0056). Its readers never open the library, the workshop or the home page, and each
one is a surface to keep working for no reader. The workshop is the largest module in the app and
the home page is what the window opens on.

## Decision

**Three pages are kept: the Chinese client's catalog, diagnostics, and settings.** Everything else
comes out.

- The title bar's nav lists the catalog alone. Diagnostics and settings stay where they were, as
  cells on the right of the same bar.
- `/` redirects to `/cn` rather than drawing a home page.
- The shortcuts are `Ctrl+1` for the catalog, `Ctrl+D` for diagnostics and `Ctrl+,` for settings.
- The home, library and workshop modules go, with the routes that drew them.

**What the kept pages reach for, stays.** The session bar is part of a launch, the mod health that
diagnostics reports is mounted by the shell, and the migration and repair dialogs are the storage
layer rather than a page, so all of them are kept even though the pages that used to draw them are
gone. The library module is trimmed to those pieces rather than deleted.

**The overlay's applied-skins built-in mod goes with them.** It was the second route to what the
catalog page now writes into the install (ADR-0056), and it was already unreachable: the picker that
filled `builtinMods.appliedSkins` was on the catalog page, which now swaps statically. Leaving it
would have kept a stale pick injecting on launch, so `AppliedSkins`, `AppliedSkin`,
`builtinMods.appliedSkins` and the `set_cn_skin` command are gone. The other built-in mods — ward
skins, base skins, map skins, map decorations — stay, since Settings still offers them.

## Consequences

- The built client goes from about 6.2 MB of JavaScript to about 1.9 MB, and the workshop's chunk
  is no longer built at all.
- `openOn` had no page left to send a reader to, so the effect that honoured it is gone and its row
  is out of Settings. `alwaysStartPatcher` went the same way: with no page offering a modded launch,
  a launch that started the patcher had no way to be asked for. Both fields stay in `Config` with no
  row, which ADR-0024 allows for state that lives in the file, so a `settings.json` written before
  this release still loads. The `RETIRED` table ADR-0024 describes is not built, so the two ids are
  recorded here instead — see ADR-0024.
- Nothing in the app patches any more. The launch is a plain one, the overlay is never built, and
  the four built-in mods Settings offered went with it — see
  [ADR-0058](0058-the-manager-writes-a-skin-instead-of-patching-one.md).
- An incident's suspect that names a workshop project no longer offers to open it. Only the mod
  branch of that row is left.
- The session bar no longer names the projects a session is testing.
- The library's browsing surfaces — the grid, the folder cards, the toolbar's profile picker and
  the details tab — are gone with the page that drew them.
- Two fixture files under `src/modules/workshop/` stay: `ranking.fixture.json` and
  `naturalOrder.fixture.json`, which `game_index`, `object_index` and `natural_order` read with
  `include_str!`. The backend's tests are what hold them, so the module around them can go and
  these cannot.
