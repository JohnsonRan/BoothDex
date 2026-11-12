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
// during an iOS fling stops the fling dead. A page that arrives or comes back below the reader, or gives its tiles
// up for boxes of their heights, does it a few tiles a frame (PageTiles), so a fling never waits on a whole page's
// layout. Every place is measured from the filter band's bottom edge (band.tsx covered()), so the tile in view is
// the first one below the band, not one under it.

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
  anchorAt,
  firstBelow,
  framePages,
  pageToRead,
  restoreStep,
} from "@/lib/result-frame";
import {
  boxesBefore,
  type Dir,
  drawnWith,
  type Kept,
  kidsOf as kidsOfPage,
  pageAt,
  pageBlock,
  pageStarts,
  type Page as ResultPage,
  readSnapshot,
  resized,
  restorePlan,
  resultsQuery,
  SCREENS,
  SNAPSHOT,
  type Snapshot,
  withFailed,
  withInto,
  withNext,
  withPage,
  withRest,
} from "@/lib/result-pages";
import { LOCALES, type Rates, type Settings } from "@/lib/settings-core";
import { covered } from "./band-edge";
import { FIRST_TILES, GRID, type Tile, tileOf } from "./item-tile";
import { RefreshFailed } from "./refresh";
import { silhouetteMask, TileSilhouette } from "./silhouette";
import { Tiles } from "./tiles";

type Page = ResultPage<Tile>;

/**
 * The pages of the results last left, for a return to them (lib/result-pages restorePlan), with the settings their
 * tiles were drawn in and the place last saved.
 */
let kept: (Kept<Page> & { drawn: string; place: Snapshot | null }) | undefined;

const MARGIN = `${SCREENS * 100}%`;
// no scroll event for this long and scrolling has stopped: an engine sends one a frame while a fling or a smooth
// scroll runs, so a pause this long is the reader's, and the changes held for it go in soon after
const SETTLE_MS = 200;
// tiles a gradual page draws or gives up a frame: four rows at six columns. Each step lays the whole grid out
// again, so twelve a frame (nine steps a page) held the main thread over 100 ms more often than the page at once
const STEP = 24;
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
const firstInView = (
  kids: HTMLCollection,
  from: number,
  to: number,
  edge: number,
) => firstBelow(from, to, edge, (i) => kids[i].getBoundingClientRect().bottom);

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
  const kidsOf = useCallback((q: Page) => kidsOfPage(q, pageSize), [pageSize]);

  /**
   * Runs a change of what is drawn, keeping the first tile in view where it is (the layout effect below); a box
   * or a silhouette in view above it may be what the change replaces. With none in view, what stays above is kept.
   */
  const keep = useCallback(
    (change: () => void, sync = false) => {
      const g = grid.current;
      const drawn = now.current.pages.reduce((s, q) => s + kidsOf(q), 0);
      if (g && drawn) {
        const i = anchorAt(
          firstInView(g.children, 0, drawn, covered()),
          drawn,
          innerHeight,
          (j) => g.children[j].getBoundingClientRect().top,
          (j) => !!(g.children[j] as HTMLElement).dataset.item,
        );
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
            inPlace ? setPages((p) => withFailed(p, n, true)) : setFailed(true),
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
      load(last.n, (p, fresh) => withRest(p, last.n, fresh, seen), "next");
    } else if (last.n < total)
      load(last.n + 1, (p, fresh) => withNext(p, last.n, fresh), "next");
  }, [load, total]);

  /** Reads page `n` into its box. */
  const loadInto = useCallback(
    (n: number, dir: Dir) =>
      load(n, (p, fresh) => withInto(p, n, fresh, dir), dir, true),
    [load],
  );

  /** The pages before `n`, at most ABOVE of them, as boxes. */
  const before = (n: number) => boxesBefore<Tile>(n, block.current);

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
    // the height of a box standing for a tile not drawn yet (PageTiles)
    g.style.setProperty("--_tile-h", `${pitch - gap}px`);
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
    const offset = el
      ? el.getBoundingClientRect().top - edge - s.top
      : undefined;
    if (restoreStep(offset, p[p.length - 1], total) === "read") loadNext();
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
      const g = grid.current;
      const p = now.current.pages;
      const i = p.findIndex((q) => q.n === shown);
      if (!g || i < 0 || restoring.current) return false;
      if (resultsQuery(location.search) !== own) return true;
      const from = pageStarts(p, pageSize)[i];
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
      const at = pageStarts(p, pageSize);
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
      const next = framePages(
        p,
        {
          tops,
          bottoms,
          height: h,
          edge: c,
          inView: i,
          up,
          scrolling: scrolling.current,
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
      }, SETTLE_MS);
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
      keep(() => setPages((cur) => resized(cur, height, block.current)));
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
                    keep(() => setPages((p) => withFailed(p, q.n, false)))
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
            ) : (
              <PageTiles
                tiles={q.tiles}
                heights={q.heights}
                // a return draws its pages whole, so the place it restores is there to scroll to
                gradual={!!q.gradual && !restoring.current}
                from={q.n === page ? 0 : FIRST_TILES}
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

/**
 * A page's tiles, or while it is far from view a box of each tile's height in its place. A gradual page goes from
 * one to the other STEP tiles a frame, the rest standing as boxes (one not drawn yet as tall as a measured tile,
 * `--_tile-h`): a page of 100 tiles drawn in one commit held the main thread for its style and layout, about 0.2 s
 * (over 1 s at 4x CPU), mid-fling. Only a change that moves nothing in view is gradual (Page.gradual).
 */
function PageTiles({
  tiles,
  heights,
  gradual,
  from,
  favLabel,
}: {
  tiles: Tile[];
  heights?: number[];
  gradual: boolean;
  from: number;
  favLabel: string;
}) {
  const target = heights ? 0 : tiles.length;
  const [drawn, setDrawn] = useState(gradual ? 0 : target);
  if (!gradual && drawn !== target) setDrawn(target);
  // the heights the page last stood at, for its boxes as it is drawn again
  const last = useRef(heights);
  useEffect(() => {
    if (heights) last.current = heights;
  }, [heights]);
  useEffect(() => {
    if (drawn === target) return;
    const frame = requestAnimationFrame(() =>
      setDrawn((d) =>
        d < target ? Math.min(target, d + STEP) : Math.max(target, d - STEP),
      ),
    );
    return () => cancelAnimationFrame(frame);
  }, [drawn, target]);
  const box = heights ?? last.current;
  return (
    <>
      <Tiles
        tiles={drawn < tiles.length ? tiles.slice(0, drawn) : tiles}
        from={from}
        markViewed
        favLabel={favLabel}
      />
      {tiles.slice(drawn).map((tile, i) => (
        <GridSpan
          key={tile.id}
          aria-hidden
          className="page-box"
          style={{ blockSize: box?.[drawn + i] ?? "var(--_tile-h)" }}
        />
      ))}
    </>
  );
}
