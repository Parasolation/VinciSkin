import { useMutation, useQueryClient } from "@tanstack/react-query";

import { deepLinkMutations } from "./mutations";

export type { ProtocolInstallVariables } from "./mutations";

/** Install the mod a `vinciskin://` link names. */
export function useProtocolInstall() {
  return useMutation(deepLinkMutations.install(useQueryClient()));
}
