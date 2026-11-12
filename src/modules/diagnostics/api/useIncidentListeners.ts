import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import { useToast } from "@/components";
import { m } from "@/i18n";
import type { Incident } from "@/lib/tauri";
import { useTauriEvent } from "@/lib/useTauriEvent";
import { useSettings } from "@/modules/settings";
import { useIncidentLineStore, useInstallMismatchStore } from "@/stores";
import { slashed } from "@/utils";

import { isInformational, verdictCause } from "../utils/incident";
import { diagnosticsKeys } from "./keys";

/**
 * The root's subscriptions for incidents.
 *
 * `incident-recorded` refreshes the list, puts the verdict line in the session
 * bar, and announces it with a toast whose `Details` action opens the Games
 * tab on the incident. The toast is kept in the notification center, because a
 * crash is a question the player comes back to. A wrong-install verdict raises
 * the install mismatch dialog with the log's install as the switch target.
 *
 * The toast used to carry a `Rebuild overlay` action for the verdicts that
 * asked for one. Nothing builds an overlay now (ADR-0058), so it is gone.
 */
export function useIncidentListeners() {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const toast = useToast();
  const show = useIncidentLineStore((s) => s.show);
  const raiseMismatch = useInstallMismatchStore((s) => s.raise);
  const { data: settings } = useSettings();

  useTauriEvent<Incident>("incident-recorded", (incident) => {
    queryClient.invalidateQueries({ queryKey: diagnosticsKeys.incidents() });
    show(incident);
    const ranFrom = incident.game?.gameBaseDir;
    const configuredPath = settings?.leaguePath;
    if (incident.verdict.kind === "wrong-install" && ranFrom && configuredPath) {
      raiseMismatch({
        configuredPath: slashed(configuredPath),
        configuredPatchline: null,
        sessionPath: ranFrom,
        sessionPatchline: null,
      });
    }
    if (incident.verdict.kind === "patcher-did-not-run") return;
    toast.toast({
      type: toastTypeFor(incident),
      title: incident.verdict.title,
      description: verdictCause(incident),
      notify: true,
      action: {
        label: m.diagnostics_incident_details_action(),
        onClick: () =>
          navigate({ to: "/diagnostics", search: { tab: "games", incident: incident.id } }),
      },
    });
  });
}

function toastTypeFor(incident: Incident) {
  return isInformational(incident.verdict.kind) ? ("info" as const) : ("warning" as const);
}
