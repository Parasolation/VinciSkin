// @vitest-environment happy-dom

import { QueryClientProvider } from "@tanstack/react-query";
import { act, renderHook, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it } from "vitest";

import { ToastProvider } from "@/components";
import type { LaunchRoute } from "@/lib/bindings";
import { createMockSettings } from "@/test/fixtures";
import { mockInvoke } from "@/test/mocks/tauri";
import { createTestQueryClient } from "@/test/utils";

import { usePlay } from "../usePlay";

function wrapper({ children }: { children: ReactNode }) {
  const queryClient = createTestQueryClient();
  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>{children}</ToastProvider>
    </QueryClientProvider>
  );
}

function renderPlay() {
  return renderHook(() => usePlay(), { wrapper });
}

/** Answers the commands a launch touches. */
function mockBackend(route: LaunchRoute = "EXISTING_CLIENT") {
  mockInvoke.mockImplementation((cmd: string) => {
    switch (cmd) {
      case "get_settings":
        // Already seen, so the HDD check short-circuits instead of probing disks.
        return Promise.resolve({
          ok: true,
          value: createMockSettings({ hasSeenHddWarning: true }),
        });
      case "launch_league":
        return Promise.resolve({
          ok: true,
          value: { route, riotClientPid: 1234, sessionId: null },
        });
      default:
        return Promise.resolve({ ok: true, value: null });
    }
  });
}

function invokedCommands() {
  return mockInvoke.mock.calls.map(([cmd]) => cmd as string);
}

describe("usePlay", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  /* A launch is a launch: this build writes the skin into the game's own
     archive, so nothing starts a patcher and the game wants no second process. */
  it("asks the Riot Client to launch, and starts no patcher", async () => {
    mockBackend();
    const { result } = renderPlay();

    await act(async () => {
      await result.current.launchOnly();
    });

    expect(invokedCommands()).toContain("launch_league");
    expect(invokedCommands()).not.toContain("start_patcher");
    expect(result.current.isBusy).toBe(false);
  });

  it("says when there was nothing to launch", async () => {
    mockBackend("ALREADY_RUNNING");
    const { result } = renderPlay();

    await act(async () => {
      await result.current.launchOnly();
    });

    expect(await screen.findByText("League is already running")).toBeInTheDocument();
  });

  it("says the manager is following a game it adopted", async () => {
    mockBackend("ADOPTED");
    const { result } = renderPlay();

    await act(async () => {
      await result.current.launchOnly();
    });

    expect(await screen.findByText(/following the game you already had open/)).toBeInTheDocument();
  });

  /* A refused launch is the backend's to explain, and the hook must not leave
     the button spinning on it. */
  it("reports a refused launch and stops being busy", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_settings") {
        return Promise.resolve({
          ok: true,
          value: createMockSettings({ hasSeenHddWarning: true }),
        });
      }
      return Promise.resolve({ ok: false, error: { code: "IO", detail: "no client" } });
    });
    const { result } = renderPlay();

    await act(async () => {
      await result.current.launchOnly();
    });

    expect(result.current.isBusy).toBe(false);
    expect(await screen.findByText("Couldn't launch League")).toBeInTheDocument();
  });

  /* The backend mutex is the real guarantee, but a second in-flight call must
     not reach it in the first place. */
  it("ignores a second launch while one is already running", async () => {
    mockBackend();
    const { result } = renderPlay();

    await act(async () => {
      await Promise.all([result.current.launchOnly(), result.current.launchOnly()]);
    });

    const launches = invokedCommands().filter((cmd) => cmd === "launch_league");
    expect(launches).toHaveLength(1);
  });
});
