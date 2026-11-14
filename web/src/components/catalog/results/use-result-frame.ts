// What scrolling does to the results (result-pages.tsx): the sentinels that read the next page and put more boxes
// up, and one frame after each scroll the page in view in the address bar and the history entry, far pages given
// up for boxes, near ones drawn again, and the page in view read. The decisions are lib/result-frame.ts.

import { type Dispatch, type SetStateAction, useEffect } from "react";
import { covered } from "@/components/catalog/band/band-edge";
import { framePages, pageToRead } from "@/lib/result-frame";
import {
  type Dir,
  pageAt,
  resized,
  resultsQuery,
  SCREENS,
  SNAPSHOT,
  type Snapshot,
  withAbove,
  withPage,
} from "@/lib/result-pages";
import { showInView } from "./page-in-view";
import {
  firstInView,
  type Page,
  type PageReads,
  type ResultRefs,
} from "./use-page-reads";

const MARGIN = `${SCREENS * 100}%`;
// no scroll event for this long and scrolling has stopped: an engine sends one a frame while a fling or a smooth
// scroll runs, so a pause this long is the reader's, and the changes held for it go in soon after
const SETTLE_MS = 200;

export function useResultFrame(
  r: ResultRefs,
  { starts, keep, above, loadNext, loadInto, measure }: PageReads,
  {
    page,
    key,
    lexical,
    hasNext,
    hasPrev,
    loading,
    failed,
    setPages,
    setMask,
  }: {
    page: number;
    key: string;
    lexical: boolean;
    hasNext: boolean;
    hasPrev: boolean;
    loading: Dir | null;
    failed: boolean;
    setPages: Dispatch<SetStateAction<Page[]>>;
    setMask: (mask: undefined) => void;
  },
) {
  // template :2356-2376: the sentinel under the last row loads the next page, observed again after each one
  useEffect(() => {
    const el = r.end.current;
    if (!el || !hasNext || loading || failed) return;
    const seen = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) loadNext();
      },
      { rootMargin: `0px 0px ${MARGIN} 0px` },
    );
    seen.observe(el);
    return () => seen.disconnect();
  }, [r, hasNext, loading, failed, loadNext]);

  // the one above the first row marks the top of the boxes as near: more go up when the reader moves up. Its
  // margin is screens of the viewport's height, so a new size (a rotation) sets it again.
  useEffect(() => {
    const el = r.top.current;
    if (!el || !hasPrev) return;
    let seen: IntersectionObserver | undefined;
    const watch = () => {
      seen?.disconnect();
      seen = new IntersectionObserver(
        ([e]) => {
          r.nearTop.current = e.isIntersecting;
        },
        { rootMargin: `${SCREENS * innerHeight - covered()}px 0px 0px 0px` },
      );
      seen.observe(el);
    };
    watch();
    addEventListener("resize", watch);
    return () => {
      r.nearTop.current = false;
      removeEventListener("resize", watch);
      seen?.disconnect();
    };
  }, [r, hasPrev]);

  // A Skeleton pulses without end (core Skeleton.tsx:99-103), one under every cover still loading, and each pulse
  // sends an `animationiteration` that React's root listener walks the tree for, though nothing listens to it:
  // during a fling hundreds of them a second held the main thread. Stopped at the window, they never reach React.
  useEffect(() => {
    const stop = (e: Event) => e.stopPropagation();
    addEventListener("animationiteration", stop, true);
    return () => removeEventListener("animationiteration", stop, true);
  }, []);

  // One frame after scrolling: the page in view goes to the address bar, far pages give up their tiles and near
  // ones take them back, a page not read yet near view is read, the page in view first, and more boxes go up when
  // the reader moves up near the top of them. The entry records where the reader is when a page is crossed, when
  // scrolling settles, and when the page in view has been read.
  // biome-ignore lint/correctness/useExhaustiveDependencies: reads the latest state through `now`
  useEffect(() => {
    let frame = 0;
    let settle = 0;
    let lastY = scrollY;
    let up = false;
    let shown = page;
    let saved = true;
    // the address these results answer; once a navigation changes it, they no longer write to it
    const own = resultsQuery(location.search);
    const save = () => {
      const g = r.grid.current;
      const p = r.now.current.pages;
      const i = p.findIndex((q) => q.n === shown);
      if (!g || i < 0 || r.restoring.current) return false;
      if (resultsQuery(location.search) !== own) return true;
      const at = starts(p);
      const edge = covered();
      const el = g.children[firstInView(g.children, at[i], at[i + 1], edge)] as
        | HTMLElement
        | undefined;
      const id = Number(el?.dataset.item);
      if (!el || !id) return false;
      const snap: Snapshot = {
        key,
        page: shown,
        id,
        top: el.getBoundingClientRect().top - edge,
      };
      // with Next's own entry state, which its history patch passes through: the router is not told, so a
      // navigation under way (a filter change scrolls to the top) is not replaced by this address
      r.place.current = snap;
      history.replaceState(
        { ...history.state, [SNAPSHOT]: snap },
        "",
        `${location.pathname}${withPage(location.search, shown, lexical)}`,
      );
      return true;
    };
    const run = () => {
      frame = 0;
      const g = r.grid.current;
      const { pages: p } = r.now.current;
      if (!g) return;
      const first = p[0].n;
      if (up && r.nearTop.current && first > 1)
        above(
          "boxes",
          () => keep(() => setPages((cur) => withAbove(cur, first))),
          true,
        );
      const kids = g.children;
      const at = starts(p);
      const h = innerHeight;
      const tops = p.map((_, j) => kids[at[j]].getBoundingClientRect().top);
      const lastKid = kids[at[p.length] - 1];
      const bottoms = [
        ...tops.slice(1),
        lastKid?.getBoundingClientRect().bottom ?? 0,
      ];
      // the middle of what the band leaves in view
      const c = covered();
      const i = pageAt(tops, (c + h) / 2);
      if (!r.restoring.current && (p[i].n !== shown || !saved)) {
        if (p[i].n !== shown) {
          shown = p[i].n;
          showInView(shown);
        }
        saved = save();
      }
      const focused = document.activeElement;
      const next = framePages(
        p,
        {
          tops,
          bottoms,
          height: h,
          edge: c,
          scrolling: r.scrolling.current,
        },
        // a far page's tiles measured for its boxes; none while focus is inside it
        (j) => {
          const own = [...kids].slice(at[j], at[j] + p[j].tiles.length);
          if (focused && own.some((k) => k.contains(focused))) return undefined;
          return own.map((k) => k.getBoundingClientRect().height);
        },
      );
      if (next.some((q, j) => q !== p[j]))
        keep(() => setPages((cur) => (cur === p ? next : cur)));
      const j = pageToRead(p, next, i);
      if (j !== undefined && !r.restoring.current)
        loadInto(p[j].n, tops[j] < c ? "prev" : "next");
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(run);
    };
    const onScroll = () => {
      up = scrollY < lastY;
      lastY = scrollY;
      r.scrolling.current = true;
      schedule();
      clearTimeout(settle);
      settle = window.setTimeout(() => {
        r.scrolling.current = false;
        saved = save();
        for (const change of r.held.current.values()) change();
        r.held.current.clear();
        schedule();
      }, SETTLE_MS);
    };
    const onWheel = (e: WheelEvent) => {
      if (e.deltaY >= 0) return;
      up = true;
      schedule();
    };
    // a new width reflows the rows: the boxes take the columns, the pitch and a tile's height laid out at it,
    // measured after the place is taken, so their change of height is kept out of view too
    const onResize = () => {
      const g = r.grid.current;
      if (!g || g.clientWidth === r.width.current) return;
      r.width.current = g.clientWidth;
      setMask(undefined);
      keep(() => {
        const height = measure();
        setPages((cur) => resized(cur, height));
      });
    };
    r.tick.current = schedule;
    addEventListener("scroll", onScroll, { passive: true });
    addEventListener("wheel", onWheel, { passive: true });
    addEventListener("resize", onResize);
    return () => {
      cancelAnimationFrame(frame);
      clearTimeout(settle);
      showInView(undefined);
      r.tick.current = () => {};
      removeEventListener("scroll", onScroll);
      removeEventListener("wheel", onWheel);
      removeEventListener("resize", onResize);
    };
  }, []);
}
