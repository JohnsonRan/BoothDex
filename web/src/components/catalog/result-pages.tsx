"use client";
// The results as one endless grid (the table-filter template's useInfiniteBatches, page.tsx:1160-1237, with our
// /api/search pages for its timer). The server renders the page in the URL, its first screen of tiles in the HTML;
// the rest of that page and the next pages are read here and drawn in the same grid, so rows run on across pages.
// A start past page 1 puts the pages before it above it as boxes of a page's estimated height (its rows at the row
// pitch measured from the server's tiles), the window scrolled by as much before the first paint, so the
// scrollbar shows the place and scrolling up just works: the page in view is read into its box, whichever it is.
// Only ABOVE pages are put up at once; more join as the reader nears the top of them, which keeps the document
// within every engine's element height (2,000 pages on a two-column phone would be about 30 million px).
// The address bar names the page in view (replaced, never pushed), and the history entry keeps the first tile in
// view, so a reload or a return lands on it with the pages it had.
// Pages more than SCREENS screens from view give their tiles up for boxes of the same heights (a page not read
// yet is one box; near view, silhouette tiles); a box paints the tiles' silhouettes (silhouette.tsx
// silhouetteMask), so no speed of scrolling shows an empty page, and every change keeps the first tile in view
// where it was (the grid opts out of the browser's own scroll anchoring, which not every engine has, so all of
// them move the same way). A change above the reader waits for scrolling to stop: moving the scroll position
// during an iOS fling stops the fling dead. Every place is measured from the filter band's bottom edge (band.tsx
// covered()), so the tile in view is the first one below the band, not one under it.

