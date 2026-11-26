import { SpinnerGapIcon } from "@phosphor-icons/react";
import { createRootRoute, Outlet, useLocation, useNavigate } from "@tanstack/react-router";
import { Suspense, useEffect } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { useOverscrollSpring, useReducedMotion, useZoomHotkeys } from "@/hooks";
import {
  loadMonoFace,
  loadSansFace,
  monoStack,
  sansStack,
  sansWeights,
  WEIGHT_TIERS,
} from "@/lib/fonts";
import { ProtocolInstallDialogLazy, useDeepLinkListener } from "@/modules/deep-link";
import { useCleanGameWatch, useIncidentListeners } from "@/modules/diagnostics";
import {
  InstallMismatchDialog,
  useInstallMismatchWatch,
  useLeagueSession,
} from "@/modules/launcher";
import {
  LibraryMigrationDialog,
  ModHealthSweepListener,
  useLibraryWatcher,
  useModStorageToast,
} from "@/modules/library";
import { useAppInfo, useCheckSetupRequired } from "@/modules/settings";
import {
  DevConsoleLazy,
  OfficialFooterBar,
  OfficialNoticeDialog,
  TitleBar,
  useDevLogStream,
} from "@/modules/shell";
import { useDisplayStore } from "@/stores";

/** `promise`, resolving either way, so a failed load still runs what follows it. */
function settled(promise: Promise<unknown>): Promise<void> {
  return promise.then(
    () => undefined,
    (reason: unknown) => {
      console.error("Font face failed to load", reason);
    },
  );
}

function RootLayout() {
  const { data: appInfo } = useAppInfo();
  const navigate = useNavigate();
  const location = useLocation();

  const { data: setupRequired, isLoading: isCheckingSetup } = useCheckSetupRequired();
  const isOverlayWindow = location.pathname === "/cn-overlay";

  const zoomLevel = useDisplayStore((s) => s.zoomLevel);
  const cornerStyle = useDisplayStore((s) => s.cornerStyle);
  const sansFont = useDisplayStore((s) => s.sansFont);
  const monoFont = useDisplayStore((s) => s.monoFont);
  const surfaceTint = useDisplayStore((s) => s.surfaceTint);
  const cardScale = useDisplayStore((s) => s.cardScale);
  const scrollMode = useDisplayStore((s) => s.scrollMode);
  const scrollbarSize = useDisplayStore((s) => s.scrollbarSize);
  const isReducedMotion = useReducedMotion();

  useDevLogStream();
  useDeepLinkListener();
  useLibraryWatcher();
  useModStorageToast();
  useIncidentListeners();
  useCleanGameWatch();
  useLeagueSession();
  useInstallMismatchWatch();
  useOverscrollSpring();
  useZoomHotkeys();

  useEffect(() => {
    document.documentElement.style.setProperty("--zoom-scale", String(zoomLevel / 100));
  }, [zoomLevel]);

  useEffect(() => {
    document.documentElement.dataset.corners = cornerStyle;
  }, [cornerStyle]);

  /* The family is applied after the face is registered, so a lazily loaded face
     never draws a frame in the fallback stack. A face whose chunk does not
     arrive is applied anyway and falls back, which reads better than a stale
     family the reader did not choose. */
  useEffect(() => {
    let current = true;
    void settled(loadSansFace(sansFont)).then(() => {
      if (!current) return;
      const root = document.documentElement;
      root.style.setProperty("--face-sans", sansStack(sansFont));
      /* Every tier is written or cleared, so the face before this one leaves
         nothing of its own behind. */
      const weights = sansWeights(sansFont);
      for (const tier of WEIGHT_TIERS) {
        const weight = weights[tier];
        if (weight === undefined) root.style.removeProperty(`--weight-${tier}`);
        else root.style.setProperty(`--weight-${tier}`, String(weight));
      }
    });
    return () => {
      current = false;
    };
  }, [sansFont]);

  useEffect(() => {
    let current = true;
    void settled(loadMonoFace(monoFont)).then(() => {
      if (!current) return;
      document.documentElement.style.setProperty("--face-mono", monoStack(monoFont));
    });
    return () => {
      current = false;
    };
  }, [monoFont]);

  useEffect(() => {
    document.documentElement.style.setProperty("--surface-tint", String(surfaceTint / 100));
  }, [surfaceTint]);

  useEffect(() => {
    document.documentElement.style.setProperty("--card-scale", String(cardScale / 100));
  }, [cardScale]);

  useEffect(() => {
    document.documentElement.dataset.reduceMotion = String(isReducedMotion);
  }, [isReducedMotion]);

  useEffect(() => {
    document.documentElement.dataset.scrollMode = scrollMode;
  }, [scrollMode]);

  useEffect(() => {
    document.documentElement.dataset.scrollbars = scrollbarSize;
  }, [scrollbarSize]);

  useHotkeys("ctrl+1", () => navigate({ to: "/cn" }), { preventDefault: true });
  useHotkeys("ctrl+d", () => navigate({ to: "/diagnostics", search: { tab: "games" } }), {
    preventDefault: true,
  });
  useHotkeys("ctrl+,", () => navigate({ to: "/settings" }), { preventDefault: true });
  // Redirect to settings if setup is required
  useEffect(() => {
    if (!isOverlayWindow && setupRequired && location.pathname !== "/settings") {
      navigate({ to: "/settings", search: { firstRun: true } });
    }
  }, [isOverlayWindow, setupRequired, navigate, location.pathname]);

  if (isOverlayWindow) {
    return <Outlet />;
  }

  // Show loading state while checking setup
  if (isCheckingSetup) {
    return (
      <div className="flex h-screen items-center justify-center bg-linear-to-br from-surface-950 via-surface-900 to-surface-950">
        <SpinnerGapIcon className="h-6 w-6 animate-spin text-surface-400" />
      </div>
    );
  }

  return (
    <div className="root flex h-screen flex-col bg-surface-950">
      <TitleBar appInfo={appInfo} />
      <main className="relative flex-1 overflow-hidden">
        <div className="h-full">
          <Outlet />
        </div>
      </main>
      <OfficialFooterBar />
      <LibraryMigrationDialog />
      <ModHealthSweepListener />
      <InstallMismatchDialog />
      <OfficialNoticeDialog />
      <Suspense fallback={null}>
        <ProtocolInstallDialogLazy />
        {import.meta.env.DEV && <DevConsoleLazy />}
      </Suspense>
    </div>
  );
}

export const Route = createRootRoute({
  component: RootLayout,
});
