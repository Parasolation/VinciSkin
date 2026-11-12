// @vitest-environment happy-dom

import { render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import type { CnHero } from "@/lib/tauri";

import { CnHeroPicker } from "../CnHeroPicker";

/** A champion of the client's roster, with a portrait unless a test says otherwise. */
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
    keywords: ["安妮", "黑暗之女", "火女", "Annie", "anni", "hn"],
  }),
  hero({
    id: 103,
    alias: "Ahri",
    name: "九尾妖狐",
    keywords: ["九尾", "九尾妖狐", "狐狸", "Ahri", "jiuwei"],
  }),
  hero({ id: 64, alias: "LeeSin", name: "盲僧", portraitUrl: null }),
];

/** The tile a champion's tile is, found by the label its title carries. */
function tile(label: string) {
  return screen.getByTitle(label);
}

function mount(picked = "") {
  const onPick = vi.fn();
  render(<CnHeroPicker heroes={roster} picked={picked} onPick={onPick} />);

  return { onPick };
}

describe("CnHeroPicker", () => {
  it("draws a tile per champion, the Chinese title on it and the portrait beside it", () => {
    mount();

    const annie = tile("黑暗之女 (Annie)");
    expect(within(annie).getByText("黑暗之女")).toBeInTheDocument();
    expect(annie.querySelector("img")).toHaveAttribute(
      "src",
      "https://game.gtimg.cn/images/lol/act/img/champion/Annie.png",
    );
  });

  /* A table that spells an alias with something other than letters and digits names no portrait,
     so the tile falls back to the first character of the title rather than drawing nothing. */
  it("falls back to the title's first character where no portrait was named", () => {
    mount();

    const leeSin = tile("盲僧 (LeeSin)");
    expect(leeSin.querySelector("img")).toBeNull();
    expect(within(leeSin).getByText("盲")).toBeInTheDocument();
  });

  it("narrows the wall to what the search matches", async () => {
    mount();

    await userEvent.type(screen.getByPlaceholderText("Search champions"), "九尾");

    expect(screen.getAllByRole("button")).toHaveLength(1);
    expect(tile("九尾妖狐 (Ahri)")).toBeInTheDocument();
  });

  /* A player reaches a champion by its nickname or its pinyin as readily as by its title, so
     the box accepts all of them. */
  it("narrows the wall to a nickname and to a pinyin spelling alike", async () => {
    mount();

    await userEvent.clear(screen.getByPlaceholderText("Search champions"));
    await userEvent.type(screen.getByPlaceholderText("Search champions"), "安妮");
    expect(within(screen.getAllByRole("button")[0]!).getByText("黑暗之女")).toBeInTheDocument();

    await userEvent.clear(screen.getByPlaceholderText("Search champions"));
    await userEvent.type(screen.getByPlaceholderText("Search champions"), "jiuwei");
    expect(within(screen.getAllByRole("button")[0]!).getByText("九尾妖狐")).toBeInTheDocument();
  });

  it("says so rather than drawing nothing when no champion matches", async () => {
    mount();

    await userEvent.type(screen.getByPlaceholderText("Search champions"), "亚索");

    expect(screen.getByText("No champion matches")).toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("reports the champion a tile stands for", async () => {
    const { onPick } = mount();

    await userEvent.click(tile("九尾妖狐 (Ahri)"));

    expect(onPick).toHaveBeenCalledWith("103");
  });

  it("marks the champion the page already holds", () => {
    mount("64");

    expect(tile("盲僧 (LeeSin)")).toHaveAttribute("aria-pressed", "true");
    expect(tile("黑暗之女 (Annie)")).toHaveAttribute("aria-pressed", "false");
  });
});
