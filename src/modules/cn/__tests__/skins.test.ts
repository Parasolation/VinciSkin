import { describe, expect, it } from "vitest";

import type { CnHero, CnQuality, CnSkin } from "@/lib/tauri";

import { filterCards, filterHeroes, qualityLabel, qualityOptions, skinCards } from "../skins";

/** A champion of the client's roster, everything a test does not name left at its base value. */
function hero(overrides: Partial<CnHero> & { id: number; alias: string; name: string }): CnHero {
  return {
    keywords: [],
    portraitUrl: `https://game.gtimg.cn/images/lol/act/img/champion/${overrides.alias}.png`,
    ...overrides,
  };
}

const roster = [
  hero({
    id: 1,
    alias: "Annie",
    name: "黑暗之女",
    keywords: ["安妮", "黑暗之女", "火女", "Annie", "anni", "heianzhinv", "huonv", "hazn", "hn"],
  }),
  hero({
    id: 103,
    alias: "Ahri",
    name: "九尾妖狐",
    keywords: ["九尾", "九尾妖狐", "狐狸", "Ahri", "jiuwei", "huli", "ali", "jwyh"],
  }),
  hero({
    id: 64,
    alias: "LeeSin",
    name: "盲僧",
    keywords: ["盲僧", "瞎子", "李青", "LeeSin", "mangseng", "xiazi", "liqing"],
  }),
];

/** A skin of a champion's table, everything a test does not name left at its base value. */
function skin(overrides: Partial<CnSkin> & { skinId: number; name: string }): CnSkin {
  return {
    heroId: 1,
    isBase: false,
    isChroma: false,
    parentId: overrides.skinId,
    quality: "common",
    emblemsName: null,
    artUrl: null,
    instanceId: null,
    ...overrides,
  };
}

const base = skin({ skinId: 1000, name: "黑暗之女", isBase: true });
const cafeCuties = skin({ skinId: 1022, name: "咖啡甜心 安妮", quality: "epic" });
const cafeCutiesCream = skin({
  skinId: 1023,
  name: "咖啡甜心 安妮 草莓戚风",
  isChroma: true,
  parentId: 1022,
  quality: "epic",
});
const cafeCutiesMint = skin({
  skinId: 1024,
  name: "咖啡甜心 安妮 抹茶巧克力",
  isChroma: true,
  parentId: 1022,
  quality: "epic",
});
const hextech = skin({ skinId: 1010, name: "海克斯科技 安妮", quality: "limited" });

const table = [base, cafeCuties, cafeCutiesCream, cafeCutiesMint, hextech];

describe("skinCards", () => {
  it("draws one card per skin, with its chromas folded under it", () => {
    const cards = skinCards(table);

    expect(cards.map((card) => card.skin.skinId)).toEqual([1000, 1022, 1010]);
    expect(cards[1].chromas.map((chroma) => chroma.skinId)).toEqual([1023, 1024]);
  });

  it("leaves a skin with no chromas with an empty fold", () => {
    const cards = skinCards([base, hextech]);

    expect(cards.every((card) => card.chromas.length === 0)).toBe(true);
  });

  /* A table naming a parent it does not list is the one case a chroma would otherwise vanish
     from the page, so it keeps a card of its own. */
  it("keeps a chroma whose skin the table does not list", () => {
    const orphan = skin({
      skinId: 1032,
      name: "魔域梦魇 安妮 红夜梦境",
      isChroma: true,
      parentId: 1031,
    });

    const cards = skinCards([base, orphan]);

    expect(cards.map((card) => card.skin.skinId)).toEqual([1000, 1032]);
    expect(cards[1].chromas).toEqual([]);
  });
});

