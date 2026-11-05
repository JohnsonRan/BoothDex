"use client";
// Every read the browser makes after the first screen. The first screen itself comes from the server
// (lib/server-data); components call these hooks instead of fetching on their own.

import { useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { api, apiReady } from "./api";
import { tagName } from "./catalog";
import type { UiLang } from "./messages";

/** While the API starts after a restart, checks every 3 s and re-renders the page from the server once it is up. */
export function useApiRecovery(waiting: boolean) {
  const router = useRouter();
  useEffect(() => {
    if (!waiting) return;
    const timer = setInterval(async () => {
      if (await apiReady()) router.refresh();
    }, 3000);
    return () => clearInterval(timer);
  }, [waiting, router]);
}

export type Tag = {
  name: string;
  display: string;
  display_zh?: string | null;
  item_count: number;
};

/** Smart tags matching `q` (the most used ones while it is empty); `labels` remembers names already seen. */
export function useTagSearch(q: string, ui: UiLang) {
  const [found, setFound] = useState<Tag[]>([]);
  const [labels, setLabels] = useState<Record<string, string>>({});
  useEffect(() => {
    const ctl = new AbortController();
    const timer = setTimeout(() => {
      api<{ items: Tag[] }>(
        `/api/tags?q=${encodeURIComponent(q)}&limit=${q ? 8 : 6}`,
        { signal: ctl.signal },
      )
        .then((r) => {
          setFound(r.items);
          setLabels((l) => ({
            ...l,
            ...Object.fromEntries(r.items.map((x) => [x.name, tagName(x, ui)])),
          }));
        })
        .catch(() => {});
    }, 150);
    return () => {
      clearTimeout(timer);
      ctl.abort();
    };
  }, [q, ui]);
  return { found, labels };
}
