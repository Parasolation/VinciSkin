// @vitest-environment happy-dom

import { QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, type Mock } from "vitest";

import { ToastProvider } from "@/components";
import type { Hint, Incident } from "@/lib/tauri";
import { createMockIncident } from "@/modules/diagnostics/components/__tests__/fixtures";
import { settingsKeys } from "@/modules/settings";
import { useInstallMismatchStore } from "@/stores";
import { createMockSettings } from "@/test/fixtures";
import { mockInvoke, mockListen } from "@/test/mocks/tauri";
import { createTestQueryClient } from "@/test/utils";

import { useIncidentListeners } from "../useIncidentListeners";

vi.mock("@tanstack/react-router", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@tanstack/react-router")>()),
  useNavigate: () => vi.fn(),
}));

type Handler = (event: { payload: unknown }) => void;

const handlers = new Map<string, Handler[]>();

async function emit(name: string, payload: unknown) {
  await act(async () => {
    for (const handler of handlers.get(name) ?? []) handler({ payload });
  });
}

function incident(hints: Hint[]): Incident {
  return createMockIncident({
    redirected: ["Shaders.wad.client"],
    game: null,
    suspects: [],
    verdict: {
      ...createMockIncident().verdict,
      kind: "corrupt-archive",
      title: "WAD Mount Failure",
      cause: "League could not mount Shaders.wad.client.",
      subject: "Shaders.wad.client",
      hints,
    },
  });
}

function Listeners() {
  useIncidentListeners();
  return <div data-testid="listeners" />;
}

function mockBackend() {
  const settings = createMockSettings({
    leaguePath: "C:\\Riot Games\\League of Legends (PBE)",
  });
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === "get_settings") return Promise.resolve({ ok: true, value: settings });
    return Promise.resolve({ ok: true, value: null });
  });
  return settings;
}

async function mount() {
  const settings = mockBackend();
  const queryClient = createTestQueryClient();
  /* Seeded rather than fetched, so the verdict's handler reads the configured
     install the moment it runs. */
  queryClient.setQueryData(settingsKeys.settings(), settings);
  function wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>
        <ToastProvider>{children}</ToastProvider>
      </QueryClientProvider>
    );
  }
  render(<Listeners />, { wrapper });
  await waitFor(() => expect(handlers.has("incident-recorded")).toBe(true));
}

describe("useIncidentListeners", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    handlers.clear();
    useInstallMismatchStore.setState({ mismatch: null, kept: false });
    (mockListen as Mock).mockImplementation((name: string, handler: Handler) => {
      handlers.set(name, [...(handlers.get(name) ?? []), handler]);
      return Promise.resolve(() => {});
    });
  });

  it("announces the incident with a Details toast", async () => {
    await mount();

    await emit("incident-recorded", incident(["check-game-path"]));

    expect(await screen.findByText("WAD Mount Failure")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Details" })).toBeInTheDocument();
    /* Nothing builds an overlay now, so no verdict offers a rebuild. */
    expect(screen.queryByRole("button", { name: /Rebuild/ })).not.toBeInTheDocument();
  });

  /// The log is the backstop for a client that did not answer, so the verdict
  /// raises the same dialog the client check does, with the log's install.
  it("raises the install mismatch dialog on a wrong-install verdict", async () => {
    await mount();
    const wrongInstall = createMockIncident({
      verdict: {
        ...createMockIncident().verdict,
        kind: "wrong-install",
        title: "Wrong League Install",
        hints: ["check-game-path"],
      },
    });

    await emit("incident-recorded", wrongInstall);

    await waitFor(() =>
      expect(useInstallMismatchStore.getState().mismatch).toEqual({
        configuredPath: "C:/Riot Games/League of Legends (PBE)",
        configuredPatchline: null,
        sessionPath: "C:\\Riot Games\\League of Legends",
        sessionPatchline: null,
      }),
    );
    expect(screen.queryByRole("button", { name: /Rebuild/ })).not.toBeInTheDocument();
  });

  /// The verdict exists to say the patcher never ran; with no patcher there is
  /// nothing to announce, so the toast stays down.
  it("says nothing for a patcher-did-not-run verdict", async () => {
    await mount();
    const neverRan = createMockIncident({
      verdict: {
        ...createMockIncident().verdict,
        kind: "patcher-did-not-run",
        title: "Patcher Did Not Run",
        hints: [],
      },
    });

    await emit("incident-recorded", neverRan);

    expect(screen.queryByText("Patcher Did Not Run")).not.toBeInTheDocument();
  });
});
