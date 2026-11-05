"use client";
// Every read the browser makes after the first screen. The first screen itself comes from the server
// (lib/server-data); components call these hooks instead of fetching on their own.

import { useEffect, useState } from "react";
import { api, apiReady } from "./api";
import { tagName } from "./catalog";
import type { UiLang } from "./messages";

/**
 * While the API starts after a restart, checks every 3 s and calls `ready` once it is up: the session's reload
 * (account/session.tsx), which reads the account again and re-renders the page from the server, as on master.
 */
export function useApiRecovery(waiting: boolean, ready: () => void) {
  useEffect(() => {
    if (!waiting) return;
    const timer = setInterval(async () => {
      if (await apiReady()) ready();
    }, 3000);
    return () => clearInterval(timer);
  }, [waiting, ready]);
}

type Tag = {
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
