import { useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";

import type { OverlayProgress } from "@/lib/tauri";
import { useTauriProgress } from "@/lib/useTauriProgress";

import { libraryKeys } from "./keys";

const TERMINAL_STAGES = ["complete"];

export function useOverlayProgress() {
  const { progress } = useTauriProgress<OverlayProgress>("overlay-progress", {
    terminalStages: TERMINAL_STAGES,
  });
  const queryClient = useQueryClient();

  // Refresh per-mod WAD reports when the backend signals that the cache has
  // been updated. This fires *after* `record_reports()` persists the data,
  // avoiding the race where `OverlayStage::Complete` arrives before the
  // cache is written.
  useEffect(() => {
    const unlisten = listen("wad-reports-updated", () => {
      queryClient.invalidateQueries({ queryKey: libraryKeys.wadReports() });
    });
    return () => {
      unlisten.then((fn) => fn()).catch(() => {});
    };
  }, [queryClient]);

  return progress;
}
