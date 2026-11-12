# ADR-0056: A skin is swapped by writing the champion's own archive

- **Status:** Superseded (2026-10-09) by the injection route: the manager builds an overlay under `<storage>/overlay` and the bundled `ltk_patcher_host.exe` injects `ltk_patcher_dll.dll`, which redirects the game's WAD reads. Nothing is written into the install.
- **Date:** 2026-09-30
- **Crates:** `ltk-manager-core`, in `static_swap`
- **Related:** [ADR-0043](0043-a-built-in-mod-is-a-generated-project-above-every-mod.md), the built-in
  mod that shows an applied skin through the overlay. [ADR-0005](0005-a-repair-rewrites-the-archive-in-place.md),
  the other place this codebase edits a `.wad.client` in place.
  [ADR-0011](0011-a-repair-may-lose-fidelity-where-no-in-place-edit-exists.md), what a repair does
  when no in-place edit exists.

## Context and problem statement

The overlay is the manager's route to showing a champion a skin it does not own: a built-in mod
hands the game a different `skin0.bin` at runtime, and the install is never touched (ADR-0043).

That route needs the patcher host to inject a DLL into the game process. A client whose anti-cheat
refuses the injection is left with nothing, and the Chinese client is one. The same effect is
reachable without any process: a champion's skins live in one archive under the install,
`DATA/FINAL/Champions/<Name>.wad.client`, and a swap is one chunk of it rewritten.

Writing the install is the part that has to be earned. Three facts decide the shape:

- Every champion archive carries Riot's signature over its table of contents, and no key we hold
  can re-sign one after a chunk changes. The game's archive loader does not check it: the overlay
  route has fed the game archives with no valid signature for years, so a rewritten archive mounts.
- A chunk's slot holds its compressed bytes, and a replacement is rarely the same length. A skin's
  own bin is _larger_ when it stands in for the base skin, because the two objects that make it up
  are renamed onto skin 0 and lose nothing, while the base skin's bin they replace is smaller.
  In-place was measured against four real champions and fitted none of them.
- The game rewrites the archives itself. A patch, a repair, and a client that decides to restore a
  file all put the client's own bytes back under the record.

## Decision

**A swap writes the stand-in into the game's own archive, over a backup that was taken first, and
the archive is never written over unless the record and the file agree about what is there.**

**The write moves one 32-byte entry, and appends when it must.** The chunk's body is written over
its slot when the replacement fits, and appended at the end of the file when it does not; either
way only that chunk's table entry is stamped, and the file's length changes only in the second
case. Re-encoding the archive, and the tail layout Riot writes after the last chunk, are both
avoided rather than reproduced. The replacement is claimed by its own codec rather than by
Riot's: the encoder tries zstd levels from 22 down and keeps the smallest, or stores the bytes
uncompressed when nothing beats them.

**The stand-in is the same one the overlay writes.** `SkinContent::parse` reads the skin's own
`skinN.bin`, renames its two objects onto skin 0 and hands back the bytes, so the two routes
cannot drift apart in what a swapped champion wears. `skin0.bin` is what is replaced; the rest of
the archive is left where it was.

**A backup exists before the first byte is written, and a record describes what was written.**
Under `<storage>/static-swap/`, the client's archive is copied once per champion, and
`records.json` holds one row per champion: the archive's `DATA/FINAL`-relative name, the game's
own skin number, and a fingerprint (length, clock, SHA-256) of both the original and the written
file.

**The digest decides what state a swap is in; the clock does not.** `SwapState` is `untouched`,
`applied`, `reverted`, `drifted` or `backupMissing`. A repair changes the modification time
without changing the bytes, so a clock answers the drift question wrongly; a length that matches
neither recorded one is the one cheap answer there is, and the digest is asked otherwise.
`applied` is a promise that the archive can be put back, so a missing backup is its own state
rather than a detail of that one.

**Only a file the record accounts for may be written or restored.** A record reading `drifted` or
`backupMissing` refuses both, because the file it would overwrite is neither ours nor the
client's, and `forget` is the way out — it drops the row and touches no archive.

**The catalog's skin id is the game's number, a thousand per hero.** `apply` takes the catalog's
id and the hero's id, and the number the archive holds is the difference. The base skin is not a
write at all: it is the same call as `restore`.

The overlay's built-in mod for applied skins stood beside this one while both routes were offered,
and was retired once this one worked — see
[ADR-0057](0057-the-manager-keeps-three-pages.md). What the two routes share — reading a skin's own
bin and renaming its objects onto skin 0 — is `SkinContent`, which both called.

## Consequences

- Measured at 16.19 on Annie's 4083-chunk archive: the stand-in for a skin was 1767 bytes over a
  1507-byte slot, so it was appended and the file grew by the difference. Every other chunk read
  back unchanged, and the archive still mounted.
- Every champion archive read was signed, and none of them can be re-signed. A mount that checks
  the signature would refuse a swapped archive, which is the one risk the design cannot answer
  from this side of the game.
- A game patch, a repair, or the client restoring a file puts the client's archive back. The
  record then reads `reverted`, and writing another swap is safe.
- The install is mutated, which is the whole difference from the overlay. Everything in the state
  machine exists to keep that reversible, and nothing here is a substitute for a backup the user
  keeps themselves.
- The swap shows on the reader's own screen alone, as the overlay's does. Every other player still
  sees the skin the server named.
- A swap needs no patcher, so the Chinese client's page launches the game plainly rather than
  starting one.
