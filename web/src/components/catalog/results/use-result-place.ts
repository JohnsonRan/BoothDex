// Where the results stand (result-pages.tsx): the first tile in view kept in place through every change, the
// return to the place a history entry keeps, the pages above a start past page 1, and the pages kept for a return.

import {
  type Dispatch,
  type SetStateAction,
  startTransition,
  useCallback,
  useLayoutEffect,
} from "react";
import { covered } from "@/components/catalog/band/band-edge";
import { restoreStep } from "@/lib/result-frame";
import {
  type Dir,
  drawnWith,
  type Kept,
  readSnapshot,
  restorePlan,
  type Snapshot,
  withAbove,
} from "@/lib/result-pages";
import type { Settings } from "@/lib/settings-core";
import type { Page, PageReads, ResultRefs } from "./use-page-reads";

/**
 * The pages of the results last left, for a return to them (lib/result-pages restorePlan), with the settings their
 * tiles were drawn in and the place last saved.
 */
let kept: (Kept<Page> & { drawn: string; place: Snapshot | null }) | undefined;

export function useResultPlace(
  r: ResultRefs,
  { keep, load, loadNext }: PageReads,
  {
    page,
    total,
    key,
    settings,
    pages,
    failed,
    loading,
    setPages,
  }: {
    page: number;
    total: number;
    /** the search these results answer, as the history entry keeps it */
    key: string;
    settings: Settings;
    pages: Page[];
    failed: boolean;
    loading: Dir | null;
    setPages: Dispatch<SetStateAction<Page[]>>;
  },
) {
  /**
   * Scrolls the restored tile to where it was. Short of it (the tile's page not yet whole, or too little below it
   * to scroll that far) the next part is read and this runs again; with nothing left to read it stops there.
   */
  const position = useCallback(() => {
    const s = r.restoring.current;
    if (!s) return;
    const el = r.grid.current?.querySelector(`[data-item="${s.id}"]`);
    const edge = covered();
    if (el)
      scrollTo(0, el.getBoundingClientRect().top + scrollY - edge - s.top);
    const p = r.now.current.pages;
    const offset = el
      ? el.getBoundingClientRect().top - edge - s.top
      : undefined;
    if (restoreStep(offset, p[p.length - 1], total) === "read") loadNext();
    else r.restoring.current = null;
  }, [r, loadNext, total]);

  // before the effect below, so the pages it puts in on the first render are kept in place
  // biome-ignore lint/correctness/useExhaustiveDependencies: runs after each change of what is drawn
  useLayoutEffect(() => {
    const a = r.anchor.current;
    r.anchor.current = null;
    if (r.restoring.current) position();
    else if (a?.el.isConnected) {
      const moved = a.el.getBoundingClientRect().top + scrollY - a.at;
      if (moved) scrollBy(0, moved);
    }
    r.tick.current();
  }, [pages, failed, position]);

  // A reload, Back or Forward to these results: the entry says where the reader was. Else a start past page 1:
  // the pages above go in before the first paint, the window scrolled by their height. Before the sentinels
  // (use-result-frame.ts), so a page read for the return goes first. Leaving keeps the pages for a return.
  // biome-ignore lint/correctness/useExhaustiveDependencies: once per visit
  useLayoutEffect(() => {
    history.scrollRestoration = "manual";
    r.width.current = r.grid.current?.clientWidth ?? 0;
    const drawn = drawnWith(settings);
    // New settings (a language or currency change) remount the results through a router refresh, which writes a
    // history entry of Next's own without the place: the place these results last saved goes on from here.
    const snap =
      readSnapshot(history.state, key) ??
      (kept?.key === key && kept.drawn !== drawn ? kept.place : null);
    r.place.current = snap;
    const plan =
      snap &&
      restorePlan(
        snap,
        kept?.drawn === drawn ? kept : undefined,
        r.width.current,
        Date.now(),
      );
    if (snap) r.restoring.current = snap;
    // the same pages when the results were only hidden (a route kept in the background) and come back
    if (plan && "pages" in plan) {
      if (plan.pages !== r.now.current.pages)
        startTransition(() => setPages(plan.pages));
      else position();
    } else if (plan && plan.page !== page)
      load(
        plan.page,
        (_, fresh) => withAbove([{ n: plan.page, tiles: fresh }], plan.page),
        "next",
      );
    else if (page > 1) keep(() => setPages((p) => withAbove(p, page)), true);
    else if (snap) position();
    return () => {
      kept = {
        key,
        drawn,
        place: r.place.current,
        pages: r.now.current.pages,
        width: r.width.current,
        at: Date.now(),
      };
      history.scrollRestoration = "auto";
    };
  }, []);

  // a read is over once what it brought is drawn: before that, the next frame would still find its page unread
  useLayoutEffect(() => {
    if (!loading) r.busy.current = false;
  }, [r, loading]);
}
