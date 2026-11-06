// Shared parts of the browser checks (loading-layout.cjs, motion.cjs): Playwright, the slow-API proxy
// (slow-api.mjs) and the page geometry. Lab only: needs Playwright and a `next start` server whose run-time
// API_ORIGIN is the proxy. PLAYWRIGHT_CORE and CHROME point at a local Playwright and Chromium.
const { readdirSync } = require("node:fs");

const { chromium } = require(process.env.PLAYWRIGHT_CORE || "playwright-core");

function chrome() {
  if (process.env.CHROME) return process.env.CHROME;
  const dir = `${process.env.HOME}/.cache/ms-playwright`;
  const build = readdirSync(dir).find((d) => d.startsWith("chromium-"));
  const sub = readdirSync(`${dir}/${build}`).find((d) =>
    d.startsWith("chrome-linux"),
  );
  return `${dir}/${build}/${sub}/chrome`;
}

const launch = () =>
  chromium.launch({ executablePath: chrome(), args: ["--no-sandbox"] });

/** Sets the proxy's search delay (ms) and failure switch. */
async function lab(proxy, settings) {
  const q = new URLSearchParams(settings);
  await fetch(`${proxy}/__lab?${q}`);
}

/** Records layout shifts and every Element.animate call from the first script on. */
const RECORDER = () => {
  window.__shifts = [];
  window.__anims = [];
  new PerformanceObserver((list) => {
    for (const e of list.getEntries())
      window.__shifts.push({
        value: e.value,
        at: Math.round(e.startTime),
        input: e.hadRecentInput,
        nodes: e.sources.map(
          (s) =>
            `${s.node?.nodeName ?? "?"} "${(s.node?.textContent ?? "").slice(0, 12)}" ${Math.round(s.previousRect.x)},${Math.round(s.previousRect.y)} ${Math.round(s.previousRect.width)}w -> ${Math.round(s.currentRect.x)},${Math.round(s.currentRect.y)} ${Math.round(s.currentRect.width)}w`,
        ),
      });
  }).observe({ type: "layout-shift", buffered: true });
  // the refresh progress bar's state changes (catalog/refresh.tsx data-refresh), with their times
  window.__refresh = [];
  addEventListener("DOMContentLoaded", () =>
    new MutationObserver((list) => {
      for (const m of list)
        window.__refresh.push([
          performance.now(),
          m.target.getAttribute("data-refresh"),
        ]);
    }).observe(document.body, {
      subtree: true,
      attributeFilter: ["data-refresh"],
    }),
  );
  const animate = Element.prototype.animate;
  Element.prototype.animate = function (keyframes, options) {
    window.__anims.push({
      id: options?.id ?? "",
      tag: this.tagName,
      at: performance.now(),
    });
    return animate.call(this, keyframes, options);
  };
};

/**
 * Boxes (document coordinates) of the frame and of the tiles on the first screen: the header, the title row,
 * the count (in the filter row's readout), the search field, the avatar strip, every control button above the
 * grid (by name), and per tile its cover, title, shop, price and badge row.
 */
