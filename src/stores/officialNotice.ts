import { create } from "zustand";
import { persist } from "zustand/middleware";

import { keepUnversioned, localJsonStorage } from "./storage";

interface OfficialNoticeState {
  acknowledged: boolean;
  acknowledge: () => void;
}

export const useOfficialNoticeStore = create<OfficialNoticeState>()(
  persist(
    (set) => ({
      acknowledged: false,
      acknowledge: () => set({ acknowledged: true }),
    }),
    {
      name: "official-notice",
      version: 1,
      migrate: keepUnversioned,
      storage: localJsonStorage,
    },
  ),
);

export function useOfficialNoticeAcknowledged() {
  return useOfficialNoticeStore((s) => s.acknowledged);
}

export function useAcknowledgeOfficialNotice() {
  return useOfficialNoticeStore((s) => s.acknowledge);
}
