import { queryOptions } from "@tanstack/react-query";

import {
  api,
  type AppError,
  type ChampionSwap,
  type CnHeroRoster,
  type CnHeroSkins,
  type PackSwap,
} from "@/lib/tauri";
import { queryFn, queryFnWithArgs } from "@/utils/query";

/** Query keys for the Chinese client's catalog. */
export const cnKeys = {
  heroes: () => ["cn", "heroes"] as const,
  heroSkins: (heroId: number) => ["cn", "heroSkins", heroId] as const,
  swaps: () => ["cn", "swaps"] as const,
  packs: () => ["cn", "packs"] as const,
} as const;

/* Both tables change only with a patch, and a champion's skins are read from one
   file that does not change between patches either, so an answer stands for an
   hour rather than being re-asked on every visit. */
const TABLE_STALE_MS = 60 * 60 * 1000;

/** What the Chinese client's own tables answer. */
export const cnQueries = {
  /* No champion is picked yet, so this is the screen's first request and the
     one that says which patch the tables are on. */
  heroes: () =>
    queryOptions<CnHeroRoster, AppError>({
      queryKey: cnKeys.heroes(),
      queryFn: queryFn(api.cn.heroes),
      staleTime: TABLE_STALE_MS,
      retry: false,
    }),

  /* Idle until a champion is picked, because the CDN serves one file per
     champion and asking for all of them would be one request per champion. */
  heroSkins: (heroId: number | null) =>
    queryOptions<CnHeroSkins, AppError>({
      queryKey: cnKeys.heroSkins(heroId ?? 0),
      queryFn: queryFnWithArgs(api.cn.heroSkins, heroId ?? 0),
      enabled: heroId !== null,
      staleTime: TABLE_STALE_MS,
      retry: false,
    }),

  /* What this machine has written into the game, which a swap changes and the
     game itself can change under the record, so it is asked rather than assumed
     and stands only for as long as the page is open. */
  swaps: () =>
    queryOptions<ChampionSwap[], AppError>({
      queryKey: cnKeys.swaps(),
      queryFn: queryFn(api.cn.staticSwaps),
      staleTime: 0,
      retry: false,
    }),

  /* What packs have been written into the game, asked for the same reason the
     swaps are: the game can change a file under the record. */
  packs: () =>
    queryOptions<PackSwap[], AppError>({
      queryKey: cnKeys.packs(),
      queryFn: queryFn(api.cn.skinPacks),
      staleTime: 0,
      retry: false,
    }),
} as const;
