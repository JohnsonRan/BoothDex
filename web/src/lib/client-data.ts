"use client";
// Every read the browser makes after the first screen. The first screen itself comes from the server
// (lib/server-data); components call these hooks instead of fetching on their own.

import { useEffect, useEffectEvent, useState } from "react";
import { type ApiStarting, api, probeHealth } from "./api";
import { tagName } from "./catalog";
import type { UiLang } from "./messages";

const waiters = new Set<() => void>();
let stopProbe: (() => void) | undefined;

/** All startup views in this tab need the same session and route reload, not one reload per view. */
export function watchApiRecovery(ready: () => void, delay: number) {
  const notify = () => ready();
  waiters.add(notify);
  if (waiters.size === 1) {
    const controller = new AbortController();
    const poll = async (timeoutMs?: number) => {
      const next = await probeHealth("", controller.signal, timeoutMs);
      if (controller.signal.aborted) return;
      if (next === null) waiters.values().next().value?.();
      else timer = setTimeout(poll, next, 12_000);
    };
    let timer = setTimeout(poll, delay);
    stopProbe = () => {
      clearTimeout(timer);
      controller.abort();
    };
  }
  return () => {
    if (waiters.delete(notify) && !waiters.size) {
      stopProbe?.();
      stopProbe = undefined;
    }
  };
}

export function useApiRecovery(
  starting: ApiStarting | false,
  ready: () => void,
) {
  const reload = useEffectEvent(ready);
  useEffect(() => {
    if (starting) return watchApiRecovery(reload, starting.retryAfterMs);
  }, [starting]);
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
