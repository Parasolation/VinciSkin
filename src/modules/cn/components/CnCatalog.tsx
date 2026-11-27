import { useEffect, useState } from "react";

import { Button, ComboboxField, EmptyState, Spinner, Switch, useToast } from "@/components";
import { describeError, errorMessage, m } from "@/i18n";
import { api, type CnHero, type CnPatcherStatus, type CnQuality, isOk, unwrap } from "@/lib/tauri";

import { useCnHeroes } from "../api";
import { qualityOptions } from "../skins";
import { CnHeroPicker } from "./CnHeroPicker";
import { CnSkinGrid } from "./CnSkinGrid";

/** The champion and skin tables the Chinese client ships, read from the Tencent CDN. */
export function CnCatalog() {
  const heroes = useCnHeroes();
  const toast = useToast();

  /* Held as the option value rather than a number, so the picker stays controlled from the
     first render. Empty means nothing is picked yet. */
  const [picked, setPicked] = useState("");
  const [quality, setQuality] = useState("");
  const [onlyApplied, setOnlyApplied] = useState(false);
  const [swapAllChampions, setSwapAllChampions] = useState(false);
  const [swapAllBusy, setSwapAllBusy] = useState(false);
  const [patcher, setPatcher] = useState<CnPatcherStatus | null>(null);
  const [injectBusy, setInjectBusy] = useState(false);

  useEffect(() => {
    let alive = true;
    void api.cn.getSwapAllChampions().then((result) => {
      if (alive && isOk(result)) {
        setSwapAllChampions(result.value);
      }
    });
    return () => {
      alive = false;
    };
  }, []);

  async function toggleSwapAllChampions(checked: boolean) {
    setSwapAllBusy(true);
    try {
      const result = await api.cn.setSwapAllChampions(checked);
      if (isOk(result)) {
        setSwapAllChampions(result.value);
        toast.info(
          result.value ? "已开启：同时替换其他已配英雄" : "已关闭：默认仅替换自己当前英雄",
        );
      } else {
        toast.error("切换换肤范围失败", describeError(result.error).description);
      }
    } finally {
      setSwapAllBusy(false);
    }
  }

  /* The client's champ-select pick is what the page follows, so the skin the
     reader wants is one click away instead of a search. */
  useEffect(() => {
    let alive = true;
    let seen = 0;

    async function follow() {
      const result = await api.cn.autoStatus();
      if (!alive || !isOk(result)) return;

      const champion = result.value.currentChampion;
      if (champion && champion !== seen) {
        seen = champion;
        setPicked(String(champion));
      }
      if (!champion) seen = 0;
    }

    void follow();
    const id = setInterval(() => void follow(), 2000);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, []);

  const hero: CnHero | null =
    heroes.data?.heroes.find((candidate) => String(candidate.id) === picked) ?? null;

  useEffect(() => {
    let alive = true;

    async function pollPatcher() {
      const result = await api.cn.patcherStatus();
      if (alive && isOk(result)) {
        setPatcher(result.value);
      }
    }

    void pollPatcher();
    const id = setInterval(() => void pollPatcher(), 2000);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, []);

  const injectRunning = patcher?.running ?? false;

  async function toggleInject() {
    setInjectBusy(true);
    try {
      if (injectRunning) {
        unwrap(await api.cn.stopPatcher());
        setPatcher({ running: false, state: null, error: null });
      } else {
        unwrap(await api.cn.startPatcher());
        setPatcher({ running: true, state: null, error: null });
      }
    } catch (error: unknown) {
      const title = injectRunning ? m.cn_inject_stop_failed_title() : m.cn_inject_failed_title();
      toast.error(title, errorMessage(error));
    } finally {
      setInjectBusy(false);
    }
  }

  if (heroes.isPending) {
    return (
      <div className="flex h-full items-center justify-center gap-2 text-meta text-surface-400">
        <Spinner size="sm" />
        {m.cn_heroes_loading()}
      </div>
    );
  }

  if (heroes.isError) {
    const copy = describeError(heroes.error);

    return (
      <EmptyState
        title={m.cn_heroes_error_title()}
        description={copy.detail ?? copy.description ?? copy.title}
      />
    );
  }

  if (heroes.data.heroes.length === 0) {
    return <EmptyState title={m.cn_heroes_empty()} />;
  }

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 overflow-hidden p-5" data-ui="CnCatalog">
      <div className="flex min-h-0 flex-1 flex-col gap-4 lg:flex-row">
        <aside className="flex max-h-56 min-h-0 flex-col lg:max-h-none lg:w-60 lg:shrink-0">
          <CnHeroPicker
            heroes={heroes.data.heroes}
            picked={picked}
            onPick={(value) => setPicked(value)}
          />
        </aside>

        <section className="flex min-h-0 flex-1 flex-col gap-2">
          <div className="flex shrink-0 flex-wrap items-center gap-2">
            <ComboboxField
              className="w-40"
              placeholder={m.cn_quality_filter_placeholder()}
              options={qualityOptions()}
              value={quality}
              onValueChange={(value) => setQuality(value ?? "")}
            />

            <div className="flex items-center gap-2">
              <Switch
                id="cn-only-applied"
                checked={onlyApplied}
                onCheckedChange={(checked) => setOnlyApplied(checked)}
              />
              <label htmlFor="cn-only-applied" className="text-sm text-surface-300">
                {m.cn_only_applied_filter()}
              </label>
            </div>

            <div
              className="flex items-center gap-2"
              title="关闭时默认只替换自己当前所选英雄；开启后所有已配置的英雄都会同时生效（队友/对手选到也会换肤）"
            >
              <Switch
                id="cn-swap-all-champions"
                checked={swapAllChampions}
                disabled={swapAllBusy}
                onCheckedChange={(checked) => void toggleSwapAllChampions(checked)}
              />
              <label htmlFor="cn-swap-all-champions" className="text-sm text-surface-300">
                同步替换其他英雄
              </label>
            </div>

            <Button
              size="sm"
              variant={injectRunning ? "outline" : "default"}
              className="ml-auto"
              disabled={injectBusy}
              loading={injectBusy}
              onClick={toggleInject}
            >
              {injectRunning ? "取消注入" : "开启注入"}
            </Button>
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto pr-1">
            <CnSkinGrid
              hero={hero}
              quality={quality === "" ? null : (quality as CnQuality)}
              onlyApplied={onlyApplied}
            />
          </div>
        </section>
      </div>
    </div>
  );
}
