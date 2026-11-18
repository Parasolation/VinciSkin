import { SealCheckIcon } from "@phosphor-icons/react";

import { Button, Dialog } from "@/components";
import { m } from "@/i18n";
import { useAcknowledgeOfficialNotice, useOfficialNoticeAcknowledged } from "@/stores";

/**
 * The one-time notice naming the official, free build and the risk it carries.
 *
 * It raises itself on the first run of an unacknowledged install, per ADR-0022,
 * and the answer persists, so it never asks the same reader twice.
 */
export function OfficialNoticeDialog() {
  const acknowledged = useOfficialNoticeAcknowledged();
  const acknowledge = useAcknowledgeOfficialNotice();

  return (
    <Dialog.Shell
      open={!acknowledged}
      onClose={acknowledge}
      title={m.shell_official_notice_title()}
      size="sm"
    >
      <Dialog.Body>
        <p className="text-sm text-surface-300">{m.shell_official_notice_description()}</p>
        <p className="mt-3 text-sm text-surface-400">{m.shell_official_notice_risk()}</p>
      </Dialog.Body>
      <Dialog.Footer>
        <Button variant="outline" size="sm" onClick={acknowledge}>
          <SealCheckIcon className="h-4 w-4" weight="bold" />
          {m.shell_official_notice_action()}
        </Button>
      </Dialog.Footer>
    </Dialog.Shell>
  );
}
