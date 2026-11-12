# ADR-0059: The manager injects a skin instead of writing one

- **Status:** Accepted (2026-10-12)
- **Date:** 2026-10-12
- **Crates:** `vinciskin-core` (the overlay the swap writes), the Tauri shell (the loader it
  starts), the frontend (the CN page)
- **Related:** [ADR-0058](0058-the-manager-writes-a-skin-instead-of-patching-one.md) and
  [ADR-0056](0056-a-skin-is-swapped-by-writing-the-champions-archive.md), the write this
  replaces. [ADR-0012](0012-the-overlay-merges-a-mod-over-the-games-copy.md), the overlay this
  serves.

## Context and problem statement

ADR-0056 and ADR-0058 made a skin a write into the game's own
`DATA/FINAL/Champions/<Name>.wad.client`, on the reasoning that it needs no injected DLL. The
Chinese client is the reason the manager exists, and there the write does not hold up:

- The game runs at **high integrity** (`[LoaderDiag] target elevation=yes integrity=high`), so a
  process at a lower integrity level cannot hook it at all (`Failed to hook: ... Win32=5`).
- The bundled ltk patcher DLL aborts its own init on that build. Its integrity hook looks for an
  OpenSSL pattern in the game's `.rdata`, and the Chinese executable does not have the section it
  expects: `ossl: .rdata section not found` → `failed to install integrity hook`, and `init done`
  never arrives, so the overlay hook is never installed either.
- A modified archive in the install is a file the client's own update and repair paths fight
  over, and one the game's launch path is free to reject.

## Decision

A skin is an **overlay** the manager serves, and the manager starts the loader that serves it.

- The swap writes `<storage>/overlay/DATA/FINAL/<archive>`, seeded from the client's own archive
  with the classic skin slot replaced, and never touches the install.
- The loader is **cslol's legacy one** — `cslol-inj.exe` beside `cslol-dll-legacy.dll` — because
  it has no integrity hook to fail: it hooks `CreateFileA` and redirects the game's WAD reads
  into the overlay. Its init reaches `Init done!` on the Chinese client.
- It is started through the `runas` verb, since the game is elevated, and the process handle it
  hands back is kept so the session can be stopped again.

The ltk patcher binaries are no longer bundled: their DLL cannot init on this client.

## Consequences

- Starting the loader raises a UAC prompt, and the loader must be up before the game starts. It
  scans for the game window, so the client may be open first and the game entered afterwards.
- The loader has no stdio protocol of its own, so the session's state is read from the game's own
  process rather than from the loader.
- The static write this replaces is retired: `static_swap::apply`/`restore` and the backup
  directory they kept are gone, and the record under `<storage>/static-swap/records.json` now
  describes an overlay entry rather than a written archive.
