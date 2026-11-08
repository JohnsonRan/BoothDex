// The results' page state outside React (components/catalog/result-pages.tsx): the address of a results page,
// which page is in view, which pages are far enough to give up their tiles, and where Back or a reload returns to.

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

/** The settings a tile's text and price are drawn in (catalog/item-tile.tsx tileOf): tiles drawn in others are read again. */
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

/** The estimated height of a whole page not read yet: its rows at the measured row pitch, less the gap after the last. */
export const pageBlock = (
  pageSize: number,
  cols: number,
  pitch: number,
  gap: number,
) => Math.ceil(pageSize / cols) * pitch - gap;
