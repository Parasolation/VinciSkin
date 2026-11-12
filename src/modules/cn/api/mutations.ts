import { mutationOptions, type QueryClient } from "@tanstack/react-query";

import { api, type AppError, type ChampionSwap, type PackSwap } from "@/lib/tauri";
import { mutationFn } from "@/utils/query";

import { cnKeys } from "./queries";

/** The champion and the catalog skin to write into its own archive. */
export interface CnSkinPick {
  /** The hero's English name, which names the character's own archive. */
  alias: string;
  heroId: number;
  /** The catalog's id for the skin, or the hero's base skin to put the champion back. */
  skinId: number;
}

/** One champion, by the name its archive carries. */
export interface CnChampion {
  alias: string;
}

/** One archive a skin pack was written into, by its `DATA/FINAL`-relative name. */
export interface CnPackArchive {
  archive: string;
}

/** Writes against the Chinese client's own archives. */
export const cnMutations = {
  /* The archive itself is what holds the swap, so the record the backend keeps
     is refreshed rather than patched: whether the write landed as applied or was
     turned into a restore is the backend's to say. */
  apply: (client: QueryClient) =>
    mutationOptions<ChampionSwap, AppError, CnSkinPick>({
      mutationFn: mutationFn((pick: CnSkinPick) =>
        api.cn.applyStaticSwap(pick.alias, pick.heroId, pick.skinId),
      ),
      onSuccess: () => client.invalidateQueries({ queryKey: cnKeys.swaps() }),
    }),

  restore: (client: QueryClient) =>
    mutationOptions<ChampionSwap, AppError, CnChampion>({
      mutationFn: mutationFn((champion: CnChampion) => api.cn.restoreStaticSwap(champion.alias)),
      onSuccess: () => client.invalidateQueries({ queryKey: cnKeys.swaps() }),
    }),

  /* The way out of a record the file no longer agrees with, which touches no
     archive and so needs no confirmation beyond the page's own wording. */
  forget: (client: QueryClient) =>
    mutationOptions<null, AppError, CnChampion>({
      mutationFn: mutationFn((champion: CnChampion) => api.cn.forgetStaticSwap(champion.alias)),
      onSuccess: () => client.invalidateQueries({ queryKey: cnKeys.swaps() }),
    }),

  /* A pack replaces whole archives, so the record behind the list is what says
     what landed rather than the pick that was made. */
  importPack: (client: QueryClient) =>
    mutationOptions<PackSwap[], AppError, string>({
      mutationFn: mutationFn((filePath: string) => api.cn.importSkinPack(filePath)),
      onSuccess: () => client.invalidateQueries({ queryKey: cnKeys.packs() }),
    }),

  /* A pack that is not on this machine yet is fetched and written in one call, so what lands is
     the record's to say rather than the address that was pasted. */
  downloadPack: (client: QueryClient) =>
    mutationOptions<PackSwap[], AppError, string>({
      mutationFn: mutationFn((url: string) => api.cn.downloadSkinPack(url)),
      onSuccess: () => client.invalidateQueries({ queryKey: cnKeys.packs() }),
    }),

  restorePack: (client: QueryClient) =>
    mutationOptions<PackSwap, AppError, CnPackArchive>({
      mutationFn: mutationFn((target: CnPackArchive) => api.cn.restoreSkinPack(target.archive)),
      onSuccess: () => client.invalidateQueries({ queryKey: cnKeys.packs() }),
    }),

  forgetPack: (client: QueryClient) =>
    mutationOptions<null, AppError, CnPackArchive>({
      mutationFn: mutationFn((target: CnPackArchive) => api.cn.forgetSkinPack(target.archive)),
      onSuccess: () => client.invalidateQueries({ queryKey: cnKeys.packs() }),
    }),
} as const;
