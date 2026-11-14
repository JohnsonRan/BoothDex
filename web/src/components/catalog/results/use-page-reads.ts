// The results' reads and the changes they bring (result-pages.tsx): every change keeps the first tile in view
// where it is, one above the reader waits for scrolling to stop, and a page is read at the end, into its box, or
// the rest of the server's page. The results' state outside React lives in one object of refs, made once.

import {
  type Dispatch,
  type SetStateAction,
  startTransition,
  useCallback,
  useRef,
  useState,
} from "react";
import { covered } from "@/components/catalog/band/band-edge";
import { useUi } from "@/components/providers";
import { api } from "@/lib/api";
import type { SearchResponse } from "@/lib/api-types";
import { type Filters, searchQuery } from "@/lib/catalog";
import { anchorAt, firstBelow } from "@/lib/result-frame";
import {
  boxesBefore,
  type Dir,
  pageStarts,
  type Page as ResultPage,
  type Snapshot,
  withFailed,
  withInto,
  withNext,
  withRest,
} from "@/lib/result-pages";
import type { Rates, Settings } from "@/lib/settings-core";
import { type Tile, tileOf } from "./item-tile";

export type Page = ResultPage<Tile>;

/** The index of the first of `kids[from, to)` reaching below `edge` (their bottoms ascend in grid order). */
export const firstInView = (
  kids: HTMLCollection,
  from: number,
  to: number,
  edge: number,
) => firstBelow(from, to, edge, (i) => kids[i].getBoundingClientRect().bottom);

/**
 * The results' refs, one object for the life of the results, so the hooks that share it (use-result-place.ts,
 * use-result-frame.ts) depend on nothing that changes.
 */
export function useResultRefs(pages: Page[]) {
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
  const tick = useRef(() => {});
  /** the pages last drawn and the columns they were drawn at, for the callbacks and the frame */
  const now = useRef({ pages, cols: 1 });
  const [refs] = useState(() => ({
    grid,
    end,
    top,
    nearTop,
    busy,
    anchor,
    scrolling,
    held,
    restoring,
    place,
    width,
    tick,
    now,
  }));
  return refs;
}

export type ResultRefs = ReturnType<typeof useResultRefs>;

export function usePageReads(
  r: ResultRefs,
  {
    filters,
    pageSize,
    lexical,
    total,
    settings,
    rates,
    setPages,
    setLoading,
    setFailed,
    setCols,
  }: {
    filters: Filters;
    pageSize: number;
    lexical: boolean;
    total: number;
    settings: Settings;
    rates: Rates;
    setPages: Dispatch<SetStateAction<Page[]>>;
    setLoading: Dispatch<SetStateAction<Dir | null>>;
    setFailed: Dispatch<SetStateAction<boolean>>;
    setCols: Dispatch<SetStateAction<number>>;
  },
) {
  const { t } = useUi();
  const path = useCallback(
    (n: number) =>
      `/api/search?${searchQuery(filters, n, pageSize)}&fields=card${lexical ? "&sem=0" : ""}`,
    [filters, pageSize, lexical],
  );
  /** the index of each page's first grid child and past the last page's, at the columns last drawn */
  const starts = useCallback(
    (p: Page[]) => pageStarts(p, pageSize, r.now.current.cols),
    [r, pageSize],
  );

  /**
   * Runs a change of what is drawn, keeping the first tile in view where it is (use-result-place.ts); a box or a
   * silhouette in view above it may be what the change replaces. With none in view, what stays above is kept.
   */
  const keep = useCallback(
    (change: () => void, sync = false) => {
      const g = r.grid.current;
      const p = r.now.current.pages;
      const at = starts(p);
      const drawn = at[at.length - 1];
      if (g) {
        const i = anchorAt(
          firstInView(g.children, at[0], drawn, covered()),
          drawn,
          innerHeight,
          (j) => g.children[j].getBoundingClientRect().top,
          (j) => !!(g.children[j] as HTMLElement).dataset.item,
        );
        const el = g.children[i];
        r.anchor.current = { el, at: el.getBoundingClientRect().top + scrollY };
      }
      if (sync) change();
      else startTransition(change);
    },
    [r, starts],
  );

  /** Puts a change in now, or once scrolling stops when it is above the reader; one per `key`. */
  const above = useCallback(
    (key: string, change: () => void, isAbove: boolean) => {
      if (isAbove && r.scrolling.current) r.held.current.set(key, change);
      else change();
    },
    [r],
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
      if (r.busy.current) return;
      r.busy.current = true;
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
        (res) => {
          const fresh = res.items.map((i) => tileOf(i, settings, rates, t));
          done(() => setPages((p) => place(p, fresh)));
        },
        () => {
          r.restoring.current = null;
          done(() =>
            inPlace ? setPages((p) => withFailed(p, n, true)) : setFailed(true),
          );
        },
      );
    },
    [r, path, keep, above, settings, rates, t, setPages, setLoading, setFailed],
  );

  const loadNext = useCallback(() => {
    const last = r.now.current.pages.at(-1);
    if (!last) return;
    if (last.partial) {
      const seen = new Set(last.tiles.map((i) => i.id));
      load(last.n, (p, fresh) => withRest(p, last.n, fresh, seen), "next");
    } else if (last.n < total)
      load(last.n + 1, (p, fresh) => withNext(p, last.n, fresh), "next");
  }, [r, load, total]);

  /** Reads page `n` into its box. */
  const loadInto = useCallback(
    (n: number, dir: Dir) =>
      load(n, (p, fresh) => withInto(p, n, fresh, dir), dir, true),
    [load],
  );

  /** The pages before `n`, at most ABOVE of them, not read yet. */
  const before = (n: number) => boxesBefore<Tile>(n);

  /**
   * The grid's columns, and from a tile drawn its height and the row pitch, which size the boxes standing for pages
   * not read and tiles not drawn yet; returns the tile's height. Read before the grid is drawn for new columns, the
   * height is the old layout's, so the results measure again once it is (result-pages.tsx).
   */
  const measure = () => {
    const g = r.grid.current;
    if (!g) return 0;
    const style = getComputedStyle(g);
    // read before the grid is drawn for fewer columns, a page's box spanning past the last one adds implicit columns
    // and the count is too high: each measure after a commit (result-pages.tsx) reads fewer, until none are left
    setCols(style.gridTemplateColumns.split(" ").length);
    const tile = g.querySelector<HTMLElement>("[data-item]");
    if (!tile) return 0;
    // the column lib/deep-start.ts gave the server's first tile goes: the pages put above it in the same commit
    // (use-result-place.ts) hold it there, and a later change of columns moves it with them
    tile.style.gridColumn = "";
    // a row is a tile's height, a box for one row too (PageTiles, a page's lead and tail); a page's whole rows add
    // a pitch, the height and the gap, for each row after the first (result-pages.tsx PageBoxes). The height is
    // its content's: a tile sharing a row with a box is stretched to the box, which would keep a wrong height
    const h =
      (tile.lastElementChild as Element).getBoundingClientRect().bottom -
      tile.getBoundingClientRect().top;
    g.style.setProperty("--_tile-h", `${h}px`);
    g.style.setProperty(
      "--_pitch",
      `${h + (Number.parseFloat(style.rowGap) || 0)}px`,
    );
    return h;
  };

  return {
    starts,
    keep,
    above,
    load,
    loadNext,
    loadInto,
    before,
    measure,
  };
}

export type PageReads = ReturnType<typeof usePageReads>;
