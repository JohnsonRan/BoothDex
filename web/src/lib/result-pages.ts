// The results' page state outside React (components/catalog/results/result-pages.tsx): the address of a results page,
// which page is in view, which pages are far enough to give up their tiles, where Back or a reload returns to, and
// the list of pages as the grid draws it.

import type { Settings } from "./settings-core";

/** Pages further than this many screens from the viewport hold placeholders; the next page loads this far ahead. */
export const SCREENS = 2;
/**
 * Pages put above a start past page 1 at once, as boxes; more go up as the reader nears the top of them. It keeps
 * the document far below the engines' element height limits (WebKit and Chromium near 33 million px, Firefox near
 * 18 million): 2,000 pages of 100 on a two-column phone would be about 30 million px.
 */
export const ABOVE = 40;
/** Loaded pages kept for a return to the results this long, before they are read again. */
const KEPT_MS = 5 * 60_000;

/** The settings a tile's text and price are drawn in (catalog/results/item-tile.tsx tileOf): tiles drawn in others are read again. */
export const drawnWith = (s: Settings) => `${s.ui} ${s.content} ${s.currency}`;

/**
 * `base` (a query without `page`, `sem` or `item`) for results page `page`: page 1 has no `page`, and later pages
 * carry `sem=0` when the first page went without meaning-based matches, so they come from the same result set.
 */
export function pageQuery(base: string, page: number, noSemantic: boolean) {
  return [
    base,
    page > 1 ? `page=${page}` : "",
    page > 1 && noSemantic ? "sem=0" : "",
  ]
    .filter(Boolean)
    .join("&");
}

const pairs = (search: string) => search.slice(1).split("&").filter(Boolean);

/** The pairs of a search that choose the results, byte for byte: all but `page`, `sem` and `item`. */
export const resultsQuery = (search: string) =>
  pairs(search)
    .filter((p) => !/^(page|sem|item)=/.test(p))
    .join("&");

/**
 * The address bar's search for results page `page`. The other pairs are kept byte for byte (the catalogue's
 * lists use a literal "+"), and an open item's `item` stays last (components/item/item-data.ts).
 */
export function withPage(search: string, page: number, noSemantic: boolean) {
  const query = [
    pageQuery(resultsQuery(search), page, noSemantic),
    pairs(search).find((p) => p.startsWith("item=")),
  ]
    .filter(Boolean)
    .join("&");
  return query ? `?${query}` : "";
}

/** The index of the page in view: the last whose first tile starts above `line`, given the starts in order. */
export const pageAt = (tops: number[], line: number) =>
  Math.max(
    0,
    tops.findLastIndex((top) => top <= line),
  );

/** A page spanning `top` to `bottom` (viewport px) is more than SCREENS screens of `height` from view. */
export const isFar = (top: number, bottom: number, height: number) =>
  bottom < -SCREENS * height || top > (1 + SCREENS) * height;

/** Where the reader was, kept in the history entry: the first tile below the filter band, its offset from the band's edge, and its page. */
export type Snapshot = { key: string; page: number; id: number; top: number };

export const SNAPSHOT = "boothdex-results";

export function readSnapshot(state: unknown, key: string): Snapshot | null {
  const s = (state as Record<string, Snapshot> | null)?.[SNAPSHOT];
  return s && s.key === key && s.page > 0 && s.id > 0 ? s : null;
}

/** The pages a reader left, for a return to the same results. */
export type Kept<P> = { key: string; pages: P[]; width: number; at: number };

/**
 * How to return to `snap`: the pages kept from the last visit when they are recent and laid out at this width
 * (placeholders keep their heights), else the anchor's page read again (the server's page when it is that one).
 */
export function restorePlan<P>(
  snap: Snapshot,
  kept: Kept<P> | undefined,
  width: number,
  now: number,
): { pages: P[] } | { page: number } {
  if (
    kept &&
    kept.key === snap.key &&
    kept.width === width &&
    now - kept.at < KEPT_MS
  )
    return { pages: kept.pages };
  return { page: snap.page };
}

/**
 * The boxes page `n` draws while it is not read: `lead` cells closing the row the page above ends in, `rows` whole
 * rows, and `tail` cells opening the next row. The page fills exactly its own cells of the whole result set (every
 * page before the last is full), so the next page's first tile lands in its own column.
 */
