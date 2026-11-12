// @vitest-environment happy-dom

import { QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";

import { ConfirmHost, ToastProvider } from "@/components";
import type { ChampionSwap, CnHero, CnHeroSkins, CnQuality, CnSkin } from "@/lib/tauri";
import { mockInvoke } from "@/test/mocks/tauri";
import { createTestQueryClient } from "@/test/utils";

import { CnSkinGrid } from "../CnSkinGrid";

const leeSin: CnHero = {
  id: 64,
  alias: "LeeSin",
  name: "盲僧",
  keywords: ["盲僧", "瞎子", "李青", "LeeSin", "mangseng"],
  portraitUrl: "https://game.gtimg.cn/images/lol/act/img/champion/LeeSin.png",
};

/** A skin of Lee Sin's table, everything the tests do not name left at its base value. */
function skin(overrides: Partial<CnSkin> & { skinId: number; name: string }): CnSkin {
  return {
    heroId: 64,
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

const art = (skinId: number) =>
  `https://game.gtimg.cn/images/lol/act/img/skinloading/${skinId}.jpg`;

/* Four rows of the real table, which is what the fields the page draws are read from. */
const blindMonk = skin({ skinId: 64000, name: "盲僧", isBase: true, emblemsName: "base" });

const dragonFist = skin({
  skinId: 64003,
  name: "龙的传人 李青",
  emblemsName: "Year of the Dragon",
});

const dragonFistChroma = skin({
  skinId: 64007,
  name: "龙的传人 李青 飞龙在天",
  isChroma: true,
  parentId: 64003,
});

const godFist = skin({
  skinId: 64004,
  name: "至高之拳 李青",
  emblemsName: "Epic",
  quality: "epic",
});

const dragonFistLegend = skin({
  skinId: 64011,
  name: "神拳 李青",
  emblemsName: "Legend",
  quality: "legend",
});

const leeSinSkins: CnHeroSkins = {
  version: "16.19",
  skins: [
    { ...blindMonk, artUrl: art(64000) },
    { ...dragonFist, artUrl: art(64003) },
    { ...dragonFistChroma, artUrl: art(64007) },
    { ...godFist, artUrl: art(64004) },
    { ...dragonFistLegend, artUrl: art(64011) },
  ],
};

/** A record this machine wrote into Lee Sin's archive, showing the game's skin `number`. */
function swap(
  number: number,
  state: ChampionSwap["state"] = "applied",
  /** The skin it was last given, which a record that was put back or forgotten still carries. */
  last: number | null = number,
): ChampionSwap {
  return {
    character: "leesin",
    archive: "Champions/LeeSin.wad.client",
    state,
    skin: state === "applied" ? number : null,
    lastSkin: last,
  };
}

interface MountOptions {
  hero?: CnHero | null;
  quality?: CnQuality | null;
  onlyApplied?: boolean;
  /** The record the backend holds for this champion, or none. */
  record?: ChampionSwap | null;
  /** The table the skin read answers with, for a champion the rows above do not cover. */
  skins?: CnHeroSkins;
  /** Whether the skin read fails, which is the CDN being unreachable. */
  skinsFail?: boolean;
}

function mount({
  hero = leeSin,
  quality = null,
  onlyApplied = false,
  record = null,
  skins = leeSinSkins,
  skinsFail = false,
}: MountOptions = {}) {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === "get_cn_static_swaps") {
      return Promise.resolve({ ok: true, value: record === null ? [] : [record] });
    }

    return Promise.resolve(
      skinsFail
        ? { ok: false, error: { code: "IO", detail: "the CDN did not answer" } }
        : { ok: true, value: skins },
    );
  });

  render(
    <QueryClientProvider client={createTestQueryClient()}>
      <ToastProvider>
        <CnSkinGrid hero={hero} quality={quality} onlyApplied={onlyApplied} />
        <ConfirmHost />
      </ToastProvider>
    </QueryClientProvider>,
  );
}

/** The command names the frontend asked the backend for, in order. */
function commandNames() {
  return mockInvoke.mock.calls.map(([command]) => command);
}

/** The tier each card drew, in the order the cards stand. */
function drawnTiers() {
  return [...document.querySelectorAll('[data-ui="cn-quality-badge"]')].map(
    (badge) => badge.textContent,
  );
}

