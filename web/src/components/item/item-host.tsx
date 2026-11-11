"use client";
// Opens the item dialog for `?item=<id>`. The URL is the state: opening pushes one history entry (Back closes
// the dialog), moving to another item replaces it, closing steps back; a shared link opens on load and closing
// it just drops the parameter. The browser history API is used directly (it integrates with the Next router),
// so opening an item never renders the page again on the server.
// Tiles stay server markup: one document listener opens whatever carries `data-item` (item-tile.tsx), the
// favourite button excepted. The dialog's code loads once the page is idle (lib/lazy), so the first item opened
// does not wait for the network, nor for next/dynamic's Suspense reveal, which React holds back up to 300 ms.
// Closing closes at once and lets the history step follow: a Back the router answers first re-renders the page
// before the dialog would. Pointing at a tile fetches its record (item-cache.ts), so the card mostly has it by
// the click: a mouse resting on a tile a moment (one passing over the grid fetches nothing), a press at once.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { Lazy, lazySurface, preloadWhenIdle } from "@/lib/lazy";
import type { Rates } from "@/lib/settings-core";
import { prefetchItem } from "./item-cache";
import { itemHref, itemParam } from "./item-data";

const HOVER_MS = 120;

const ItemDialog = lazySurface(() =>
  import("./item-dialog").then((m) => m.default),
);

type Opener = {
  /** shows item `id`, in place of the one open */
  open: (id: number) => void;
  /** closes the dialog, then runs `then` once the URL has no item (for a navigation that follows) */
  close: (then?: () => void) => void;
};
const Ctx = createContext<Opener | null>(null);
const RatesCtx = createContext<Promise<Rates>>(Promise.resolve({}));

export function useItemOpener(): Opener {
  const v = useContext(Ctx);
  if (!v) throw new Error("useItemOpener outside ItemHost");
  return v;
}

/** The exchange rates the page started, for item prices outside the dialog (the search suggestions). */
export const useRates = (): Promise<Rates> => useContext(RatesCtx);

export function ItemHost({
  rates,
  children,
}: {
  rates: Promise<Rates>;
  children: React.ReactNode;
}) {
  const [id, setId] = useState(0);
  // the open entry was pushed here, so closing steps back instead of leaving a duplicate entry
  const pushed = useRef(false);
  // a close's step back has not landed yet
  const leaving = useRef(false);

  useEffect(() => {
    const sync = () => {
      leaving.current = false;
      const n = itemParam(location.search);
      if (!n) pushed.current = false;
      setId(n);
    };
    sync();
    addEventListener("popstate", sync);
    preloadWhenIdle(ItemDialog.preload);
    return () => removeEventListener("popstate", sync);
  }, []);

  const opener = useMemo<Opener>(() => {
    const open = (next: number) => {
      // the dialog closes before its history entry goes: an item opened in between waits for it, or the step
      // back would take the new item's entry
      if (leaving.current) {
        addEventListener("popstate", () => open(next), { once: true });
        return;
      }
      const href = itemHref(location.href, next);
      if (itemParam(location.search)) history.replaceState(null, "", href);
      else {
        history.pushState(null, "", href);
        pushed.current = true;
      }
      setId(next);
    };
    const close = (then?: () => void) => {
      setId(0);
      if (pushed.current) {
        pushed.current = false;
        leaving.current = true;
        if (then) addEventListener("popstate", then, { once: true });
        history.back();
        return;
      }
      history.replaceState(null, "", itemHref(location.href, 0));
      then?.();
    };
    return { open, close };
  }, []);

  const { open } = opener;
  const onTile = useCallback(
    (e: MouseEvent) => {
      // a modified click on the title link opens the item in a new tab, as a link does
      if (e.defaultPrevented || e.button !== 0) return;
      if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
      if (!(e.target instanceof Element) || e.target.closest("[data-fav]"))
        return;
      const tile = e.target.closest<HTMLElement>("[data-item]");
      if (!tile) return;
      e.preventDefault();
      open(Number(tile.dataset.item));
    },
    [open],
  );
  useEffect(() => {
    document.addEventListener("click", onTile);
    return () => document.removeEventListener("click", onTile);
  }, [onTile]);

  useEffect(() => {
    const tileOf = (e: Event) => {
      const el = e.target instanceof Element ? e.target : null;
      const tile = el?.closest<HTMLElement>("[data-item]");
      return tile && !el?.closest("[data-fav]") ? Number(tile.dataset.item) : 0;
    };
    let over = 0;
    let timer = 0;
    const onOver = (e: PointerEvent) => {
      if (e.pointerType !== "mouse") return;
      const n = tileOf(e);
      if (n === over) return;
      over = n;
      clearTimeout(timer);
      if (n) timer = window.setTimeout(() => prefetchItem(n), HOVER_MS);
    };
    const onDown = (e: PointerEvent) => {
      const n = tileOf(e);
      if (n) prefetchItem(n);
    };
    document.addEventListener("pointerover", onOver);
    document.addEventListener("pointerdown", onDown);
    return () => {
      clearTimeout(timer);
      document.removeEventListener("pointerover", onOver);
      document.removeEventListener("pointerdown", onDown);
    };
  }, []);

  // mounted from the first item on, so the dialog can play its closing transition
  const [used, setUsed] = useState(false);
  if (id && !used) setUsed(true);

  return (
    <Ctx value={opener}>
      <RatesCtx value={rates}>{children}</RatesCtx>
      {used && <Lazy surface={ItemDialog} id={id} rates={rates} />}
    </Ctx>
  );
}