import { Grid, GridSpan } from "@astryxdesign/core/Grid";
import { VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import {
  type CSSProperties,
  Fragment,
  startTransition,
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import { useUi } from "@/components/providers";
import { api } from "@/lib/api";
import type { SearchResponse } from "@/lib/api-types";
import { type Filters, searchQuery } from "@/lib/catalog";
import { ROWS } from "@/lib/motion";
import {
  ABOVE,
  drawnWith,
  isFar,
  type Kept,
  pageAt,
  pageBlock,
  readSnapshot,
  restorePlan,
  resultsQuery,
  SCREENS,
  SNAPSHOT,
  type Snapshot,
  withPage,
} from "@/lib/result-pages";
import { LOCALES, type Rates, type Settings } from "@/lib/settings-core";
import { covered } from "./band";
import { FIRST_TILES, GRID, type Tile, tileOf } from "./item-tile";
import { RefreshFailed } from "./refresh";
import { silhouetteMask, TileSilhouette } from "./silhouette";
import { Tiles } from "./tiles";

type Page = {
  n: number;
  /** none: not read yet, drawn as one box of `block` px (near view: silhouette tiles) */
  tiles: Tile[];
  /** the server's first screen of its page; the rest is still to read */
  partial?: boolean;
  /** far from view: a box of each tile's height stands in for it */
  heights?: number[];
  block?: number;
  near?: boolean;
  /** reading it failed: a notice to retry stands in its place */
  failed?: boolean;
};

type Dir = "next" | "prev";

/**
 * The pages of the results last left, for a return to them (lib/result-pages restorePlan), with the settings their
 * tiles were drawn in and the place last saved.
 */
let kept: (Kept<Page> & { drawn: string; place: Snapshot | null }) | undefined;

const MARGIN = `${SCREENS * 100}%`;
// a silhouette in the first column, out of the flow and unseen, measured for the boxes' mask
const PROBE: CSSProperties = {
  position: "absolute",
  gridArea: "1 / 1 / 2 / 2",
  insetInline: 0,
  insetBlockStart: 0,
  visibility: "hidden",
};

/** The page in view while the results are on screen (the one in the address), for the readout beside the count. */
let pageInView: number | undefined;
const inViewWatchers = new Set<() => void>();
function showInView(n: number | undefined) {
  pageInView = n;
  for (const w of inViewWatchers) w();
}
const watchInView = (w: () => void) => {
  inViewWatchers.add(w);
  return () => inViewWatchers.delete(w);
};

/**
 * "Page N" before the count in the filter row's readout (results.tsx), from page 2 on: the server's page in its
 * HTML, then the page in view as the reader scrolls, so it moves with the address.
 */
export function PageInView({ page }: { page: number }) {
  const { t } = useUi();
  const n = useSyncExternalStore(
    watchInView,
    () => pageInView ?? page,
    () => page,
  );
  if (n < 2) return null;
  return (
    <>
      <Text type="supporting" color="secondary" hasTabularNumbers>
        {t("list.page", { n })}
      </Text>
      <Text type="supporting" color="secondary">
        ·
      </Text>
    </>
  );
}

/** The index of the first of `kids[from, to)` reaching below `edge` (their bottoms ascend in grid order). */
function firstInView(
  kids: HTMLCollection,
  from: number,
  to: number,
  edge: number,
) {
  let lo = from;
  let hi = to - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (kids[mid].getBoundingClientRect().bottom > edge) hi = mid;
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
  const [failed, setFailed] = useState(false);
  const [cols, setCols] = useState(1);
  const grid = useRef<HTMLDivElement>(null);
  const end = useRef<HTMLDivElement>(null);
  const top = useRef<HTMLDivElement>(null);
  const nearTop = useRef(false);
  const busy = useRef(false);
  const anchor = useRef<{ el: Element; at: number } | null>(null);
  const scrolling = useRef(false);
  /** changes above the reader made while scrolling, put in once scrolling stops */
  const held = useRef(new Map<string, () => void>());
  const restoring = useRef<Snapshot | null>(null);
  /** the place last saved to the history entry */
  const place = useRef<Snapshot | null>(null);
  const width = useRef(0);
  // the boxes' tile mask, for the look it was measured in; a hidden silhouette stands until it is measured
  const [mask, setMask] = useState<{ look: string; style: CSSProperties }>();
  const probe = useRef<HTMLDivElement>(null);
  const tileMask = mask?.look === settings.style ? mask.style : undefined;
  useLayoutEffect(() => {
    if (tileMask || !probe.current || !grid.current) return;
    setMask({
      look: settings.style,
      style: silhouetteMask(probe.current, grid.current),
    });
  });
  /** the estimated height of a whole page not read yet */
  const block = useRef(0);
  const tick = useRef(() => {});
  const now = useRef({ pages });
  useLayoutEffect(() => {
    now.current = { pages };
  });

  const key = `${searchQuery(filters, 1, pageSize)}${lexical ? "&sem=0" : ""}`;
  const path = useCallback(
    (n: number) =>
      `/api/search?${searchQuery(filters, n, pageSize)}&fields=card${lexical ? "&sem=0" : ""}`,
    [filters, pageSize, lexical],
  );
  /** the grid children a page draws */
  const kidsOf = useCallback(
    (q: Page) => q.tiles.length || (q.near && !q.failed ? pageSize : 1),
    [pageSize],
  );
  /** pages `from` to `to` as boxes */
  const boxes = (from: number, to: number): Page[] =>
    Array.from({ length: to - from + 1 }, (_, i) => ({
      n: from + i,
      tiles: [],
      block: block.current,
    }));

  /**
   * Runs a change of what is drawn, keeping the first tile in view where it is (the layout effect below); a box
   * or a silhouette in view above it may be what the change replaces. With none in view, what stays above is kept.
   */
  const keep = useCallback(
    (change: () => void, sync = false) => {
      const g = grid.current;
      const drawn = now.current.pages.reduce((s, q) => s + kidsOf(q), 0);
      if (g && drawn) {
        let i = firstInView(g.children, 0, drawn, covered());
        for (let j = i; j < drawn; j++) {
          const k = g.children[j] as HTMLElement;
          if (k.getBoundingClientRect().top > innerHeight) break;
          if (k.dataset.item) {
            i = j;
            break;
          }
        }
        const el = g.children[i];
        anchor.current = { el, at: el.getBoundingClientRect().top + scrollY };
      }
      if (sync) change();
      else startTransition(change);
    },
    [kidsOf],
  );

  /** Puts a change in now, or once scrolling stops when it is above the reader; one per `key`. */
  const above = useCallback(
    (key: string, change: () => void, isAbove: boolean) => {
      if (isAbove && scrolling.current) held.current.set(key, change);
      else change();
    },
    [],
  );

  // template :1213-1225, the in-flight guard and the loading state, with the page read for real. A page read
  // into its box (`inPlace`) shows its own failure there; the next page's shows at the end.
  const load = useCallback(
    (
      n: number,
      place: (p: Page[], fresh: Tile[]) => Page[],
      dir: Dir,
      inPlace = false,
    ) => {
      if (busy.current) return;
      busy.current = true;
      setLoading(dir);
      const done = (change: () => void) =>
        above(
          `page ${n}`,
          () => {
            keep(() => {
              setLoading(null);
              change();
            });
          },
          dir === "prev",
        );
      api<SearchResponse>(path(n)).then(
        (r) => {
          const fresh = r.items.map((i) => tileOf(i, settings, rates, t));
          done(() => setPages((p) => place(p, fresh)));
        },
        () => {
          restoring.current = null;
          done(() =>
            inPlace
              ? setPages((p) =>
                  p.map((q) => (q.n === n ? { ...q, failed: true } : q)),
                )
              : setFailed(true),
          );
        },
      );
    },
    [path, keep, above, settings, rates, t],
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

  /** Reads page `n` into its box. */
  const loadInto = useCallback(
    (n: number, dir: Dir) =>
      load(
        n,
        (p, fresh) =>
          p.map((q) =>
            q.n === n && !q.tiles.length ? { n, tiles: fresh } : q,
          ),
        dir,
        true,
      ),
    [load],
  );

  /** The pages before `n`, at most ABOVE of them, as boxes. */
  const before = (n: number) => boxes(Math.max(1, n - ABOVE), n - 1);

  /** The estimated height of a page from the tiles drawn: its rows at the measured row pitch. */
  const measure = () => {
    const g = grid.current;
    const kids = g?.querySelectorAll("[data-item]");
    if (!g || !kids?.length) return;
    const style = getComputedStyle(g);
    const n = style.gridTemplateColumns.split(" ").length;
    const gap = Number.parseFloat(style.rowGap) || 0;
    const a = kids[0].getBoundingClientRect();
    const pitch = kids[n]
      ? kids[n].getBoundingClientRect().top - a.top
      : a.height + gap;
    block.current = pageBlock(pageSize, n, pitch, gap);
  };

  /**
   * Scrolls the restored tile to where it was. Short of it (the tile's page not yet whole, or too little below it
   * to scroll that far) the next part is read and this runs again; with nothing left to read it stops there.
   */
  const position = useCallback(() => {
    const s = restoring.current;
    if (!s) return;
    const el = grid.current?.querySelector(`[data-item="${s.id}"]`);
    const edge = covered();
    if (el)
      scrollTo(0, el.getBoundingClientRect().top + scrollY - edge - s.top);
    const p = now.current.pages;
    const last = p[p.length - 1];
    if (el && Math.abs(el.getBoundingClientRect().top - edge - s.top) < 1)
      restoring.current = null;
    else if (last.partial || (el && last.n < total)) loadNext();
    else restoring.current = null;
  }, [loadNext, total]);

  // before the effect below, so the pages it puts in on the first render are kept in place
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

  // A reload, Back or Forward to these results: the entry says where the reader was. Else a start past page 1:
  // the pages above go in before the first paint, the window scrolled by their height. Before the sentinels
  // below, so a page read for the return goes first. Leaving keeps the pages for a return.
  // biome-ignore lint/correctness/useExhaustiveDependencies: once per visit
  useLayoutEffect(() => {
    history.scrollRestoration = "manual";
    width.current = grid.current?.clientWidth ?? 0;
    measure();
    const drawn = drawnWith(settings);
    // New settings (a language or currency change) remount the results through a router refresh, which writes a
    // history entry of Next's own without the place: the place these results last saved goes on from here.
    const snap =
      readSnapshot(history.state, key) ??
      (kept?.key === key && kept.drawn !== drawn ? kept.place : null);
    place.current = snap;
    const plan =
      snap &&
      restorePlan(
        snap,
        kept?.drawn === drawn ? kept : undefined,
        width.current,
        Date.now(),
      );
    if (snap) restoring.current = snap;
    // the same pages when the results were only hidden (a route kept in the background) and come back
    if (plan && "pages" in plan) {
      if (plan.pages !== now.current.pages)
        startTransition(() => setPages(plan.pages));
      else position();
    } else if (plan && plan.page !== page)
      load(
        plan.page,
        (_, fresh) => [...before(plan.page), { n: plan.page, tiles: fresh }],
        "next",
      );
    else if (page > 1)
      keep(() => setPages((p) => [...before(page), ...p]), true);
    else if (snap) position();
    return () => {
      kept = {
        key,
        drawn,
        place: place.current,
        pages: now.current.pages,
        width: width.current,
        at: Date.now(),
      };
      history.scrollRestoration = "auto";
    };
  }, []);

  // a read is over once what it brought is drawn: before that, the next frame would still find its page unread
  useLayoutEffect(() => {
    if (!loading) busy.current = false;
  }, [loading]);

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

  // the one above the first row marks the top of the boxes as near: more go up when the reader moves up. Its
  // margin is screens of the viewport's height, so a new size (a rotation) sets it again.
  useEffect(() => {
    const el = top.current;
    if (!el || !hasPrev) return;
    let seen: IntersectionObserver | undefined;
    const watch = () => {
      seen?.disconnect();
      seen = new IntersectionObserver(
        ([e]) => {
          nearTop.current = e.isIntersecting;
        },
        { rootMargin: `${SCREENS * innerHeight - covered()}px 0px 0px 0px` },
      );
      seen.observe(el);
    };
    watch();
    addEventListener("resize", watch);
    return () => {
      nearTop.current = false;
      removeEventListener("resize", watch);
      seen?.disconnect();
    };
  }, [hasPrev]);

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
    const starts = (p: Page[]) => {
      let at = 0;
      return p.map((q) => {
        const s = at;
        at += kidsOf(q);
        return s;
      });
    };
    const save = () => {
      const g = grid.current;
      const p = now.current.pages;
      const i = p.findIndex((q) => q.n === shown);
      if (!g || i < 0 || restoring.current) return false;
      if (resultsQuery(location.search) !== own) return true;
      const from = starts(p)[i];
      const edge = covered();
      const el = g.children[
        firstInView(g.children, from, from + kidsOf(p[i]), edge)
      ] as HTMLElement | undefined;
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
      place.current = snap;
      history.replaceState(
        { ...history.state, [SNAPSHOT]: snap },
        "",
        `${location.pathname}${withPage(location.search, shown, lexical)}`,
      );
      return true;
    };
    const run = () => {
      frame = 0;
      const g = grid.current;
      const { pages: p } = now.current;
      if (!g) return;
      const first = p[0].n;
      if (up && nearTop.current && first > 1)
        above(
          "boxes",
          () =>
            keep(() =>
              setPages((cur) =>
                cur[0].n === first ? [...before(first), ...cur] : cur,
              ),
            ),
          true,
        );
      const kids = g.children;
      const at = starts(p);
      const h = innerHeight;
      const tops = at.map((s) => kids[s].getBoundingClientRect().top);
      const lastKid = kids[at[at.length - 1] + kidsOf(p[p.length - 1]) - 1];
      const bottoms = [
        ...tops.slice(1),
        lastKid?.getBoundingClientRect().bottom ?? 0,
      ];
      // the middle of what the band leaves in view
      const c = covered();
      const i = pageAt(tops, (c + h) / 2);
      if (!restoring.current && (p[i].n !== shown || !saved)) {
        if (p[i].n !== shown) {
          shown = p[i].n;
          showInView(shown);
        }
        saved = save();
      }
      const focused = document.activeElement;
      const next = p.map((q, j) => {
        const far = isFar(tops[j], bottoms[j], h);
        if (!q.tiles.length) {
          // a box above the page in view turns to silhouettes only when the reader moves up: their rows run on
          // into the page below and reflow it. One starting above the reader (another height above the tile in
          // view) waits for scrolling to stop.
          const near = !far && (j >= i || up);
          if (near === !!q.near || (tops[j] < c && scrolling.current)) return q;
          return { ...q, near };
        }
        if (far === !!q.heights) return q;
        if (!far) return { ...q, heights: undefined };
        const own = [...kids].slice(at[j], at[j] + q.tiles.length);
        if (focused && own.some((k) => k.contains(focused))) return q;
        return {
          ...q,
          heights: own.map((k) => k.getBoundingClientRect().height),
        };
      });
      if (next.some((q, j) => q !== p[j]))
        keep(() => setPages((cur) => (cur === p ? next : cur)));
      // the page in view, else the nearest one, not read yet
      const unread = p
        .map((_, j) => j)
        .filter((j) => !p[j].tiles.length && !p[j].failed && next[j].near);
      const j = unread.sort((a, b) => Math.abs(a - i) - Math.abs(b - i))[0];
      if (j !== undefined && !restoring.current)
        loadInto(p[j].n, tops[j] < c ? "prev" : "next");
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(run);
    };
    const onScroll = () => {
      up = scrollY < lastY;
      lastY = scrollY;
      scrolling.current = true;
      schedule();
      clearTimeout(settle);
      settle = window.setTimeout(() => {
        scrolling.current = false;
        saved = save();
        for (const change of held.current.values()) change();
        held.current.clear();
        schedule();
      }, 200);
    };
    const onWheel = (e: WheelEvent) => {
      if (e.deltaY >= 0) return;
      up = true;
      schedule();
    };
    // a new width reflows the rows: the boxes take the height of a tile, or of a page, laid out at it
    const onResize = () => {
      const g = grid.current;
      if (!g || g.clientWidth === width.current) return;
      width.current = g.clientWidth;
      setCols(getComputedStyle(g).gridTemplateColumns.split(" ").length);
      setMask(undefined);
      measure();
      const tile = g.querySelector("[data-item]");
      const height = tile?.getBoundingClientRect().height ?? 0;
      keep(() =>
        setPages((cur) =>
          cur.map((q) =>
            q.heights
              ? { ...q, heights: q.heights.map(() => height) }
              : q.tiles.length
                ? q
                : { ...q, block: block.current },
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
      showInView(undefined);
      tick.current = () => {};
      removeEventListener("scroll", onScroll);
      removeEventListener("wheel", onWheel);
      removeEventListener("resize", onResize);
    };
  }, []);

  const favLabel = t("fav.add");
  return (
    <>
      <Grid
        ref={grid}
        {...ROWS}
        gap={GRID.gap}
        style={{
          ...GRID.style,
          ...tileMask,
          position: "relative",
          overflowAnchor: "none",
        }}
      >
        {pages.map((q) => (
          <Fragment key={q.n}>
            {q.failed ? (
              <GridSpan columns="full">
                <RefreshFailed
                  starting={false}
                  onRetry={() =>
                    keep(() =>
                      setPages((p) =>
                        p.map((r) =>
                          r.n === q.n ? { ...r, failed: false } : r,
                        ),
                      ),
                    )
                  }
                />
              </GridSpan>
            ) : !q.tiles.length ? (
              q.near ? (
                Array.from({ length: pageSize }, (_, i) => (
                  // biome-ignore lint/suspicious/noArrayIndexKey: a page of silhouettes
                  <TileSilhouette key={i} />
                ))
              ) : (
                <GridSpan
                  columns="full"
                  aria-hidden
                  className="page-box"
                  style={{ blockSize: q.block }}
                />
              )
            ) : q.heights ? (
              q.heights.map((h, i) => (
                <GridSpan
                  // biome-ignore lint/suspicious/noArrayIndexKey: one box per tile, in the tiles' order
                  key={i}
                  aria-hidden
                  className="page-box"
                  style={{ blockSize: h }}
                />
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
          (failed ? (
            <GridSpan columns="full">
              <RefreshFailed
                starting={false}
                onRetry={() => keep(() => setFailed(false))}
              />
            </GridSpan>
          ) : (
            Array.from({ length: cols }, (_, i) => (
              // new ones after each page: the old ones go with the wait they stood for, instead of moving down
              // past the tiles that arrived (a layout shift)
              <TileSilhouette
                // biome-ignore lint/suspicious/noArrayIndexKey: a fixed row of silhouettes
                key={`${last.n}${last.partial ? "+" : ""}-${i}`}
                ref={i === 0 ? end : undefined}
              />
            ))
          ))}
        {hasPrev && (
          <GridSpan
            ref={top}
            aria-hidden
            style={{ position: "absolute", insetBlockStart: 0 }}
          />
        )}
        {/* last, as the sentinel: the children before them are the pages' */}
        {!tileMask && <TileSilhouette ref={probe} style={PROBE} />}
      </Grid>
      {/* template :4527-4537, once every page is on screen */}
      {!hasNext && pages.every((q) => q.tiles.length) && total > 1 && (
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
