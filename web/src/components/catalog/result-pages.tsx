"use client";
// The results as one endless grid (the table-filter template's useInfiniteBatches, page.tsx:1160-1237, with our
// /api/search pages for its timer). The server renders the page in the URL, its first screen of tiles in the HTML;
// the rest of that page, the next pages and, when the reader moves up, the previous ones are read here and drawn
// in the same grid, so rows run on across pages. The address bar names the page in view (replaced, never pushed),
// and the history entry keeps the first tile in view, so a reload or a return lands on it with the pages it had.
// Pages more than SCREENS screens from view give their tiles up for boxes of the same heights, and every change
// keeps the first tile in view where it was (the grid opts out of the browser's own scroll anchoring, which not
// every engine has, so all of them move the same way).

import { Grid, GridSpan } from "@astryxdesign/core/Grid";
import { VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import {
  Fragment,
  startTransition,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { useUi } from "@/components/providers";
import { api } from "@/lib/api";
import type { SearchResponse } from "@/lib/api-types";
import { type Filters, searchQuery } from "@/lib/catalog";
import { ROWS } from "@/lib/motion";
import {
  isFar,
  type Kept,
  pageAt,
  readSnapshot,
  restorePlan,
  SCREENS,
  SNAPSHOT,
  type Snapshot,
  withPage,
} from "@/lib/result-pages";
import { LOCALES, type Rates, type Settings } from "@/lib/settings-core";
import { FIRST_TILES, GRID, type Tile, tileOf } from "./item-tile";
import { RefreshFailed } from "./refresh";
import { TileSilhouette } from "./silhouette";
import { Tiles } from "./tiles";

type Page = {
  n: number;
  tiles: Tile[];
  /** the server's first screen of its page; the rest is still to read */
  partial?: boolean;
  /** far from view: a box of each tile's height stands in for it */
  heights?: number[];
};

type Dir = "next" | "prev";

/** The pages of the results last left, for a return to them (lib/result-pages restorePlan). */
let kept: Kept<Page> | undefined;

const MARGIN = `${SCREENS * 100}%`;

/** The index of the first of `kids[from, to)` reaching into the viewport (their bottoms ascend in grid order). */
function firstInView(kids: HTMLCollection, from: number, to: number) {
  let lo = from;
  let hi = to - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (kids[mid].getBoundingClientRect().bottom > 0) hi = mid;
    else lo = mid + 1;
  }
  return lo;
}

export function ResultPages({
  page,
  total,
  count,
  pageSize,
  tiles,
  partial,
  filters,
  lexical,
  settings,
  rates,
}: {
  /** the page the server rendered */
  page: number;
  total: number;
  count: number;
  pageSize: number;
  /** the server's tiles of `page` */
  tiles: Tile[];
  /** `page` has more results than `tiles` */
  partial: boolean;
  filters: Filters;
  /** the first page went without meaning-based matches: the others are read the same way */
  lexical: boolean;
  settings: Settings;
  rates: Rates;
}) {
  const { t } = useUi();
  const [pages, setPages] = useState<Page[]>(() => [
    { n: page, tiles, partial },
  ]);
  const [loading, setLoading] = useState<Dir | null>(null);
  const [failed, setFailed] = useState<Dir | null>(null);
  const [cols, setCols] = useState(1);
  const grid = useRef<HTMLDivElement>(null);
  const end = useRef<HTMLDivElement>(null);
  const top = useRef<HTMLDivElement>(null);
  const nearTop = useRef(false);
  const busy = useRef(false);
  const anchor = useRef<{ el: Element; at: number } | null>(null);
  const restoring = useRef<Snapshot | null>(null);
  const width = useRef(0);
  const tick = useRef(() => {});
  const now = useRef({ pages, failed });
  useLayoutEffect(() => {
    now.current = { pages, failed };
  });

  const key = `${searchQuery(filters, 1, pageSize)}${lexical ? "&sem=0" : ""}`;
  const path = useCallback(
    (n: number) =>
      `/api/search?${searchQuery(filters, n, pageSize)}&fields=card${lexical ? "&sem=0" : ""}`,
    [filters, pageSize, lexical],
  );

  /** Runs a change of what is drawn, keeping the first tile in view where it is (the layout effect below). */
  const keep = useCallback((change: () => void) => {
    const g = grid.current;
    const drawn = now.current.pages.reduce((s, p) => s + p.tiles.length, 0);
    if (g && drawn) {
      const el = g.children[firstInView(g.children, 0, drawn)];
      anchor.current = { el, at: el.getBoundingClientRect().top + scrollY };
    }
    startTransition(change);
  }, []);

  // template :1213-1225, the in-flight guard and the loading state, with the page read for real
  const load = useCallback(
    (n: number, place: (p: Page[], fresh: Tile[]) => Page[], dir: Dir) => {
      if (busy.current) return;
      busy.current = true;
      setLoading(dir);
      api<SearchResponse>(path(n)).then(
        (r) => {
          busy.current = false;
          const fresh = r.items.map((i) => tileOf(i, settings, rates, t));
          keep(() => {
            setLoading(null);
            setPages((p) => place(p, fresh));
          });
        },
        () => {
          busy.current = false;
          restoring.current = null;
          keep(() => {
            setLoading(null);
            setFailed(dir);
          });
        },
      );
    },
    [path, keep, settings, rates, t],
  );

  const loadNext = useCallback(() => {
    const last = now.current.pages.at(-1);
    if (!last) return;
    if (last.partial) {
      const seen = new Set(last.tiles.map((i) => i.id));
      load(
        last.n,
        (p, fresh) =>
          p.map((q) =>
            q.n === last.n && q.partial
              ? {
                  n: q.n,
                  tiles: [...q.tiles, ...fresh.filter((i) => !seen.has(i.id))],
                }
              : q,
          ),
        "next",
      );
    } else if (last.n < total)
      load(
        last.n + 1,
        (p, fresh) =>
          p.at(-1)?.n === last.n ? [...p, { n: last.n + 1, tiles: fresh }] : p,
        "next",
      );
  }, [load, total]);

  const loadPrev = useCallback(() => {
    const n = now.current.pages[0].n - 1;
    if (n >= 1)
      load(
        n,
        (p, fresh) => (p[0].n === n + 1 ? [{ n, tiles: fresh }, ...p] : p),
        "prev",
      );
  }, [load]);

  /**
   * Scrolls the restored tile to where it was. Short of it (the tile's page not yet whole, or too little below it to
   * scroll that far) the next part is read and this runs again; with nothing left to read it stops where it is.
   */
  const position = useCallback(() => {
    const s = restoring.current;
    if (!s) return;
    const el = grid.current?.querySelector(`[data-item="${s.id}"]`);
    if (el) scrollTo(0, el.getBoundingClientRect().top + scrollY - s.top);
    const last = now.current.pages[now.current.pages.length - 1];
    if (el && Math.abs(el.getBoundingClientRect().top - s.top) < 1)
      restoring.current = null;
    else if (last.partial || (el && last.n < total)) loadNext();
    else restoring.current = null;
  }, [loadNext, total]);

  // A reload, Back or Forward to these results: the entry says where the reader was. Before the sentinels below,
  // so a page read for the return goes first. Leaving keeps the pages for a return.
  // biome-ignore lint/correctness/useExhaustiveDependencies: once per visit
  useEffect(() => {
    history.scrollRestoration = "manual";
    width.current = grid.current?.clientWidth ?? 0;
    const snap = readSnapshot(history.state, key);
    if (snap) {
      restoring.current = snap;
      const plan = restorePlan(snap, kept, width.current, Date.now());
      // the same pages when the results were only hidden (a route kept in the background) and come back
      if ("pages" in plan && plan.pages !== now.current.pages)
        startTransition(() => setPages(plan.pages));
      else if ("page" in plan && plan.page !== page)
        load(plan.page, (_, fresh) => [{ n: plan.page, tiles: fresh }], "next");
      else position();
    }
    return () => {
      kept = {
        key,
        pages: now.current.pages,
        width: width.current,
        at: Date.now(),
      };
      history.scrollRestoration = "auto";
    };
  }, []);

  // biome-ignore lint/correctness/useExhaustiveDependencies: runs after each change of what is drawn
  useLayoutEffect(() => {
    const a = anchor.current;
    anchor.current = null;
    if (restoring.current) position();
    else if (a?.el.isConnected) {
      const moved = a.el.getBoundingClientRect().top + scrollY - a.at;
      if (moved) scrollBy(0, moved);
    }
    tick.current();
  }, [pages, failed, position]);

  const last = pages[pages.length - 1];
  const hasNext = !!last.partial || last.n < total;
  const hasPrev = pages[0].n > 1;

  // template :2356-2376: the sentinel under the last row loads the next page, observed again after each one
  useEffect(() => {
    const el = end.current;
    if (!el || !hasNext || loading || failed) return;
    const seen = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) loadNext();
      },
      { rootMargin: `0px 0px ${MARGIN} 0px` },
    );
    seen.observe(el);
    return () => seen.disconnect();
  }, [hasNext, loading, failed, loadNext]);

  // the one above the first row only marks the top as near: the previous page loads when the reader moves up
  useEffect(() => {
    const el = top.current;
    if (!el || !hasPrev) return;
    const seen = new IntersectionObserver(
      ([e]) => {
        nearTop.current = e.isIntersecting;
      },
      { rootMargin: `${MARGIN} 0px 0px 0px` },
    );
    seen.observe(el);
    return () => {
      nearTop.current = false;
      seen.disconnect();
    };
  }, [hasPrev]);

  // One frame after scrolling: the page in view goes to the address bar, far pages give up their tiles and near
  // ones take them back, and the previous page loads when the reader moves up. The entry records where the
  // reader is when a page is crossed and when scrolling settles.
  // biome-ignore lint/correctness/useExhaustiveDependencies: reads the latest state through `now`
  useEffect(() => {
    let frame = 0;
    let settle = 0;
    let lastY = scrollY;
    let up = false;
    let shown = page;
    const starts = (p: Page[]) => {
      let at = 0;
      return p.map((q) => {
        const s = at;
        at += q.tiles.length;
        return s;
      });
    };
    const save = () => {
      const g = grid.current;
      const p = now.current.pages;
      const i = p.findIndex((q) => q.n === shown);
      if (!g || i < 0 || restoring.current) return;
      const from = starts(p)[i];
      const el = g.children[
        firstInView(g.children, from, from + p[i].tiles.length)
      ] as HTMLElement | undefined;
      const id = Number(el?.dataset.item);
      if (!el || !id) return;
      const snap: Snapshot = {
        key,
        page: shown,
        id,
        top: el.getBoundingClientRect().top,
      };
      history.replaceState(
        { [SNAPSHOT]: snap },
        "",
        `${location.pathname}${withPage(location.search, shown, lexical)}`,
      );
    };
    const run = () => {
      frame = 0;
      const g = grid.current;
      const { pages: p, failed } = now.current;
      if (!g) return;
      if (up && nearTop.current && !failed && p[0].n > 1) loadPrev();
      const kids = g.children;
      const at = starts(p);
      const h = innerHeight;
      const tops = at.map((s) => kids[s].getBoundingClientRect().top);
      const lastTile =
        kids[at[at.length - 1] + p[p.length - 1].tiles.length - 1];
      const bottoms = [
        ...tops.slice(1),
        lastTile?.getBoundingClientRect().bottom ?? 0,
      ];
      const inView = p[pageAt(tops, h / 2)].n;
      if (inView !== shown && !restoring.current) {
        shown = inView;
        save();
      }
      const focused = document.activeElement;
      const next = p.map((q, i) => {
        const far = isFar(tops[i], bottoms[i], h);
        if (far === !!q.heights) return q;
        if (!far) return { ...q, heights: undefined };
        const own = [...kids].slice(at[i], at[i] + q.tiles.length);
        if (focused && own.some((k) => k.contains(focused))) return q;
        return {
          ...q,
          heights: own.map((k) => k.getBoundingClientRect().height),
        };
      });
      if (next.some((q, i) => q !== p[i]))
        keep(() => setPages((cur) => (cur === p ? next : cur)));
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(run);
    };
    const onScroll = () => {
      up = scrollY < lastY;
      lastY = scrollY;
      schedule();
      clearTimeout(settle);
      settle = window.setTimeout(save, 200);
    };
    const onWheel = (e: WheelEvent) => {
      if (e.deltaY >= 0) return;
      up = true;
      schedule();
    };
    // a new width reflows the rows: the boxes take the height of a tile laid out at it
    const onResize = () => {
      const g = grid.current;
      if (!g || g.clientWidth === width.current) return;
      width.current = g.clientWidth;
      setCols(getComputedStyle(g).gridTemplateColumns.split(" ").length);
      const tile = g.querySelector("[data-item]");
      const height = tile?.getBoundingClientRect().height ?? 0;
      keep(() =>
        setPages((cur) =>
          cur.map((q) =>
            q.heights ? { ...q, heights: q.heights.map(() => height) } : q,
          ),
        ),
      );
    };
    const g = grid.current;
    if (g) setCols(getComputedStyle(g).gridTemplateColumns.split(" ").length);
    tick.current = schedule;
    addEventListener("scroll", onScroll, { passive: true });
    addEventListener("wheel", onWheel, { passive: true });
    addEventListener("resize", onResize);
    return () => {
      cancelAnimationFrame(frame);
      clearTimeout(settle);
      tick.current = () => {};
      removeEventListener("scroll", onScroll);
      removeEventListener("wheel", onWheel);
      removeEventListener("resize", onResize);
    };
  }, []);

  const favLabel = t("fav.add");
  return (
    <>
      {failed === "prev" && (
        <RefreshFailed
          starting={false}
          onRetry={() => {
            keep(() => setFailed(null));
            loadPrev();
          }}
        />
      )}
      <Grid
        ref={grid}
        {...ROWS}
        gap={GRID.gap}
        style={{ ...GRID.style, position: "relative", overflowAnchor: "none" }}
      >
        {pages.map((q) => (
          <Fragment key={q.n}>
            {q.heights ? (
              q.heights.map((h, i) => (
                // biome-ignore lint/suspicious/noArrayIndexKey: one box per tile, in the tiles' order
                <div key={i} aria-hidden style={{ blockSize: h }} />
              ))
            ) : (
              <Tiles
                tiles={q.tiles}
                from={q.n === page ? 0 : FIRST_TILES}
                markViewed
                favLabel={favLabel}
              />
            )}
          </Fragment>
        ))}
        {hasNext &&
          (failed === "next" ? (
            <GridSpan columns="full">
              <RefreshFailed
                starting={false}
                onRetry={() => keep(() => setFailed(null))}
              />
            </GridSpan>
          ) : (
            Array.from({ length: cols }, (_, i) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: a fixed row of silhouettes
              <TileSilhouette key={i} ref={i === 0 ? end : undefined} />
            ))
          ))}
        {hasPrev && (
          <div
            ref={top}
            aria-hidden
            style={{ position: "absolute", insetBlockStart: 0 }}
          />
        )}
      </Grid>
      {/* template :4527-4537, once every page is on screen */}
      {!hasNext && pages[0].n === 1 && total > 1 && (
        <VStack minHeight={56} vAlign="center" hAlign="center">
          <Text type="supporting" color="secondary">
            {t("list.allLoaded", {
              n: count.toLocaleString(LOCALES[settings.ui]),
            })}
          </Text>
        </VStack>
      )}
    </>
  );
}
