// @vitest-environment happy-dom

import { QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it } from "vitest";

import type { ChampionSwap, CnHeroRoster, CnHeroSkins } from "@/lib/tauri";
import { mockInvoke } from "@/test/mocks/tauri";
import { createTestQueryClient } from "@/test/utils";

import {
  useApplyCnSkin,
  useCnHeroes,
  useCnHeroSkins,
  useForgetCnSwap,
  useHeroSwap,
  useRestoreCnSkin,
  useStaticSwaps,
} from "../useCnCatalog";

const roster: CnHeroRoster = {
  version: "16.19",
  heroes: [
    {
      id: 1,
      alias: "Annie",
      name: "黑暗之女",
      keywords: ["安妮", "黑暗之女", "火女", "Annie", "anni"],
      portraitUrl: "https://game.gtimg.cn/images/lol/act/img/champion/Annie.png",
    },
    {
      id: 64,
      alias: "LeeSin",
      name: "盲僧",
      keywords: ["盲僧", "瞎子", "李青", "LeeSin", "mangseng"],
      portraitUrl: "https://game.gtimg.cn/images/lol/act/img/champion/LeeSin.png",
    },
  ],
};

const leeSinSkins: CnHeroSkins = {
  version: "16.19",
  skins: [
    {
      skinId: 64000,
      heroId: 64,
      name: "盲僧",
      isBase: true,
      isChroma: false,
      parentId: 64000,
      quality: "common",
      emblemsName: "base",
      artUrl: "https://game.gtimg.cn/images/lol/act/img/skinloading/64000.jpg",
      instanceId: null,
    },
    {
      skinId: 64001,
      heroId: 64,
      name: "传统僧侣",
      isBase: false,
      isChroma: false,
      parentId: 64001,
      quality: "epic",
      emblemsName: "Epic",
      artUrl: "https://game.gtimg.cn/images/lol/act/img/skinloading/64001.jpg",
      instanceId: null,
    },
  ],
};

const leeSinSwap: ChampionSwap = {
  character: "leesin",
  archive: "Champions/LeeSin.wad.client",
  state: "applied",
  skin: 1,
  lastSkin: 1,
};

function wrapper({ children }: { children: ReactNode }) {
  return <QueryClientProvider client={createTestQueryClient()}>{children}</QueryClientProvider>;
}

function Roster() {
  const { data, isPending } = useCnHeroes();

  if (isPending || !data) return <div data-testid="roster-pending" />;
  return <div data-testid="patch">{data.version}</div>;
}

function Skins({ heroId }: { heroId: number | null }) {
  const { data } = useCnHeroSkins(heroId);

  return <div data-testid="skins">{data ? String(data.skins.length) : "none"}</div>;
}

function Swaps() {
  const { data } = useStaticSwaps();

  return (
    <div data-testid="swaps">{data ? data.map((swap) => swap.character).join(",") : "none"}</div>
  );
}

function HeroSwap({ alias }: { alias: string | null }) {
  const swap = useHeroSwap(alias);

  return <div data-testid="hero-swap">{swap ? String(swap.skin) : "none"}</div>;
}

/** The command names the frontend asked the backend for, in order. */
function commands() {
  return mockInvoke.mock.calls.map(([command]) => command);
}

describe("the Chinese catalog queries", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation((command: string) => {
      if (command === "get_cn_heroes") return Promise.resolve({ ok: true, value: roster });
      if (command === "get_cn_hero_skins") return Promise.resolve({ ok: true, value: leeSinSkins });
      if (command === "get_cn_static_swaps") {
        return Promise.resolve({ ok: true, value: [leeSinSwap] });
      }

      return Promise.resolve({ ok: true, value: null });
    });
  });

  it("reads the roster from the one command that serves it", async () => {
    render(<Roster />, { wrapper });

    expect(await screen.findByTestId("patch")).toHaveTextContent("16.19");
    expect(commands()).toEqual(["get_cn_heroes"]);
  });

  /* The CDN serves one table per champion and no combined list, so a champion
     that has not been picked must not cost a request. */
  it("asks for no skins while no champion is picked", async () => {
    render(<Skins heroId={null} />, { wrapper });

    expect(await screen.findByTestId("skins")).toHaveTextContent("none");
    expect(commands()).toEqual([]);
  });

  it("asks for the picked champion's skins, and names that champion", async () => {
    render(<Skins heroId={64} />, { wrapper });

    await waitFor(() => expect(screen.getByTestId("skins")).toHaveTextContent("2"));
    expect(mockInvoke).toHaveBeenCalledWith("get_cn_hero_skins", { heroId: 64 });
  });

  /* A second mount inside the stale window is served from the cache, which is
     what keeps a revisit from spending another round trip. */
  it("serves a repeated read of one champion from the cache", async () => {
    const queryClient = createTestQueryClient();
    function Shared({ children }: { children: ReactNode }) {
      return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
    }

    const first = render(<Skins heroId={64} />, { wrapper: Shared });
    await screen.findByText("2");
    first.unmount();

    render(<Skins heroId={64} />, { wrapper: Shared });
    await screen.findByText("2");

    await waitFor(() =>
      expect(commands().filter((command) => command === "get_cn_hero_skins")).toHaveLength(1),
    );
  });

  it("reads what this machine has written into the game", async () => {
    render(<Swaps />, { wrapper });

    await waitFor(() => expect(screen.getByTestId("swaps")).toHaveTextContent("leesin"));
    expect(commands()).toEqual(["get_cn_static_swaps"]);
  });

  /* A record is keyed by the character's lowercased name, which is what the
     hero's alias is once folded the same way. */
  it("finds a champion's record by its alias", async () => {
    render(<HeroSwap alias="LeeSin" />, { wrapper });

    await waitFor(() => expect(screen.getByTestId("hero-swap")).toHaveTextContent("1"));
  });

  it("finds no record for a champion that has none", async () => {
    render(<HeroSwap alias="Annie" />, { wrapper });

    await waitFor(() => expect(commands()).toContain("get_cn_static_swaps"));
    expect(screen.getByTestId("hero-swap")).toHaveTextContent("none");
  });
});