/** The card titles the grid drew, in the order they stand. */
function drawnNames() {
  return [...document.querySelectorAll("li[class] > span[title]")].map((title) =>
    title.getAttribute("title"),
  );
}

/** The card `name` titles, so a control can be found on the card it belongs to. */
function cardFor(name: string): HTMLElement {
  const card = screen.getByText(name).closest("li");
  if (!card) throw new Error(`no card titled ${name}`);
  return card;
}

describe("CnSkinGrid", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  /* The champion's own archive is what holds its swap, so what the record saves is the
     question of which cards are applied rather than whether a champion is picked. */
  it("asks for no skins before a champion is picked", async () => {
    mount({ hero: null });

    expect(await screen.findByText("Pick a champion to list its skins")).toBeInTheDocument();
    expect(commandNames()).not.toContain("get_cn_hero_skins");
  });

  it("lists the champion's skins, with the base one named as such", async () => {
    mount();

    expect(await screen.findByText("盲僧")).toBeInTheDocument();
    expect(screen.getByText("龙的传人 李青")).toBeInTheDocument();
    expect(screen.getByText("Base skin")).toBeInTheDocument();
  });

  /* The table lists a skin's chromas beside the skins and names the skin each varies, so a
     chroma is a card's own control rather than a card of its own. */
  it("folds a chroma under the skin it varies", async () => {
    mount();

    const chroma = await screen.findByRole("button", { name: "Chroma 龙的传人 李青 飞龙在天" });
    expect(chroma).toBeInTheDocument();
    /* The name lives on the chroma's own control rather than as a card's title. */
    expect(drawnNames()).not.toContain("龙的传人 李青 飞龙在天");
  });

  /* The tier is what the page's filter narrows by, and the client's own label is where it
     comes from rather than anything guessed from the name. The base skin is not a tier, so its
     card draws the label it has instead of one. */
  it("labels each card with the tier the client gave it", async () => {
    mount();

    expect(await screen.findByText("至高之拳 李青")).toBeInTheDocument();
    expect(drawnTiers()).toEqual(["Standard", "Epic", "Legendary"]);
  });

  it("says so when the page's tier names no skin of this champion", async () => {
    mount({ quality: "ultimate" });

    expect(
      await screen.findByText("No skin of this champion matches the filters"),
    ).toBeInTheDocument();
  });

  it("keeps the skins the page's tier names", async () => {
    mount({ quality: "legend" });

    expect(await screen.findByText("神拳 李青")).toBeInTheDocument();
    expect(drawnNames()).toEqual(["神拳 李青"]);
  });

  it("says so when the champion has no skins in the table", async () => {
    mount({ skins: { version: "16.19", skins: [] } });

    expect(
      await screen.findByText("This champion has no skins in the Chinese client's table"),
    ).toBeInTheDocument();
  });

  /* A CDN that cannot be read is the failure this screen is most likely to meet,
     so it names itself rather than leaving an empty panel. */
  it("names the read when the skin table cannot be fetched", async () => {
    mount({ skinsFail: true });

    expect(await screen.findByText("Couldn't read this champion's skins")).toBeInTheDocument();
  });

  /* The record holds the game's own skin number, so the card it marks is the catalog entry the
     hero's thousand back on top of that names. */
  it("marks the skin the record holds as applied", async () => {
    mount({ record: swap(4) });

    expect(await screen.findByText("Applied")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Use the champion's own skin" })).toBeEnabled();
    /* Applied says it already, so the memory beside it would be the same thing twice. */
    expect(screen.queryByText("Last used")).not.toBeInTheDocument();
  });

  /* Putting the archive back ends the swap but not the memory, so the skin the champion was
     last given is offered again with one press instead of being picked out of the table. */
  it("remembers the skin a champion was last given after the archive is put back", async () => {
    mount({ record: swap(4, "untouched") });

    expect(await screen.findByText("Last used")).toBeInTheDocument();
    expect(within(cardFor("至高之拳 李青")).getByText("Last used")).toBeInTheDocument();
    expect(screen.queryByText("Applied")).not.toBeInTheDocument();
    /* Nothing is written, so nothing went wrong, and no notice is raised. */
    expect(screen.queryByText(/The client put its own archive back/)).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Use the champion's own skin" })).toBeDisabled();
  });

  /* A remembered chroma is what the champion would wear again, so it is the card that carries
     the mark, showing the chroma. */
  it("remembers a chroma the champion was given under the skin it varies", async () => {
    mount({ record: swap(7, "untouched") });

    expect(await screen.findByText("Last used")).toBeInTheDocument();
    expect(drawnNames()).toContain("龙的传人 李青 飞龙在天");
  });

  it("says nothing about a champion nothing was ever written into", async () => {
    mount({ record: swap(4, "drifted", null) });

    expect(
      await screen.findByText(/The archive is not in the state the record left it in/),
    ).toBeInTheDocument();
    expect(screen.queryByText("Last used")).not.toBeInTheDocument();
  });

  /* A chroma is what the champion actually wears, so its card shows the chroma and carries the
     applied mark rather than a card of its own. */
  it("marks the card a chroma the record holds belongs to", async () => {
    mount({ record: swap(7) });

    expect(await screen.findByText("Applied")).toBeInTheDocument();
    expect(drawnNames()).toContain("龙的传人 李青 飞龙在天");
  });

  /* Nothing is applied yet, so the champion's own card has nothing to put back. */
  it("offers no way back to a base skin that is already showing", async () => {
    mount();

    expect(await screen.findByText("龙的传人 李青")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Use the champion's own skin" })).toBeDisabled();
  });

  it("lists only the champion's applied skin under the filter", async () => {
    mount({ onlyApplied: true, record: swap(4) });

    expect(await screen.findByText("至高之拳 李青")).toBeInTheDocument();
    expect(drawnNames()).toEqual(["至高之拳 李青"]);
  });

  /* Nothing applied is nothing to keep rather than everything, which is what the toggle would
     otherwise mean while the champion still wears its own skin. */
  it("lists nothing under that filter while nothing is applied", async () => {
    mount({ onlyApplied: true });

    expect(
      await screen.findByText("No skin of this champion matches the filters"),
    ).toBeInTheDocument();
  });

  /* A patch or a repair puts the client's own archive back, which leaves the record naming
     nothing. The swap is gone, and the page says so rather than reading it as applied. */
  it("reads a record the client put back as nothing applied", async () => {
    mount({ record: swap(4, "reverted") });

    expect(
      await screen.findByText(/The client put its own archive back, so this swap is gone/),
    ).toBeInTheDocument();
    expect(screen.queryByText("Applied")).not.toBeInTheDocument();
  });

  /* The one state that needs an action rather than a word: neither a write nor a restore is
     safe until the record is dropped, so the cards are held and the way out is offered. */
  it("holds the cards and offers to forget a record the archive drifted from", async () => {
    mount({ record: swap(4, "drifted") });

    expect(
      await screen.findByText(/The archive is not in the state the record left it in/),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Forget the record" })).toBeInTheDocument();
    for (const button of screen.getAllByRole("button", { name: "Apply" })) {
      expect(button).toBeDisabled();
    }
  });

  it("offers to forget a record whose backup is gone", async () => {
    mount({ record: swap(4, "backupMissing") });

    expect(await screen.findByText(/The backup this record names is gone/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Forget the record" })).toBeInTheDocument();
  });

  it("asks first, then writes the skin into the archive", async () => {
    const user = userEvent.setup();
    mount();
    await screen.findByText("至高之拳 李青");

    await user.click(within(cardFor("至高之拳 李青")).getByRole("button", { name: "Apply" }));

    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "Apply" }));

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("apply_cn_static_swap", {
        character: "LeeSin",
        heroId: 64,
        skinId: 64004,
      }),
    );
  });

  /* The base skin's card is the way back, which is the backend's own restore rather than a
     write of anything. */
  it("puts the champion's own archive back from its base skin's card", async () => {
    const user = userEvent.setup();
    mount({ record: swap(4) });
    await screen.findByText("Applied");

    await user.click(
      within(cardFor("盲僧")).getByRole("button", { name: "Use the champion's own skin" }),
    );

    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith("restore_cn_static_swap", {
        character: "LeeSin",
      }),
    );
  });
});