export function boxParts(n: number, pageSize: number, cols: number) {
  const lead = Math.min(
    pageSize,
    (cols - (((n - 1) * pageSize) % cols)) % cols,
  );
  return {
    lead,
    rows: Math.floor((pageSize - lead) / cols),
    tail: (pageSize - lead) % cols,
  };
}

/** The empty cells before the first page drawn, page `n`, so its first tile stands in its column of the whole set. */
export const leadOffset = (n: number, pageSize: number, cols: number) =>
  ((n - 1) * pageSize) % cols;

/** A results page as the grid draws it, with tiles of type `T`. */
export type Page<T> = {
  n: number;
  /** none: not read yet, drawn as the boxes of its cells (boxParts; near view: silhouette tiles) */
  tiles: T[];
  /** the server's first screen of its page; the rest is still to read */
  partial?: boolean;
  /** far from view: a box of each tile's height stands in for it */
  heights?: number[];
  /** its tiles come and go a few a frame (PageTiles): the change moves nothing in view */
  gradual?: boolean;
  near?: boolean;
  /** reading it failed: a notice to retry stands in its place */
  failed?: boolean;
};

/** Which way a read goes: the next page at the end, or a page above the reader. */
export type Dir = "next" | "prev";

/** The grid children a page draws at `cols` columns: its tiles, a notice, a page of silhouettes near view, else its boxes. */
export function kidsOf<T>(q: Page<T>, pageSize: number, cols: number) {
  if (q.tiles.length) return q.tiles.length;
  if (q.failed) return 1;
  if (q.near) return pageSize;
  const { lead, rows, tail } = boxParts(q.n, pageSize, cols);
  return [lead, rows, tail].filter(Boolean).length;
}

/**
 * The index of each page's first grid child, after the child offsetting the first page (leadOffset) when there is
 * one, and last the index past the last page's children.
 */
export function pageStarts<T>(
  pages: Page<T>[],
  pageSize: number,
  cols: number,
) {
  const at = [Number(leadOffset(pages[0].n, pageSize, cols) > 0)];
  for (const q of pages) at.push(at[at.length - 1] + kidsOf(q, pageSize, cols));
  return at;
}

/** The pages before `n`, at most ABOVE of them, not read yet. */
export function boxesBefore<T>(n: number): Page<T>[] {
  const from = Math.max(1, n - ABOVE);
  return Array.from({ length: n - from }, (_, i) => ({
    n: from + i,
    tiles: [],
  }));
}

/**
 * The pages before page `first` put above it while it leads `pages`, else `pages` as they are: run twice (React
 * Strict Mode runs a mount effect twice in development), it puts them up once.
 */
export const withAbove = <T>(pages: Page<T>[], first: number) =>
  pages[0].n === first ? [...boxesBefore<T>(first), ...pages] : pages;

/** The rest of partial page `n` read: the tiles not `seen` go after the server's, drawn a few a frame. */
export const withRest = <T extends { id: number }>(
  pages: Page<T>[],
  n: number,
  fresh: T[],
  seen: Set<number>,
): Page<T>[] =>
  pages.map((q) =>
    q.n === n && q.partial
      ? {
          n: q.n,
          tiles: [...q.tiles, ...fresh.filter((i) => !seen.has(i.id))],
          gradual: true,
        }
      : q,
  );

/** The page after `last` read: it goes at the end while `last` is still the last page. */
export const withNext = <T>(
  pages: Page<T>[],
  last: number,
  fresh: T[],
): Page<T>[] =>
  pages.at(-1)?.n === last
    ? [...pages, { n: last + 1, tiles: fresh, gradual: true }]
    : pages;

/** Page `n` read into its box, drawn a few a frame when it is below the reader. */
export const withInto = <T>(
  pages: Page<T>[],
  n: number,
  fresh: T[],
  dir: Dir,
): Page<T>[] =>
  pages.map((q) =>
    q.n === n && !q.tiles.length
      ? { n, tiles: fresh, gradual: dir === "next" }
      : q,
  );

/** Page `n` with its failure notice shown or taken away. */
export const withFailed = <T>(pages: Page<T>[], n: number, failed: boolean) =>
  pages.map((q) => (q.n === n ? { ...q, failed } : q));

/**
 * The pages at a new width: a page given up for boxes takes `tile` px per tile. The pages drawn reflow by
 * themselves, and the boxes of a page not read follow the columns and the measured row pitch.
 */
export const resized = <T>(pages: Page<T>[], tile: number) =>
  pages.map((q) =>
    q.heights ? { ...q, heights: q.heights.map(() => tile) } : q,
  );
