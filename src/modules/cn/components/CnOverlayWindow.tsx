import { XIcon } from "@phosphor-icons/react";
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
  const [selectedSkinId, setSelectedSkinId] = useState<number | null>(null);
  const [statusText, setStatusText] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<{ kind: "success" | "error"; text: string } | null>(
    null,
  );
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const feedbackTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

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

    async function loadInitial() {
      const result = await api.cn.autoStatus();
      if (!alive || !isOk(result)) return;
      if (result.value.currentChampion) {
        setChampionId(result.value.currentChampion);
      }
    }

    void loadInitial();

    const unlistenPromise = listen<number>("cn-overlay-champion", (event) => {
      if (alive) {
        setChampionId(event.payload);
      }
    });

    return () => {
      alive = false;
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
    setSelectedSkinId(null);
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

  const effectiveSkinId = selectedSkinId ?? appliedSkinId ?? cards[0]?.skin.skinId ?? null;

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

  const activeEntry: CnSkin | null = useMemo(() => {
    if (!activeCard) return null;
    return (
      activeCard.chromas.find((chroma) => chroma.skinId === effectiveSkinId) ?? activeCard.skin
    );
  }, [activeCard, effectiveSkinId]);

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

    setSelectedSkinId(picked.skinId);
    setStatusText("切换中…");
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
          {/* Featured splash preview */}
          {activeEntry && (
            <div className="relative h-44 shrink-0 overflow-hidden border-b border-amber-500/20 bg-[#010a13]">
              {activeEntry.artUrl ? (
                <img
                  src={activeEntry.artUrl}
                  alt={activeEntry.name}
                  className="h-full w-full object-cover object-top transition-opacity duration-200"
                />
              ) : (
                <div className="flex h-full items-center justify-center text-xs text-[#5b5a56]">
                  暂无预览图
                </div>
              )}
              <div className="pointer-events-none absolute inset-x-0 bottom-0 h-20 bg-gradient-to-t from-[#091428] via-[#091428]/70 to-transparent" />
              <div className="absolute inset-x-3 bottom-2 flex items-end justify-between gap-2">
                <div className="min-w-0">
                  <div className="truncate text-sm font-semibold text-[#f0e6d2] drop-shadow">
                    {activeEntry.name}
                  </div>
                  <div className="text-[11px] text-[#c8aa6e]">
                    {activeEntry.isBase ? "经典原画" : qualityLabel(activeEntry.quality)}
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* Chroma thumbnail strip when the active skin has chromas */}
          {activeCard && activeCard.chromas.length > 0 && (
            <div className="flex shrink-0 flex-wrap items-center gap-1.5 border-b border-amber-500/15 bg-[#0a1428]/80 px-3 py-2">
              <button
                type="button"
                title={activeCard.skin.name}
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
                    className="h-full w-full object-cover"
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
                        className="h-full w-full object-cover"
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
              className={`shrink-0 border-b px-3 py-1.5 text-xs font-medium ${
                feedback.kind === "success"
                  ? "border-emerald-500/30 bg-emerald-950/70 text-emerald-300"
                  : "border-rose-500/30 bg-rose-950/70 text-rose-300"
              }`}
            >
              {feedback.text}
            </div>
          )}

          {/* Scrollable client-style skin selector grid */}
          <div className="min-h-0 flex-1 overflow-y-auto p-2.5">
            <ul className="grid grid-cols-2 gap-2">
              {cards.map((card) => {
                const isSelected =
                  effectiveSkinId === card.skin.skinId ||
                  card.chromas.some((chroma) => chroma.skinId === effectiveSkinId);

                return (
                  <li key={card.skin.skinId}>
                    <button
                      type="button"
                      onClick={() => scheduleApply(card.skin)}
                      className={`group relative flex w-full flex-col overflow-hidden rounded border text-left transition ${
                        isSelected
                          ? "border-[#c8aa6e] ring-1 ring-[#c8aa6e]/50"
                          : "border-[#3c3c41]/80 hover:border-[#c8aa6e]/60"
                      }`}
                    >
                      <div className="relative aspect-[16/10] w-full overflow-hidden bg-[#010a13]">
                        {card.skin.artUrl ? (
                          <img
                            src={card.skin.artUrl}
                            alt={card.skin.name}
                            loading="lazy"
                            className="h-full w-full object-cover object-top transition duration-200 group-hover:scale-105"
                          />
                        ) : null}
                        {card.chromas.length > 0 && (
                          <span className="absolute right-1 bottom-1 rounded bg-black/75 px-1 py-0.5 text-[10px] text-[#c8aa6e]">
                            {card.chromas.length} 炫彩
                          </span>
                        )}
                      </div>
                      <div className="truncate bg-[#1e2328] px-2 py-1 text-[11px] font-medium text-[#f0e6d2]">
                        {card.skin.name}
                      </div>
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
