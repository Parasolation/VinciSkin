import { useMemo, useState } from "react";

import { FormField } from "@/components";
import { m } from "@/i18n";
import type { CnHero } from "@/lib/tauri";

import { filterHeroes } from "../skins";

interface CnHeroPickerProps {
  /** Every champion the client's table ships. */
  heroes: readonly CnHero[];
  /** The champion the page holds, as its id, or an empty string while none is picked. */
  picked: string;
  onPick: (heroId: string) => void;
}

/**
 * The champion list, as portraits a search narrows.
 *
 * The client's own table names a portrait for every champion, and a wall of faces is how a
 * player finds one: the search box narrows the wall, the page holds the pick. The selected tile
 * keeps its ring while the list scrolls, so what the skins beside it belong to stays visible.
 */
export function CnHeroPicker({ heroes, picked, onPick }: CnHeroPickerProps) {
  const [query, setQuery] = useState("");

  const matched = useMemo(() => filterHeroes(heroes, query), [heroes, query]);

  return (
    <div className="flex min-h-0 flex-col gap-2" data-ui="CnHeroPicker">
      <FormField
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        placeholder={m.cn_hero_search_placeholder()}
        aria-label={m.cn_hero_search_placeholder()}
      />

      {matched.length === 0 ? (
        <p className="px-1 py-2 text-meta text-surface-400">{m.cn_heroes_empty()}</p>
      ) : (
        <ul className="grid min-h-0 grid-cols-4 content-start gap-2 overflow-y-auto pr-1 sm:grid-cols-6 lg:grid-cols-4">
          {matched.map((hero) => (
            <li key={hero.id}>
              <HeroTile hero={hero} selected={String(hero.id) === picked} onPick={onPick} />
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

interface HeroTileProps {
  hero: CnHero;
  selected: boolean;
  onPick: (heroId: string) => void;
}

/** One champion: its face, its Chinese title, and whether the page holds it. */
function HeroTile({ hero, selected, onPick }: HeroTileProps) {
  /* The title carries both names, because the Chinese one is what a player reads and the alias
     is what a tooltip's reader may know the champion by. */
  const label = m.cn_hero_option_label({ name: hero.name, alias: hero.alias });

  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      aria-pressed={selected}
      data-ui="cn-hero-tile"
      onClick={() => onPick(String(hero.id))}
      className={
        selected
          ? "flex w-full flex-col items-center gap-1 rounded-md border border-accent-400 bg-accent-500/10 p-1"
          : "flex w-full flex-col items-center gap-1 rounded-md border border-transparent p-1 hover:border-surface-600 hover:bg-surface-700/40"
      }
    >
      <span className="aspect-square w-full overflow-hidden rounded-sm bg-surface-700">
        {hero.portraitUrl === null ? (
          /* A champion whose alias the table spells with something other than letters and digits
             has no portrait to draw, so the tile falls back to its first character. */
          <span className="flex h-full w-full items-center justify-center text-meta text-surface-400">
            {hero.name.slice(0, 1)}
          </span>
        ) : (
          <img
            src={hero.portraitUrl}
            alt=""
            loading="lazy"
            className="h-full w-full object-cover"
          />
        )}
      </span>
      <span className="w-full truncate text-center text-fine text-surface-300">{hero.name}</span>
    </button>
  );
}
