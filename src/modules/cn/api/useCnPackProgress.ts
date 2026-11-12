import type { CnPackProgress } from "@/lib/tauri";
import { useTauriProgress } from "@/lib/useTauriProgress";

/**
 * How far the skin pack being written has got, or none between writes.
 *
 * The backend reports a stage per archive, so the panel can say which one it is on rather than
 * only that something is happening. `complete` clears it, which is what the terminal stage is.
 */
export function useCnPackProgress(): CnPackProgress | null {
  const { progress } = useTauriProgress<CnPackProgress>("cn-pack-progress", {
    terminalStages: ["complete"],
  });

  return progress;
}
