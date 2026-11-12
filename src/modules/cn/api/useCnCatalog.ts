import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import type { ChampionSwap } from "@/lib/tauri";

import { cnMutations } from "./mutations";
import { cnQueries } from "./queries";

/** The champions the Chinese client's table lists, with the patch it reports. */
export function useCnHeroes() {
  return useQuery(cnQueries.heroes());
}

/** One champion's skins, read once the champion is picked. */
export function useCnHeroSkins(heroId: number | null) {
  return useQuery(cnQueries.heroSkins(heroId));
}

/** What this machine has written into the game's own archives. */
export function useStaticSwaps() {
  return useQuery(cnQueries.swaps());
}

/**
 * One champion's swap, by the name its archive carries.
 *
 * The record is keyed by the character's lowercased English name, which is what
 * the hero's alias is, so the two are matched case-insensitively.
 */
export function useHeroSwap(alias: string | null): ChampionSwap | null {
  const swaps = useStaticSwaps();
  if (alias === null) return null;

  const character = alias.toLowerCase();
  return swaps.data?.find((swap) => swap.character === character) ?? null;
}

/** Write a champion's own archive so it shows a skin the user picked. */
export function useApplyCnSkin() {
  const client = useQueryClient();

  return useMutation(cnMutations.apply(client));
}

/** Put a champion's archive back to the one the client shipped. */
export function useRestoreCnSkin() {
  const client = useQueryClient();

  return useMutation(cnMutations.restore(client));
}

/** Drop a champion's record, which is the way out of one the file no longer agrees with. */
export function useForgetCnSwap() {
  const client = useQueryClient();

  return useMutation(cnMutations.forget(client));
}

/** The archives a third-party skin pack has been written over. */
export function useCnSkinPacks() {
  return useQuery(cnQueries.packs());
}

/** Write a skin pack the user picked over the install's own champion archives. */
export function useImportCnSkinPack() {
  const client = useQueryClient();

  return useMutation(cnMutations.importPack(client));
}

/** Fetch a skin pack from an allowed host and write it over the install's own archives. */
export function useDownloadCnSkinPack() {
  const client = useQueryClient();

  return useMutation(cnMutations.downloadPack(client));
}

/** Put an archive a pack replaced back to the one the client shipped. */
export function useRestoreCnSkinPack() {
  const client = useQueryClient();

  return useMutation(cnMutations.restorePack(client));
}

/** Drop a pack-written archive's record, which is the way out of one the file no longer agrees with. */
export function useForgetCnSkinPack() {
  const client = useQueryClient();

  return useMutation(cnMutations.forgetPack(client));
}
