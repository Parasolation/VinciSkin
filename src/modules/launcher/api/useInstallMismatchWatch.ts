import { useCallback } from "react";

import { api, isOk } from "@/lib/tauri";
import { useTauriEvent } from "@/lib/useTauriEvent";
import { useInstallMismatchStore } from "@/stores";

/**
 * Asks the backend whether the League the client runs is the install the
 * manager is set up for, at the moments the answer can change.
 *
 * The answer matters more here than it did for an overlay: a swap writes into
 * the configured install, so a client running from another one would have the
 * skin written somewhere the game never reads. Per "The install mismatch
 * dialog" in docs/ux/LEAGUE_DIAGNOSTICS.md. Mount once at the root. The
 * backend is silent without a client, a session, or a registry entry for the
 * configured path, and so is this.
 *
 * A session opening is also what lets a dialog the reader kept down come back,
 * which is why the store is reset before the check rather than after it.
 *
 * The other moment this used to ask at — a patcher starting and its DLL
 * attaching to a game — is gone with the patcher (ADR-0058).
 */
export function useInstallMismatchWatch() {
  const raise = useInstallMismatchStore((s) => s.raise);
  const reset = useInstallMismatchStore((s) => s.reset);

  const check = useCallback(async () => {
    const result = await api.launcher.checkInstallMismatch();
    if (isOk(result) && result.value) raise(result.value);
  }, [raise]);

  useTauriEvent<unknown>("session-started", () => {
    reset();
    void check();
  });
}
