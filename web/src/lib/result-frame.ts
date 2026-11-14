// The results' decisions one frame after scrolling (components/catalog/results/result-pages.tsx), outside React and the
// DOM: which grid child the reader sees, what each page draws for where it stands, which page to read next, and
// whether a return has reached its place. The caller measures; these only decide.

import { isFar, type Page } from "./result-pages";

/**
 * The index of the first of the children `[from, to)` reaching below `edge`, given each one's bottom (they ascend
 * in grid order); the last of them when none does.
 */
export function firstBelow(
  from: number,
  to: number,
  edge: number,
  bottom: (i: number) => number,
) {
  let lo = from;
  let hi = to - 1;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (bottom(mid) > edge) hi = mid;
    else lo = mid + 1;
  }
  return lo;
}

/**
 * The child to keep in place through a change: the first tile from `from` on that starts within the viewport's
 * `height`, else `from` itself (a box or a silhouette in view, which the change may replace).
 */
export function anchorAt(
  from: number,
  to: number,
  height: number,
  top: (i: number) => number,
  isTile: (i: number) => boolean,
) {
  for (let j = from; j < to; j++) {
    if (top(j) > height) break;
    if (isTile(j)) return j;
  }
  return from;
}

/** Where the pages stand this frame (viewport px), and whether the reader is scrolling. */
export type Frame = {
  /** each page's first child's top, and its end (the next page's top, the last child's bottom for the last) */
  tops: number[];
  bottoms: number[];
  /** the viewport's height, and the filter band's bottom edge in it */
  height: number;
  edge: number;
  /** scroll events are still coming */
  scrolling: boolean;
};

/**
 * Each page as it should be drawn this frame, the same object when nothing changes. A page more than SCREENS
 * screens from view gives its tiles up for boxes of their heights (`giveUp(j)` measures page `j`'s, or is undefined
 * when it must stay: focus is inside it); one that comes back near takes them back, a few a frame when it comes
 * from below the screen. A page not read yet near view turns to silhouettes.
 */
export function framePages<T>(
  pages: Page<T>[],
  f: Frame,
  giveUp: (j: number) => number[] | undefined,
): Page<T>[] {
  return pages.map((q, j) => {
    const far = isFar(f.tops[j], f.bottoms[j], f.height);
    if (!q.tiles.length) {
      // its silhouettes fill exactly the cells of its boxes (result-pages.ts boxParts), so the change moves
      // nothing below it, above the page in view too. One starting above the reader (another height above the
      // tile in view) waits for scrolling to stop.
      const near = !far;
      if (near === !!q.near || (f.tops[j] < f.edge && f.scrolling)) return q;
      return { ...q, near };
    }
    if (far === !!q.heights) return q;
    // tiles back from below the screen come a few a frame; above the reader, all at once, kept in place
    if (!far)
      return { ...q, heights: undefined, gradual: f.tops[j] > f.height };
    const heights = giveUp(j);
    return heights ? { ...q, heights, gradual: true } : q;
  });
}

/** The page to read: of those not read, not failed and near in `next`, the page in view, else the nearest one. */
export function pageToRead<T>(
  pages: Page<T>[],
  next: Page<T>[],
  inView: number,
): number | undefined {
  return pages
    .map((_, j) => j)
    .filter((j) => !pages[j].tiles.length && !pages[j].failed && next[j].near)
    .sort((a, b) => Math.abs(a - inView) - Math.abs(b - inView))[0];
}

/**
 * A return's next step once the restored tile was scrolled to: `offset` is how far it still stands from its place
 * (undefined when it is not drawn yet). Short of it, the rest of the last page or the next page is read while
 * there is one ("read"); else the return ends there ("done").
 */
export function restoreStep<T>(
  offset: number | undefined,
  last: Page<T>,
  total: number,
): "done" | "read" {
  if (offset !== undefined && Math.abs(offset) < 1) return "done";
  if (last.partial || (offset !== undefined && last.n < total)) return "read";
  return "done";
}