describe("filterCards", () => {
  const cards = skinCards(table);
  const ids = (kept: ReturnType<typeof filterCards>) => kept.map((card) => card.skin.skinId);

  it("returns every card when no filter is set", () => {
    expect(
      ids(filterCards(cards, { quality: null, onlyApplied: false, appliedSkinId: null })),
    ).toHaveLength(3);
  });

  it("keeps the cards of the tier it is given", () => {
    expect(
      ids(filterCards(cards, { quality: "epic", onlyApplied: false, appliedSkinId: null })),
    ).toEqual([1022]);
    expect(
      ids(filterCards(cards, { quality: "limited", onlyApplied: false, appliedSkinId: null })),
    ).toEqual([1010]);
  });

  it("keeps only the card the applied skin belongs to", () => {
    expect(
      ids(filterCards(cards, { quality: null, onlyApplied: true, appliedSkinId: 1010 })),
    ).toEqual([1010]);
  });

  /* A chroma is applied as often as a skin, and the card it folds under is the one to keep. */
  it("keeps the card an applied chroma folds under", () => {
    expect(
      ids(filterCards(cards, { quality: null, onlyApplied: true, appliedSkinId: 1024 })),
    ).toEqual([1022]);
  });

  /* Nothing applied is nothing to keep rather than everything, which is what the toggle would
     otherwise mean while the champion still wears its own skin. */
  it("keeps nothing while the filter is on and nothing is applied", () => {
    expect(
      ids(filterCards(cards, { quality: null, onlyApplied: true, appliedSkinId: null })),
    ).toEqual([]);
  });

  it("applies both filters together", () => {
    const filters = { quality: "epic" as const, onlyApplied: true };

    expect(ids(filterCards(cards, { ...filters, appliedSkinId: 1010 }))).toEqual([]);
    expect(ids(filterCards(cards, { ...filters, appliedSkinId: 1023 }))).toEqual([1022]);
  });
});

describe("qualityOptions", () => {
  it("names every tier the client has a word for", () => {
    const values: CnQuality[] = qualityOptions().map((option) => option.value);

    expect(values).toEqual([
      "common",
      "epic",
      "legend",
      "mythic",
      "ultimate",
      "prestige",
      "exalted",
      "limited",
    ]);
  });

  it("gives each tier a label and no two the same", () => {
    const labels = qualityOptions().map((option) => option.label);

    expect(labels.every((label) => label.length > 0)).toBe(true);
    expect(new Set(labels).size).toBe(labels.length);
  });

  it("reads a tier's label back", () => {
    expect(qualityLabel("epic")).toBe("Epic");
    expect(qualityLabel("limited")).toBe("Limited");
  });
});

describe("filterHeroes", () => {
  const aliases = (heroes: readonly CnHero[]) => heroes.map((candidate) => candidate.alias);

  it("leaves the whole roster while nothing is typed", () => {
    expect(aliases(filterHeroes(roster, ""))).toEqual(["Annie", "Ahri", "LeeSin"]);
    expect(aliases(filterHeroes(roster, "   "))).toEqual(["Annie", "Ahri", "LeeSin"]);
  });

  /* The Chinese title is what a player types, so a substring of it reaches the champion. */
  it("matches the Chinese title a player types", () => {
    expect(aliases(filterHeroes(roster, "九尾"))).toEqual(["Ahri"]);
    expect(aliases(filterHeroes(roster, "盲僧"))).toEqual(["LeeSin"]);
  });

  /* The alias is matched without case, so the English name reaches the champion as well. */
  it("matches the alias through any case", () => {
    expect(aliases(filterHeroes(roster, "lee"))).toEqual(["LeeSin"]);
    expect(aliases(filterHeroes(roster, "AHRI"))).toEqual(["Ahri"]);
  });

  /* A player calls a champion by its nickname more often than its title, so those reach it too. */
  it("matches a nickname the client's keywords carry", () => {
    expect(aliases(filterHeroes(roster, "瞎子"))).toEqual(["LeeSin"]);
    expect(aliases(filterHeroes(roster, "狐狸"))).toEqual(["Ahri"]);
    expect(aliases(filterHeroes(roster, "安妮"))).toEqual(["Annie"]);
  });

  /* Pinyin is what a player types without a Chinese keyboard, so the spellings reach the
     champion as well, initials included. */
  it("matches the pinyin spellings and their initials", () => {
    expect(aliases(filterHeroes(roster, "mangseng"))).toEqual(["LeeSin"]);
    expect(aliases(filterHeroes(roster, "jiuwei"))).toEqual(["Ahri"]);
    expect(aliases(filterHeroes(roster, "hn"))).toEqual(["Annie"]);
  });

  it("matches the id a table is addressed by", () => {
    expect(aliases(filterHeroes(roster, "64"))).toEqual(["LeeSin"]);
  });

  /* Nothing matching is nothing rather than everything, which is what an empty search means. */
  it("leaves nothing when nothing matches", () => {
    expect(filterHeroes(roster, "亚索")).toEqual([]);
  });
});
