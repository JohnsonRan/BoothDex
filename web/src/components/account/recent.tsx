"use client";
// The recent view (`?v=recent`), master's browse.tsx:235-254 and 119-122: the items the visitor opened, newest
// first, one page of up to 100, no filters or sort. The list is the session's (the account's when signed in,
// else this browser's), so it is read here rather than on the server; the grid and its tiles are the home
// page's (results-grid.tsx), without the viewed marks, and its silhouettes hold the places while it loads.

import { EmptyState } from "@astryxdesign/core/EmptyState";
import { Icon } from "@astryxdesign/core/Icon";
import { Text } from "@astryxdesign/core/Text";
import { use, useEffect, useState } from "react";
import {
  CountSilhouette,
  ResultsSilhouette,
} from "@/components/catalog/loading";
import { FailedResults } from "@/components/catalog/results/result-states";
import { ResultsGrid } from "@/components/catalog/results/results-grid";
import { useUi } from "@/components/providers";
import { api } from "@/lib/api";
import type { ItemResponse } from "@/lib/api-types";
import { LOCALES, type Rates, type Settings } from "@/lib/settings-core";
import { useAccount } from "./session";
import "@/lib/icons-on-demand";

const PAGE = 100;

// one request per list while it is under way, shared by the count and the grid; a later read asks again
const requests = new Map<string, Promise<ItemResponse[]>>();
export function itemsOf(ids: number[]): Promise<ItemResponse[]> {
  const key = ids.join(",");
  let p = requests.get(key);
  if (!p) {
    p = ids.length
      ? api<{ items: ItemResponse[] }>(`/api/items?ids=${key}`).then(
          (r) => r.items,
        )
      : Promise.resolve([]);
    requests.set(key, p);
    const done = () => requests.delete(key);
    p.then(done, done);
  }
  return p;
}

// the count and the grid read again together when the visitor retries
const retries = new Set<() => void>();
const retry = () => {
  for (const again of retries) again();
};

/** The opened items still in the list, in its order; null while either is loading, "failed" if the read failed. */
function useRecentItems(): ItemResponse[] | "failed" | null {
  const { viewed } = useAccount();
  const [found, setFound] = useState<ItemResponse[] | "failed" | null>(null);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    const again = () => {
      setFound(null);
      setAttempt((n) => n + 1);
    };
    retries.add(again);
    return () => {
      retries.delete(again);
    };
  }, []);
  const ids = viewed?.slice(0, PAGE);
  const key = ids?.join(",");
  // biome-ignore lint/correctness/useExhaustiveDependencies: the key stands for the ids; attempt reads them again
  useEffect(() => {
    if (!ids) return;
    let live = true;
    itemsOf(ids).then(
      (items) => live && setFound(items),
      () => live && setFound("failed"),
    );
    return () => {
      live = false;
    };
  }, [key, attempt]);
  if (!viewed || !found) return null;
  if (found === "failed") return found;
  // a sign-out or a cleared history hides the items at once, before the next answer
  const order = new Map(viewed.map((id, i) => [id, i]));
  return found
    .filter((it) => order.has(it.id))
    .sort((a, b) => (order.get(a.id) ?? 0) - (order.get(b.id) ?? 0));
}

export function RecentCount() {
  const { t, ui } = useUi();
  const items = useRecentItems();
  if (!items) return <CountSilhouette />;
  if (items === "failed") return null;
  return (
    <Text type="supporting" hasTabularNumbers>
      {t("title.count", { n: items.length.toLocaleString(LOCALES[ui]) })}
    </Text>
  );
}

export function RecentResults({
  settings,
  rates,
}: {
  settings: Settings;
  rates: Promise<Rates>;
}) {
  const { t } = useUi();
  const items = useRecentItems();
  const resolved = use(rates);
  if (!items) return <ResultsSilhouette label={t("list.loading")} />;
  if (items === "failed")
    return <FailedResults starting={false} onRetry={retry} />;
  if (!items.length)
    return (
      <EmptyState
        icon={<Icon icon="site:recent" size="lg" />}
        title={t("recent.empty")}
      />
    );
  return (
    <ResultsGrid
      items={items}
      settings={settings}
      rates={resolved}
      t={t}
      markViewed={false}
    />
  );
}
