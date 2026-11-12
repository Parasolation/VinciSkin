import { useCallback, useRef, useState } from "react";

import { useToast } from "@/components";
import type { AppError, LaunchOutcome } from "@/lib/tauri";
import { useHddWarning } from "@/modules/settings";

import { useLaunchErrorToast } from "./useLaunchErrorToast";
import { useLaunchLeague } from "./useLaunchLeague";

/**
 * Launching League through the Riot Client, and nothing else.
 *
 * There was a `play` here that built an overlay and waited for a patcher before
 * launching, and a step machine the session bar drew. This build writes a skin
 * into the game's own archive instead (ADR-0056), so a launch is a launch: the
 * game wants no second process and there is nothing to step through.
 */
export function usePlay() {
  const maybeShowHddWarning = useHddWarning();
  const launchLeague = useLaunchLeague();
  const showLaunchError = useLaunchErrorToast();
  const toast = useToast();

  /* A ref rather than the state: two clicks in the same tick would both read a
     stale `isBusy` from the render they closed over. */
  const launching = useRef(false);
  const [isBusy, setBusy] = useState(false);

  const launchOnly = useCallback(async () => {
    if (launching.current) return;
    launching.current = true;
    setBusy(true);

    try {
      await maybeShowHddWarning();
      const outcome = await launchLeague.mutateAsync(undefined);
      if (outcome && alreadyUp(outcome)) {
        toast.info("League is already running", alreadyUpDetail(outcome.route));
      }
    } catch (error) {
      showLaunchError(error as AppError);
    } finally {
      launching.current = false;
      setBusy(false);
    }
  }, [maybeShowHddWarning, launchLeague, showLaunchError, toast]);

  return { launchOnly, isBusy };
}

/**
 * Whether the launch found a game rather than starting one.
 *
 * `ADOPTED` is the same news with a better ending: the Riot Client had lost
 * track of the game and has now been handed it.
 */
function alreadyUp(outcome: LaunchOutcome): boolean {
  return outcome.route === "ALREADY_RUNNING" || outcome.route === "ADOPTED";
}

/** Whether anything came of finding the game, beyond leaving it alone. */
function alreadyUpDetail(route: LaunchOutcome["route"]): string {
  if (route === "ADOPTED") {
    return "There was nothing to launch. The manager is following the game you already had open.";
  }
  return "There was nothing to launch, so your open client was left alone.";
}
