// @vitest-environment happy-dom

import { QueryClientProvider } from "@tanstack/react-query";
import { render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ConfirmHost, ToastProvider } from "@/components";
import { m } from "@/i18n";
import type { PackSwap } from "@/lib/tauri";
import { mockInvoke } from "@/test/mocks/tauri";
import { createTestQueryClient } from "@/test/utils";

import { CnSkinPacks } from "../CnSkinPacks";

/** The file the OS picker answers with, or none where the reader cancelled it. */
const pickFile = vi.fn<() => Promise<string | null>>();
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: () => pickFile() }));

/** One archive a pack has been written over, everything unnamed left at its base value. */
function pack(overrides: Partial<PackSwap> = {}): PackSwap {
  return {
    archive: "Champions/Annie.wad.client",
    pack: "Annie.fantome",
    state: "applied",
    bytes: 1024,
    ...overrides,
  };
}

function mount(records: PackSwap[] = []) {
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === "get_cn_skin_packs") return Promise.resolve({ ok: true, value: records });
    return Promise.resolve({ ok: true, value: records });
  });

  render(
    <QueryClientProvider client={createTestQueryClient()}>
      <ToastProvider>
        <CnSkinPacks />
        <ConfirmHost />
      </ToastProvider>
    </QueryClientProvider>,
  );
}

/** The command names the frontend asked the backend for, in order. */
function commandNames() {
  return mockInvoke.mock.calls.map(([command]) => command);
}

describe("CnSkinPacks", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    pickFile.mockReset();
  });

  it("says so when no pack has been written", async () => {
    mount([]);

    expect(await screen.findByText(m.cn_packs_empty())).toBeInTheDocument();
  });

  it("writes a pack the reader picked, once confirmed", async () => {
    pickFile.mockResolvedValue("/tmp/Annie.fantome");
    mount([]);
    await screen.findByText(m.cn_packs_empty());

    await userEvent.click(screen.getByRole("button", { name: m.cn_pack_import_action() }));

    /* The confirmation is the only gate between a third-party archive and the game's own, so the
       write does not start until it is accepted. */
    const dialog = await screen.findByRole("dialog");
    expect(commandNames()).not.toContain("import_cn_skin_pack");

    await userEvent.click(within(dialog).getByRole("button", { name: m.cn_pack_import_action() }));

    await waitFor(() => expect(commandNames()).toContain("import_cn_skin_pack"));
    expect(screen.getByText(m.cn_pack_applied_title())).toBeInTheDocument();
  });

  it("writes nothing when the confirmation is turned down", async () => {
    pickFile.mockResolvedValue("/tmp/Annie.fantome");
    mount([]);
    await screen.findByText(m.cn_packs_empty());

    await userEvent.click(screen.getByRole("button", { name: m.cn_pack_import_action() }));
    const dialog = await screen.findByRole("dialog");
    await userEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(commandNames()).not.toContain("import_cn_skin_pack");
  });

  it("puts a written archive back", async () => {
    mount([pack()]);
    await screen.findByText("Champions/Annie.wad.client");

    await userEvent.click(screen.getByRole("button", { name: m.cn_pack_restore_action() }));

    await waitFor(() => expect(commandNames()).toContain("restore_cn_skin_pack"));
  });

  /* A record the archive no longer agrees with refuses both a write and a restore, so the row
     stays shut and the page says why. */
  it("shuts a row whose archive the game moved under it", async () => {
    mount([pack({ state: "drifted" })]);
    await screen.findByText(m.cn_pack_drifted_notice());

    expect(screen.getByRole("button", { name: m.cn_pack_restore_action() })).toBeDisabled();
    expect(screen.getByRole("button", { name: m.cn_pack_forget_action() })).toBeEnabled();
  });

  it("has nothing to fetch until a link is pasted", async () => {
    mount([]);
    await screen.findByText(m.cn_packs_empty());

    const download = screen.getByRole("button", { name: m.cn_pack_download_action() });
    expect(download).toBeDisabled();

    await userEvent.type(
      screen.getByLabelText(m.cn_pack_url_placeholder()),
      "https://github.com/a/b/Annie.fantome",
    );

    expect(download).toBeEnabled();
  });

  it("fetches a pack from a pasted link, once confirmed", async () => {
    mount([]);
    await screen.findByText(m.cn_packs_empty());

    await userEvent.type(
      screen.getByLabelText(m.cn_pack_url_placeholder()),
      "https://github.com/a/b/Annie.fantome",
    );
    await userEvent.click(screen.getByRole("button", { name: m.cn_pack_download_action() }));

    const dialog = await screen.findByRole("dialog");
    await userEvent.click(
      within(dialog).getByRole("button", { name: m.cn_pack_download_action() }),
    );

    await waitFor(() => expect(commandNames()).toContain("download_cn_skin_pack"));
  });
});