/** What a write's button says, so a test can follow it from idle to its answer. */
function label(isPending: boolean, isError: boolean, isSuccess: boolean) {
  return isPending ? "saving" : isError ? "failed" : isSuccess ? "saved" : "idle";
}

/** A button that writes one skin into a champion's archive. */
function Apply({ pick }: { pick: { alias: string; heroId: number; skinId: number } }) {
  const apply = useApplyCnSkin();

  return (
    <button type="button" onClick={() => apply.mutate(pick)}>
      {label(apply.isPending, apply.isError, apply.isSuccess)}
    </button>
  );
}

/** A button that puts one champion's archive back. */
function Restore({ alias }: { alias: string }) {
  const restore = useRestoreCnSkin();

  return (
    <button type="button" onClick={() => restore.mutate({ alias })}>
      {label(restore.isPending, restore.isError, restore.isSuccess)}
    </button>
  );
}

/** A button that drops one champion's record. */
function Forget({ alias }: { alias: string }) {
  const forget = useForgetCnSwap();

  return (
    <button type="button" onClick={() => forget.mutate({ alias })}>
      {label(forget.isPending, forget.isError, forget.isSuccess)}
    </button>
  );
}

describe("writing into a champion's archive", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockImplementation(() => Promise.resolve({ ok: true, value: leeSinSwap }));
  });

  it("names the champion, its hero and the catalog's skin id", async () => {
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={createTestQueryClient()}>
        <Apply pick={{ alias: "LeeSin", heroId: 64, skinId: 64001 }} />
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole("button"));

    await waitFor(() => expect(screen.getByRole("button")).toHaveTextContent("saved"));
    expect(mockInvoke).toHaveBeenCalledWith("apply_cn_static_swap", {
      character: "LeeSin",
      heroId: 64,
      skinId: 64001,
    });
  });

  /* A skin the install holds no file for is refused by the backend, and the
     screen has to say the write failed rather than showing it as applied. */
  it("reports a refused write as a failure", async () => {
    mockInvoke.mockResolvedValue({
      ok: false,
      error: { code: "VALIDATION_FAILED", detail: "the install holds no skin 21 of annie" },
    });
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={createTestQueryClient()}>
        <Apply pick={{ alias: "Annie", heroId: 1, skinId: 1021 }} />
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole("button"));

    await waitFor(() => expect(screen.getByRole("button")).toHaveTextContent("failed"));
  });

  it("names the champion whose archive is put back", async () => {
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={createTestQueryClient()}>
        <Restore alias="LeeSin" />
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole("button"));

    await waitFor(() => expect(screen.getByRole("button")).toHaveTextContent("saved"));
    expect(mockInvoke).toHaveBeenCalledWith("restore_cn_static_swap", { character: "LeeSin" });
  });

  /* Forgetting touches no archive, which is why it is the way out of a record
     the file no longer agrees with. */
  it("names the champion whose record is dropped", async () => {
    const user = userEvent.setup();
    render(
      <QueryClientProvider client={createTestQueryClient()}>
        <Forget alias="LeeSin" />
      </QueryClientProvider>,
    );

    await user.click(screen.getByRole("button"));

    await waitFor(() => expect(screen.getByRole("button")).toHaveTextContent("saved"));
    expect(mockInvoke).toHaveBeenCalledWith("forget_cn_static_swap", { character: "LeeSin" });
  });
});