const GEOMETRY = () => {
  const box = (el) => {
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return { x: r.x, y: r.y + scrollY, w: r.width, h: r.height };
  };
  const main = document.querySelector("main") ?? document.body;
  const h1 = main.querySelector("h1");
  const grid = main.querySelector(".astryx-grid");
  const out = {
    // the top bar (logo and end actions), then the page header
    topBar: box(document.querySelector("nav")),
    header: box(h1?.parentElement?.parentElement),
    titleRow: box(h1?.parentElement),
    title: box(h1),
  };
  // streaming leaves <template> markers beside a pending boundary's fallback
  const elements = (el) =>
    [...(el?.children ?? [])].filter((c) => c.tagName !== "TEMPLATE");
  out.count = box(elements(main.querySelector("[data-readout]"))[0]);
  out.search = box(main.querySelector("input"));
  out.avatars = box(main.querySelector('[aria-roledescription="carousel"]'));
  const top = grid ? grid.getBoundingClientRect().top : Infinity;
  // keyed by the button's name (digits folded, so "+2" and "+3" are one), not by its place in the row: the "+N" fold
  // trigger appears at hydration, which would renumber every control after it
  const seen = {};
  [...main.querySelectorAll("button")]
    .filter(
      (b) =>
        b.getBoundingClientRect().bottom < top &&
        b.checkVisibility() &&
        !b.closest('[aria-roledescription="carousel"]'),
    )
    .forEach((b) => {
      const name = (b.ariaLabel || b.textContent || "")
        .trim()
        .replace(/\d+/g, "#");
      seen[name] = (seen[name] ?? 0) + 1;
      out[`control ${name} ${seen[name]}`] = box(b);
    });
  out.grid = box(grid);
  const tiles = grid
    ? elements(grid).filter((t) => t.getBoundingClientRect().top < innerHeight)
    : [];
  tiles.forEach((t, i) => {
    out[`tile${i}`] = box(t);
    out[`tile${i}.cover`] = box(t.children[0]);
    const text = t.children[1]?.children ?? [];
    // title, shop row (icon and name), price row (price and favourite button), badge row
    ["title", "shop", "price", "badges"].forEach((part, n) => {
      out[`tile${i}.${part}`] = box(text[n]);
    });
  });
  const firstTop = tiles[0]?.getBoundingClientRect().top;
  out.perRow = tiles.filter(
    (t) => t.getBoundingClientRect().top === firstTop,
  ).length;
  out.placeholder = !!main.querySelector("[data-placeholder]");
  out.images = grid ? grid.querySelectorAll("img").length : 0;
  return out;
};

/** Differences above `tolerance` px between two GEOMETRY results; the count's width is its text's. */
function compare(a, b, tolerance = 2) {
  const bad = [];
  let worst = 0;
  for (const [k, va] of Object.entries(a)) {
    if (!va || typeof va !== "object") continue;
    const vb = b[k];
    if (!vb) {
      // a control with no counterpart appeared inside a place the loading frame already holds (the fold trigger at
      // hydration): the real shift is judged by the layout-shift entries and by every box that does have one
      if (
        (!k.startsWith("tile") || k.includes(".")) &&
        !k.startsWith("control ")
      )
        bad.push(`${k}: missing`);
      continue;
    }
    for (const axis of ["x", "y", "w", "h"]) {
      // the count's width is its text's; the grid grows with the results below the first screen
      if ((k === "count" && axis === "w") || (k === "grid" && axis === "h"))
        continue;
      const d = Math.abs(va[axis] - vb[axis]);
      worst = Math.max(worst, d);
      if (d > tolerance)
        bad.push(
          `${k}.${axis} ${va[axis].toFixed(1)} -> ${vb[axis].toFixed(1)}`,
        );
    }
  }
  if (a.perRow !== b.perRow) bad.push(`perRow ${a.perRow} -> ${b.perRow}`);
  return { worst, bad };
}

/**
 * Scrolls down by wheel, as a visitor would, until the pagination's "next page" button is on screen, and
 * returns it. A jump straight to the bottom would pass tiles 25-100 before they are drawn (later-tiles.tsx).
 */
async function nextPageButton(p) {
  const next = p.locator("nav.astryx-pagination button").last();
  for (let i = 0; i < 80 && !(await next.isVisible()); i++) {
    await p.mouse.wheel(0, 500);
    await p.waitForTimeout(60);
  }
  for (let i = 0; i < 80; i++) {
    const box = await next.boundingBox();
    const { height } = p.viewportSize();
    if (box && box.y + box.height < height) break;
    await p.mouse.wheel(0, 500);
    await p.waitForTimeout(60);
  }
  return next;
}

module.exports = { launch, lab, RECORDER, GEOMETRY, compare, nextPageButton };
