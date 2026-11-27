import { CaretLeftIcon, CaretRightIcon, XIcon } from "@phosphor-icons/react";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useRef, useState } from "react";

import { Spinner } from "@/components";
import { errorSummary } from "@/i18n";
import { api, type CnSkin, isOk } from "@/lib/tauri";

import { useApplyCnSkin, useCnHeroes, useCnHeroSkins, useHeroSwap, useRestoreCnSkin } from "../api";
import { qualityLabel, skinCards } from "../skins";

const APPLY_DEBOUNCE_MS = 450;

/**
 * Floating champ-select skin picker docked beside the League client.
 *
 * Clicking a skin or chroma updates the preview immediately and debounces the
 * actual overlay write so rapid browsing never queues redundant rebuilds.
 */
export function CnOverlayWindow() {
  const [championId, setChampionId] = useState<number | null>(null);
  const [browsedSkinId, setBrowsedSkinId] = useState<number | null>(null);
  const [hoveredChroma, setHoveredChroma] = useState<CnSkin | null>(null);
  const [statusText, setStatusText] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<{ kind: "success" | "error"; text: string } | null>(
    null,
  );
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const feedbackTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const activeThumbRef = useRef<HTMLButtonElement | null>(null);

  const heroes = useCnHeroes();
  const hero = useMemo(
    () => heroes.data?.heroes.find((candidate) => candidate.id === championId) ?? null,
    [heroes.data, championId],
  );

  const skins = useCnHeroSkins(hero?.id ?? null);
  const swap = useHeroSwap(hero?.alias ?? null);
  const apply = useApplyCnSkin();
  const restore = useRestoreCnSkin();

  useEffect(() => {
    let alive = true;

    async function syncChampion() {
      const result = await api.cn.autoStatus();
      if (!alive || !isOk(result)) return;
      if (result.value.currentChampion) {
        setChampionId(result.value.currentChampion);
      }
    }

    void syncChampion();
    const pollId = setInterval(() => void syncChampion(), 1000);

    const unlistenPromise = listen<number>("cn-overlay-champion", (event) => {
      if (alive) {
        setChampionId(event.payload);
      }
    });

    return () => {
      alive = false;
      clearInterval(pollId);
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
      debounceRef.current = null;
    }
    if (feedbackTimerRef.current) {
      clearTimeout(feedbackTimerRef.current);
      feedbackTimerRef.current = null;
    }
    setBrowsedSkinId(null);
    setHoveredChroma(null);
    setStatusText(null);
    setFeedback(null);
  }, [championId]);

  useEffect(() => {
    return () => {
      if (debounceRef.current) {
        clearTimeout(debounceRef.current);
      }
      if (feedbackTimerRef.current) {
        clearTimeout(feedbackTimerRef.current);
      }
    };
  }, []);

  const cards = useMemo(() => {
    if (!skins.data) return [];
    return skinCards(skins.data.skins);
  }, [skins.data]);

  const appliedSkinId =
    hero !== null && swap?.state === "applied" && swap.skin !== null
      ? hero.id * 1000 + swap.skin
      : null;

  const effectiveSkinId = browsedSkinId ?? appliedSkinId ?? cards[0]?.skin.skinId ?? null;

  const activeCard = useMemo(() => {
    if (effectiveSkinId === null) return cards[0] ?? null;
    return (
      cards.find(
        (card) =>
          card.skin.skinId === effectiveSkinId ||
          card.chromas.some((chroma) => chroma.skinId === effectiveSkinId),
      ) ??
      cards[0] ??
      null
    );
  }, [cards, effectiveSkinId]);

  const activeIndex = useMemo(() => {
    if (!activeCard) return 0;
    const idx = cards.findIndex((c) => c.skin.skinId === activeCard.skin.skinId);
    return idx >= 0 ? idx : 0;
  }, [cards, activeCard]);

  function stepSkin(delta: number) {
    if (cards.length === 0) return;
    const nextIndex = (activeIndex + delta + cards.length) % cards.length;
    const nextCard = cards[nextIndex];
    if (nextCard) {
      setHoveredChroma(null);
      setBrowsedSkinId(nextCard.skin.skinId);
    }
  }

  useEffect(() => {
    activeThumbRef.current?.scrollIntoView({
      behavior: "smooth",
      inline: "center",
      block: "nearest",
    });
  }, [activeIndex]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "ArrowLeft") {
        event.preventDefault();
        stepSkin(-1);
      } else if (event.key === "ArrowRight") {
        event.preventDefault();
        stepSkin(1);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  /* Default featured preview shows the base skin splash; hovering a chroma previews it immediately. */
  const previewEntry: CnSkin | null = hoveredChroma ?? activeCard?.skin ?? null;

  function showFeedback(kind: "success" | "error", text: string) {
    if (feedbackTimerRef.current) {
      clearTimeout(feedbackTimerRef.current);
      feedbackTimerRef.current = null;
    }
    setFeedback({ kind, text });
    if (kind === "success") {
      feedbackTimerRef.current = setTimeout(() => {
        setFeedback(null);
      }, 2500);
    }
  }

  function scheduleApply(picked: CnSkin) {
    if (hero === null) return;

    setBrowsedSkinId(picked.skinId);
    setStatusText("应用中…");
    setFeedback(null);

    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
    }

    const targetHero = hero;
    debounceRef.current = setTimeout(() => {
      debounceRef.current = null;
      if (picked.isBase) {
        restore.mutate(
          { alias: targetHero.alias },
          {
            onSuccess: () => {
              setStatusText("已恢复默认");
              showFeedback("success", "已恢复默认皮肤");
            },
            onError: (error) => {
              const detail = errorSummary(error);
              setStatusText(detail);
              showFeedback("error", `恢复失败：${detail}`);
            },
          },
        );
      } else {
        apply.mutate(
          { alias: targetHero.alias, heroId: picked.heroId, skinId: picked.skinId },
          {
            onSuccess: () => {
              setStatusText("已应用");
              showFeedback("success", `已应用：${picked.name}`);
            },
            onError: (error) => {
              const detail = errorSummary(error);
              setStatusText(detail);
              showFeedback("error", `应用失败：${detail}`);
            },
          },
        );
      }
    }, APPLY_DEBOUNCE_MS);
  }

  return (
    <div
      className="flex h-screen flex-col overflow-hidden border border-amber-500/30 bg-[#091428] text-[#f0e6d2] select-none"
      data-ui="CnOverlayWindow"
    >
      {/* League-client-inspired draggable header */}
      <header
        data-tauri-drag-region
        className="flex shrink-0 items-center gap-2.5 border-b border-amber-500/20 bg-[#0a1428] px-3 py-2"
      >
        {hero?.portraitUrl ? (
          <img
            src={hero.portraitUrl}
            alt={hero.name}
            data-tauri-drag-region
            className="h-7 w-7 rounded-full border border-amber-400/50 object-cover"
          />
        ) : (
          <div
            data-tauri-drag-region
            className="h-7 w-7 rounded-full border border-amber-400/30 bg-[#1e2328]"
          />
        )}

        <div data-tauri-drag-region className="min-w-0 flex-1">
          <div data-tauri-drag-region className="truncate text-sm font-semibold tracking-wide text-[#f0e6d2]">
            {hero ? hero.name : "等待选人…"}
          </div>
          <div data-tauri-drag-region className="truncate text-[11px] text-[#a09b8c]">
            {statusText ?? (appliedSkinId ? "已应用自定义皮肤" : "点击皮肤直接切换")}
          </div>
        </div>

        <button
          type="button"
          onClick={() => void api.cn.dismissOverlay()}
          title="本次选人隐藏浮窗"
          className="flex h-6 w-6 items-center justify-center rounded text-[#a09b8c] transition hover:bg-white/10 hover:text-[#f0e6d2]"
        >
          <XIcon className="h-4 w-4" />
        </button>
      </header>

      {skins.isPending || !hero ? (
        <div className="flex flex-1 items-center justify-center gap-2 text-xs text-[#a09b8c]">
          <Spinner size="sm" />
          <span>{hero ? "加载皮肤列表…" : "正在读取当前英雄…"}</span>
        </div>
      ) : (
        <div className="flex min-h-0 flex-1 flex-col">
          {/* Client-style carousel: left/right arrows + clickable 308:560 featured skin card */}
          {previewEntry && (
            <div
              className="relative flex min-h-0 flex-1 items-center justify-center overflow-hidden border-b border-amber-500/20 bg-[#010a13] px-10 py-3"
              onWheel={(event) => {
                if (Math.abs(event.deltaY) > 4 || Math.abs(event.deltaX) > 4) {
                  stepSkin(event.deltaY > 0 || event.deltaX > 0 ? 1 : -1);
                }
              }}
            >
              <button
                type="button"
                onClick={() => stepSkin(-1)}
                title="上一个皮肤 (←)"
                className="absolute left-2 z-10 flex h-9 w-9 items-center justify-center rounded-full border border-[#c8aa6e]/40 bg-[#091428]/85 text-[#c8aa6e] transition hover:border-[#c8aa6e] hover:bg-[#1e2328]"
              >
                <CaretLeftIcon className="h-5 w-5" weight="bold" />
              </button>

              <button
                type="button"
                onClick={() => scheduleApply(activeCard?.skin ?? previewEntry)}
                title="点击封面应用此皮肤"
                className={`group relative h-full aspect-[308/560] cursor-pointer overflow-hidden rounded-md border bg-[#091428] text-left shadow-xl transition ${
                  appliedSkinId === activeCard?.skin.skinId ||
                  activeCard?.chromas.some((c) => c.skinId === appliedSkinId)
                    ? "border-[#c8aa6e] ring-1 ring-[#c8aa6e]/50"
                    : "border-[#c8aa6e]/40 hover:border-[#c8aa6e]"
                }`}
              >
                {previewEntry.artUrl ? (
                  <img
                    src={previewEntry.artUrl}
                    alt={previewEntry.name}
                    className={`h-full w-full transition duration-150 ${
                      hoveredChroma
                        ? "object-contain p-1.5"
                        : "object-cover object-top group-hover:scale-[1.02]"
                    }`}
                  />
                ) : (
                  <div className="flex h-full items-center justify-center text-xs text-[#5b5a56]">
                    暂无预览图
                  </div>
                )}
                <div className="pointer-events-none absolute inset-x-0 bottom-0 h-24 bg-gradient-to-t from-[#091428] via-[#091428]/85 to-transparent" />
                <div className="absolute inset-x-2.5 bottom-2">
                  <div className="truncate text-xs font-semibold text-[#f0e6d2] drop-shadow">
                    {previewEntry.name}
                  </div>
                  <div className="flex items-center justify-between gap-1 text-[11px]">
                    <span className="text-[#c8aa6e]">
                      {previewEntry.isBase ? "经典原画" : qualityLabel(previewEntry.quality)}
                    </span>
                    <span className="text-[#a09b8c]">
                      {appliedSkinId !== null &&
                      (appliedSkinId === activeCard?.skin.skinId ||
                        activeCard?.chromas.some((c) => c.skinId === appliedSkinId))
                        ? "✓ 已应用"
                        : "点击封面应用"}
                    </span>
                  </div>
                </div>
              </button>

              <button
                type="button"
                onClick={() => stepSkin(1)}
                title="下一个皮肤 (→)"
                className="absolute right-2 z-10 flex h-9 w-9 items-center justify-center rounded-full border border-[#c8aa6e]/40 bg-[#091428]/85 text-[#c8aa6e] transition hover:border-[#c8aa6e] hover:bg-[#1e2328]"
              >
                <CaretRightIcon className="h-5 w-5" weight="bold" />
              </button>
            </div>
          )}

          {/* Chroma thumbnail strip when the active skin has chromas */}
          {activeCard && activeCard.chromas.length > 0 && (
            <div
              className="flex shrink-0 flex-wrap items-center justify-center gap-1.5 border-b border-amber-500/15 bg-[#0a1428]/80 px-3 py-2"
              onMouseLeave={() => setHoveredChroma(null)}
            >
              <button
                type="button"
                title={activeCard.skin.name}
                onMouseEnter={() => setHoveredChroma(null)}
                onClick={() => scheduleApply(activeCard.skin)}
                className={`h-8 w-8 overflow-hidden rounded border transition ${
                  effectiveSkinId === activeCard.skin.skinId
                    ? "border-[#c8aa6e] ring-1 ring-[#c8aa6e]/60"
                    : "border-[#3c3c41] opacity-80 hover:border-[#c8aa6e]/60 hover:opacity-100"
                }`}
              >
                {activeCard.skin.artUrl ? (
                  <img
                    src={activeCard.skin.artUrl}
                    alt={activeCard.skin.name}
                    className="h-full w-full object-cover object-top"
                  />
                ) : (
                  <span className="flex h-full w-full items-center justify-center bg-[#1e2328] text-[10px]">
                    原
                  </span>
                )}
              </button>
              {activeCard.chromas.map((chroma) => {
                const selected = effectiveSkinId === chroma.skinId;
                return (
                  <button
                    key={chroma.skinId}
                    type="button"
                    title={chroma.name}
                    onMouseEnter={() => setHoveredChroma(chroma)}
                    onClick={() => scheduleApply(chroma)}
                    className={`h-8 w-8 overflow-hidden rounded border transition ${
                      selected
                        ? "border-[#c8aa6e] ring-1 ring-[#c8aa6e]/60"
                        : "border-[#3c3c41] opacity-80 hover:border-[#c8aa6e]/60 hover:opacity-100"
                    }`}
                  >
                    {chroma.artUrl ? (
                      <img
                        src={chroma.artUrl}
                        alt={chroma.name}
                        loading="lazy"
                        className="h-full w-full object-contain"
                      />
                    ) : (
                      <span className="block h-full w-full bg-[#1e2328]" />
                    )}
                  </button>
                );
              })}
            </div>
          )}

          {feedback && (
            <div
              className={`shrink-0 border-b px-3 py-1.5 text-center text-xs font-medium ${
                feedback.kind === "success"
                  ? "border-emerald-500/30 bg-emerald-950/70 text-emerald-300"
                  : "border-rose-500/30 bg-rose-950/70 text-rose-300"
              }`}
            >
              {feedback.text}
            </div>
          )}

          {/* Horizontal skin strip at the bottom (client-style quick jump) */}
          <div className="shrink-0 overflow-x-auto bg-[#0a1428] px-2.5 py-2">
            <ul className="flex gap-1.5">
              {cards.map((card) => {
                const isSelected =
                  effectiveSkinId === card.skin.skinId ||
                  card.chromas.some((chroma) => chroma.skinId === effectiveSkinId);

                return (
                  <li key={card.skin.skinId} className="shrink-0">
                    <button
                      ref={isSelected ? activeThumbRef : null}
                      type="button"
                      title={card.skin.name}
                      onClick={() => {
                        setHoveredChroma(null);
                        setBrowsedSkinId(card.skin.skinId);
                      }}
                      className={`relative h-14 w-9 overflow-hidden rounded border transition ${
                        isSelected
                          ? "border-[#c8aa6e] ring-1 ring-[#c8aa6e]/60"
                          : "border-[#3c3c41]/80 opacity-65 hover:border-[#c8aa6e]/60 hover:opacity-100"
                      }`}
                    >
                      {card.skin.artUrl ? (
                        <img
                          src={card.skin.artUrl}
                          alt={card.skin.name}
                          loading="lazy"
                          className="h-full w-full object-cover object-top"
                        />
                      ) : null}
                    </button>
                  </li>
                );
              })}
            </ul>
          </div>
        </div>
      )}
    </div>
  );
}
