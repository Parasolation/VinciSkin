import type { Incident, ScanStatus } from "@/lib/tauri";

/**
 * How each rejection reads, for the surfaces that report one.
 *
 * Nothing records a rejection any more — this build writes a skin into the
 * install instead of patching it (ADR-0058) — so these only read history: an
 * incident or a decoded token from a session that ran while the manager still
 * patched.
 */
const SCAN_CAUSES: Readonly<Record<ScanStatus, string>> = {
  skinhack:
    "The patcher's safety scan found a skinhack among the enabled mods, so no mods were applied.",
  "missing-bin": "The scan could not find a linked .bin a mod needs, so no mods were applied.",
  corrupt: "A modded WAD could not be read, so no mods were applied.",
  "out-of-memory": "The game ran out of memory while loading mods.",
  "base-skin":
    "The base-skin check found a champion skin with a mesh missing, so no mods were applied.",
  "base-wad":
    "The scan objected to League's own copy of an archive rather than to a mod, so no mods were applied.",
  unknown: "A modded file failed the game's integrity scan, so no mods were applied.",
};

/** The status in a few words, for a field that has already been labelled. */
export const SCAN_STATUS_LABELS: Readonly<Record<ScanStatus, string>> = {
  skinhack: "skinhack",
  "missing-bin": "a linked .bin is missing",
  corrupt: "corrupt or unsupported",
  "out-of-memory": "out of memory mid-scan",
  "base-skin": "base skin with a mesh missing",
  "base-wad": "the game's own copy of the archive",
  unknown: "a status this build does not know",
};

/**
 * What a recorded rejection reads like under its verdict.
 *
 * An incident stored before the backend stopped writing prose still carries its
 * own sentence, and keeps it, so a history does not change wording underneath
 * the reader.
 */
export function scanRejectionCause(incident: Incident): string {
  if (incident.verdict.cause) return incident.verdict.cause;
  const status = incident.scanStatus;
  if (!status) return "";

  const lead = SCAN_CAUSES[status];
  const code = incident.scanStatusCode;
  const named = status === "unknown" && code ? `${lead} The status was ${code}.` : lead;

  const others = Math.max(0, incident.scanRejected - 1);
  if (others === 0) return named;
  return `${named} ${others} more archive${others === 1 ? "" : "s"} failed the scan.`;
}
