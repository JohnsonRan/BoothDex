// The filter band's elements, attached by band.tsx, and the lines measured from them. A module of their own: the
// results (result-pages.tsx) and a filter change (filter-url.tsx) read them, and band.tsx itself depends on
// filter-url.tsx through the strip and the refresh line.

let band: HTMLElement | null = null;
let bar: HTMLElement | null = null;

/** the band's root (its sticky top and strip geometry) */
export function attachBand(el: HTMLElement | null) {
  band = el;
}

/** the toolbar row, stuck under the top bar */
export function attachBar(el: HTMLElement | null) {
  bar = el;
}

/**
 * The viewport px above the line the band's bottom edge holds once stuck with its strip shrunk (opened by hand or
 * not, so a place saved and the same place restored agree): content above it is under the top bar or the band.
 * Scrolling does not change it, so a place measured from it is found again at the same scroll. deep-start.ts
 * reads the same line before the page hydrates.
 */
export function covered() {
  if (!band || !bar) return 0;
  const head = getComputedStyle(band);
  return (
    Number.parseFloat(getComputedStyle(bar).top) -
    (Number.parseFloat(head.getPropertyValue("--strip-d")) || 0) +
    band.getBoundingClientRect().height
  );
}

/**
 * Where a filter change scrolls the window (filter-url.tsx), as the new results go in from their start: the page's
 * top while the avatar strip is at full size; with it shrunk (opened by hand or not) where it stays shrunk, the
 * results at their start under the band; part-way, where the window is. Without a strip, the top.
 */
export function changeTop() {
  const at = band?.dataset.shrink;
  if (!band || !at || at === "none") return 0;
  const end =
    (Number.parseFloat(band.style.getPropertyValue("--strip-y0")) || 0) +
    (Number.parseFloat(getComputedStyle(band).getPropertyValue("--strip-d")) ||
      0);
  return Math.min(scrollY, Math.ceil(end));
}
