import { SealCheckIcon } from "@phosphor-icons/react";

import { ExternalLink } from "@/components";
import { m } from "@/i18n";

import { OFFICIAL_URL } from "../official";

/**
 * The one-line notice that stays under every page: free, official, at risk.
 *
 * The middle segment opens the repository, so the sentence ends where the
 * reader can check it.
 */
export function OfficialFooterBar() {
  return (
    <footer
      className="flex h-7 shrink-0 items-center gap-2 border-t border-accent-500/30 bg-accent-500/10 px-4 text-[0.625rem] text-accent-200 select-none"
      data-ui="OfficialFooterBar"
    >
      <SealCheckIcon className="h-3 w-3 shrink-0" weight="fill" />
      <span className="whitespace-nowrap">{m.shell_official_footer_label()}</span>
      <ExternalLink
        href={OFFICIAL_URL}
        hideIcon
        className="font-medium text-accent-300 transition-colors hover:text-accent-100"
      >
        {m.shell_official_footer_url()}
      </ExternalLink>
      <span className="whitespace-nowrap">{m.shell_official_footer_risk()}</span>
    </footer>
  );
}
