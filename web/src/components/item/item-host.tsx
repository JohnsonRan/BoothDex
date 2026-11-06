"use client";
// Opens the item dialog for `?item=<id>`. The URL is the state: opening pushes one history entry (Back closes
// the dialog), moving to another item replaces it, closing steps back; a shared link opens on load and closing
// it just drops the parameter. The browser history API is used directly (it integrates with the Next router),
// so opening an item never renders the page again on the server.
// Tiles stay server markup: one document listener opens whatever carries `data-item` (item-tile.tsx), the
// favourite button excepted. The dialog's code loads with the first item opened.

import dynamic from "next/dynamic";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { Rates } from "@/lib/settings-core";
import { itemHref, itemParam } from "./item-data";

const ItemDialog = dynamic(() => import("./item-dialog"), { ssr: false });

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

  useEffect(() => {
    const sync = () => {
      const n = itemParam(location.search);
      if (!n) pushed.current = false;
      setId(n);
    };
    sync();
    addEventListener("popstate", sync);
    return () => removeEventListener("popstate", sync);
  }, []);

  const opener = useMemo<Opener>(
    () => ({
      open: (next) => {
        const href = itemHref(location.href, next);
        if (itemParam(location.search)) history.replaceState(null, "", href);
        else {
          history.pushState(null, "", href);
          pushed.current = true;
        }
        setId(next);
      },
      close: (then) => {
        if (pushed.current) {
          pushed.current = false;
          if (then) addEventListener("popstate", then, { once: true });
          history.back();
          return;
        }
        history.replaceState(null, "", itemHref(location.href, 0));
        setId(0);
        then?.();
      },
    }),
    [],
  );

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

  // mounted from the first item on, so the dialog can play its closing transition
  const [used, setUsed] = useState(false);
  if (id && !used) setUsed(true);

  return (
    <Ctx value={opener}>
      <RatesCtx value={rates}>{children}</RatesCtx>
      {used && <ItemDialog id={id} rates={rates} />}
    </Ctx>
  );
}
