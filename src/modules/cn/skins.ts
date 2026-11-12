import { m } from "@/i18n";
import type { CnHero, CnQuality, CnSkin } from "@/lib/tauri";

/** One skin of a champion's table, with the chromas the table lists under it. */
export interface SkinCard {
  /** The skin itself, never a chroma. */
  skin: CnSkin;
  /** The variants of the skin, in table order. */
  chromas: CnSkin[];
}

/**
 * A champion's table as the page draws it: one card per skin, its chromas nested.
 *
 * The table lists a skin's chromas beside the skins rather than under them, and a chroma names
 * its skin in `chromasBelongId`, so folding is what turns the flat table into cards. A chroma
 * whose parent the table does not list keeps a card of its own instead of being dropped, which
 * is what a table naming a parent from another hero would otherwise cost.
 */
export function skinCards(skins: readonly CnSkin[]): SkinCard[] {
  const byId = new Map(skins.map((skin) => [skin.skinId, skin]));
  const chromas = new Map<number, CnSkin[]>();
  const cards: SkinCard[] = [];

  for (const skin of skins) {
    if (skin.parentId === skin.skinId) {
      cards.push({ skin, chromas: [] });
      continue;
    }

    const parent = chromas.get(skin.parentId);
    if (parent) {
      parent.push(skin);
    } else {
      chromas.set(skin.parentId, [skin]);
    }
  }

  const folded = cards.map((card) => ({
    skin: card.skin,
    chromas: chromas.get(card.skin.skinId) ?? [],
  }));

  for (const [parentId, orphaned] of chromas) {
    if (!byId.has(parentId)) {
      folded.push(...orphaned.map((skin) => ({ skin, chromas: [] })));
    }
  }

  return folded;
}

/** What the page's filters narrow the cards by. */
export interface SkinFilters {
  /** The tier to keep, or none. */
  quality: CnQuality | null;
  /** Whether only the champion's own skin is kept. */
  onlyApplied: boolean;
  /** The catalog id of the skin the champion shows, or none while nothing is applied. */
  appliedSkinId: number | null;
}

/** What the page's filters leave of `cards`. */
export function filterCards(cards: readonly SkinCard[], filters: SkinFilters): SkinCard[] {
  return cards.filter((card) => {
    if (filters.quality !== null && card.skin.quality !== filters.quality) {
      return false;
    }

    /* Nothing applied is nothing to keep rather than everything, which is what
       the toggle means while a champion is still wearing its own skin. */
    if (filters.onlyApplied) {
      if (filters.appliedSkinId === null) {
        return false;
      }

      return carries(card, filters.appliedSkinId);
    }

    return true;
  });
}

/** Whether `card` is the skin `skinId` names, whether as itself or as one of its chromas. */
function carries(card: SkinCard, skinId: number): boolean {
  return card.skin.skinId === skinId || card.chromas.some((chroma) => chroma.skinId === skinId);
}

/** Each quality with its label, in the order the picker lists them. */
export function qualityOptions(): { value: CnQuality; label: string }[] {
  return [
    { value: "common", label: m.cn_quality_common() },
    { value: "epic", label: m.cn_quality_epic() },
    { value: "legend", label: m.cn_quality_legend() },
    { value: "mythic", label: m.cn_quality_mythic() },
    { value: "ultimate", label: m.cn_quality_ultimate() },
    { value: "prestige", label: m.cn_quality_prestige() },
    { value: "exalted", label: m.cn_quality_exalted() },
    { value: "limited", label: m.cn_quality_limited() },
  ];
}

/** The tier `quality` names, as a card's badge reads it. */
export function qualityLabel(quality: CnQuality): string {
  return qualityOptions().find((option) => option.value === quality)?.label ?? quality;
}

/**
 * The champions `query` leaves, in table order.
 *
 * A player finds their champion by whatever they happen to call it, so every name the client
 * itself indexes a champion by is matched: its nickname (`亚索`), its title (`疾风剑豪`), its
 * English name (`Yasuo`), its pinyin (`jifengjianhao`) and its initial (`ys`). All of those
 * live in the client's own `keywords`, so one token list carries them. The id is matched too,
 * since a table is addressed by it.
 *
 * Every term is a case-insensitive substring, so a partial `jf` reaches 疾风剑豪 through `jfjh`.
 * A query of several words is not split: a champion has no multi-word handle to match one word
 * each against.
 */
export function filterHeroes(heroes: readonly CnHero[], query: string): CnHero[] {
  const needle = query.trim().toLowerCase();
  if (needle === "") return [...heroes];

  return heroes.filter(
    (hero) => String(hero.id).includes(needle) || terms(hero).some((term) => term.includes(needle)),
  );
}

/** Everything a player might call `hero`, as one lowercased list: the table's own keywords,
    with the title and the alias kept beside them in case a table ships no keywords at all. */
function terms(hero: CnHero): string[] {
  return [hero.name, hero.alias, ...hero.keywords].map((term) => term.toLowerCase());
}
