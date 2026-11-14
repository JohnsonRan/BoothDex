// Search and navigation check: drives the catalogue as a visitor does and checks that the address bar and the
// screen agree at every step. At 1440x900:
//   1. a word typed into the search field and sent with Enter goes to `q`, and the count narrows;
//   2. a price filter goes to the address and narrows the count; taking it off restores both;
//   3. scrolling into results page 3 writes `page=3`;
//   4. a tile opens its item as `item=<id>`; Back closes it and leaves the same tile at the same place;
//   5. leaving for another page and coming Back returns to that tile at that place, as does a reload;
//   6. the page-3 address opened in a new tab starts with page 3's first result in its first row, in its column of
//      the whole result set from the first frame it is painted in until page 2 has arrived above it, records no
//      layout shift, and keeps its place through a resize to 800 px and back;
//   7. an `item=` link opens its dialog; closing it drops the parameter without leaving a history entry.
// A place is the first tile showing below the filter band; "the same place" is that tile's top within 2 px, in
// any column. At 1440 px that tile is page 2's last but one, leading the row page 3 starts in, so step 5 also needs
// page 2 read again on the return, not left as boxes (lib/result-pages.ts boxParts, lib/result-frame.ts framePages).
// The search sends `q`, so this needs LAB_AI_OFF=1 and an API with AI_BASE_URL at a closed port (lab.cjs).
//   node tools/nav.cjs [page url=http://127.0.0.1:3203/] [query=ドレス]
const { launch, scrollToPage } = require("./lab.cjs");

const [origin = "http://127.0.0.1:3203/", query = "ドレス"] =
  process.argv.slice(2);
if (process.env.LAB_AI_OFF !== "1")
  throw new Error(
    "the search sends q: set LAB_AI_OFF=1 with an API whose AI_BASE_URL is a closed port",
  );

const failures = [];
const check = (name, ok, detail = "") => {
  if (!ok) failures.push(name);
  console.log(`${ok ? "ok  " : "FAIL"} ${name}${detail ? `: ${detail}` : ""}`);
};
const param = (p, k) => new URL(p.url()).searchParams.get(k);

/** The readout's count as a number; 0 while it is a silhouette or missing. */
const count = (p) =>
  p.evaluate(() => {
    const el = document.querySelector("[data-readout]")?.firstElementChild;
    // "9,648 件", "198,702 items": the first number
    const n = (el?.textContent ?? "").match(/\d[\d,]*/)?.[0] ?? "0";
    return Number(n.replace(/,/g, ""));
  });

/** Waits up to 15 s for the count to show a value other than `before`, and returns it (0 if it never did). */
async function countChanges(p, before) {
  for (let i = 0; i < 75; i++) {
    const n = await count(p);
    if (n > 0 && n !== before) return n;
    await p.waitForTimeout(200);
  }
  return 0;
}

/**
 * The first tile, in reading order, that shows below the filter band's lower edge, and its top: the tile the
 * results keep in place across a return (use-result-frame.ts, firstInView). The band is the sticky element before
 * `[data-band-content]` (lib/deep-start.ts).
 */
const place = (p) =>
  p.evaluate(() => {
    const band = document.querySelector(
      "[data-band-content]",
    )?.previousElementSibling;
    const edge = band ? band.getBoundingClientRect().bottom : 0;
    const tile = [
      ...document.querySelectorAll(".astryx-grid [data-item]"),
    ].find((t) => t.getBoundingClientRect().bottom > edge);
    if (!tile) return null;
    return {
      id: tile.dataset.item,
      top: Math.round(tile.getBoundingClientRect().top),
    };
  });

const samePlace = (a, b) =>
  !!a && !!b && a.id === b.id && Math.abs(a.top - b.top) <= 2;
const show = (a) => (a ? `${a.id}@${a.top}` : "none");

/** Where tile `id` is now, as `place` reads it ({id, top: null} when it is not in the grid). */
const placeOf = (p, id) =>
  p.evaluate((id) => {
    const tile = document.querySelector(`.astryx-grid [data-item="${id}"]`);
    return {
      id,
      top: tile ? Math.round(tile.getBoundingClientRect().top) : null,
    };
  }, id);

/** Whether tile `a.id` is back at its top: the same row, in whichever column (see the header). */
const sameRow = (a, now) => now.top !== null && Math.abs(now.top - a.top) <= 2;

/** Waits for the grid to hold covers, then for the scroll restore and the motion to finish. */
async function ready(p) {
  await p.waitForFunction(
    () => document.querySelector(".astryx-grid [data-item] img"),
    null,
    { timeout: 20000 },
  );
  await p.waitForTimeout(800);
}

/** Retries `place` until two reads 200 ms apart agree: a restore can land a frame after the first paint. */
async function stablePlace(p) {
  let last = await place(p);
  for (let i = 0; i < 20; i++) {
    await p.waitForTimeout(200);
    const now = await place(p);
    if (samePlace(last, now)) return now;
    last = now;
  }
  return last;
}

(async () => {
  const browser = await launch();
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
  });
  const p = await context.newPage();
  await p.goto(origin, { waitUntil: "networkidle" });
  await ready(p);
  const all = await count(p);
  check("home page shows a count", all > 0, String(all));

  // 1. search
  // the plain field gives way to the suggestions' own input once their code loads (search-box.tsx), so the
  // keys go to whichever input has the focus
  await p.locator(".filter-search input").first().click();
  await p.keyboard.type(query);
  await p.keyboard.press("Enter");
  await p.waitForURL((u) => u.searchParams.get("q") === query, {
    timeout: 10000,
  });
  const found = await countChanges(p, all);
  check(
    "search writes q and narrows the count",
    found > 0 && found < all,
    `${all} -> ${found}`,
  );
  await ready(p);
  const searchUrl = p.url();

  // 2. filter on and off
  const radio = (value) =>
    p.locator(`.filter-rail input[type=radio][value="${value}"]`);
  await radio("free").check({ force: true });
  await p.waitForURL((u) => u.searchParams.get("free") === "1", {
    timeout: 10000,
  });
  const free = await countChanges(p, found);
  check(
    "a price filter writes free=1 and narrows the count",
    free > 0 && free < found,
    `${found} -> ${free}`,
  );
  await radio("any").check({ force: true });
  await p.waitForURL((u) => !u.searchParams.has("free"), { timeout: 10000 });
  const back = await countChanges(p, free);
  check(
    "taking the filter off restores the address and the count",
    p.url() === searchUrl && back === found,
    `${p.url() === searchUrl ? "same address" : p.url()}, ${back}`,
  );
  await ready(p);

  // 3. scroll into page 3
  await scrollToPage(p, 3);
  await p.waitForTimeout(600);
  check("scrolling writes page=3", param(p, "page") === "3", param(p, "page"));
  const deep = await stablePlace(p);
  const deepUrl = p.url();

  // 4. item dialog and Back
  const before = await p.evaluate(() => history.length);
  // the tile under the middle of the window: the first one in view may sit half under the band
  const picked = await p.evaluate(() => {
    const r = document.querySelector(".astryx-grid").getBoundingClientRect();
    return document
      .elementFromPoint(r.x + 40, innerHeight / 2)
      ?.closest("[data-item]")?.dataset.item;
  });
  await p.locator(`[data-item="${picked}"]`).first().click();
  await p.waitForFunction(() => document.querySelector("dialog[open]"));
  check(
    "a tile opens its item as item=<id>",
    param(p, "item") === picked,
    `${picked} -> ${param(p, "item")}`,
  );
  await p.goBack();
  await p.waitForFunction(() => !document.querySelector("dialog[open]"));
  await stablePlace(p);
  const afterItem = await placeOf(p, deep.id);
  check(
    "Back closes the item at the same place",
    !param(p, "item") &&
      p.url() === deepUrl &&
      sameRow(deep, afterItem) &&
      (await p.evaluate(() => history.length)) === before + 1,
    `${show(deep)} -> ${show(afterItem)}`,
  );

  // 5. leave and come back; reload
  await p.goto(new URL("/privacy", origin).href, { waitUntil: "load" });
  await p.goBack({ waitUntil: "load" });
  await ready(p);
  await stablePlace(p);
  const returned = await placeOf(p, deep.id);
  check(
    "Back from another page returns to the same place",
    p.url() === deepUrl && sameRow(deep, returned),
    `${show(deep)} -> ${show(returned)}, page=${param(p, "page")}`,
  );
  await p.reload({ waitUntil: "load" });
  await ready(p);
  await stablePlace(p);
  const reloaded = await placeOf(p, deep.id);
  check(
    "a reload returns to the same place",
    param(p, "page") === "3" && sameRow(deep, reloaded),
    `${show(deep)} -> ${show(reloaded)}`,
  );

  // 6. the deep address in a new tab starts at page 3's first result (100 a page: server-data.ts PAGE_SIZE)
  const api = new URL("/api/search", origin);
  api.search = `q=${encodeURIComponent(query)}&sort=relevant&page=3&pageSize=100&fields=card`;
  const third = (await (await fetch(api)).json()).items?.[0]?.id;
  api.searchParams.set("page", "2");
  const second = (await (await fetch(api)).json()).items?.at(-1)?.id;
  const tab = await context.newPage();
  // the x of page 3's first result in the first frame it is painted in, and the layout shifts from the first paint
  await tab.addInitScript((id) => {
    window.__shift = 0;
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) window.__shift += e.value;
    }).observe({ type: "layout-shift", buffered: true });
    const seen = () => {
      const r = document
        .querySelector(`[data-item="${id}"]`)
        ?.getBoundingClientRect();
      if (r?.width) window.__firstX = r.x;
      else requestAnimationFrame(seen);
    };
    requestAnimationFrame(seen);
  }, String(third));
  await tab.goto(deepUrl, { waitUntil: "load" });
  await ready(tab);
  const fresh = await stablePlace(tab);
  const start = await placeOf(tab, String(third));
  check(
    "the page-3 address opens with page 3's first result in the first row in view",
    param(tab, "page") === "3" && !!fresh && sameRow(fresh, start),
    `${show(start)}, first in view ${show(fresh)}`,
  );
  // page 2, read above it, leaves it where it was first painted: in its column of the whole set
  await tab.waitForSelector(`[data-item="${second}"]`, { timeout: 15000 });
  const column = await tab.evaluate((id) => {
    const tile = document.querySelector(`[data-item="${id}"]`);
    const grid = tile.parentElement;
    const tracks = getComputedStyle(grid)
      .gridTemplateColumns.split(" ")
      .filter(parseFloat);
    const x = tile.getBoundingClientRect().x;
    // a column's pitch is the grid's width over its columns, near enough with a small gap
    const pitch = grid.clientWidth / tracks.length;
    return {
      cols: tracks.length,
      col: Math.round((x - grid.getBoundingClientRect().x) / pitch) + 1,
      first: Math.round(window.__firstX),
      now: Math.round(x),
    };
  }, String(third));
  check(
    "page 3's first result stays in its column from its first paint",
    column.first === column.now && column.col === (200 % column.cols) + 1,
    `column ${column.col} of ${column.cols}, x ${column.first} -> ${column.now}`,
  );
  // the pages put above it with the window scrolled by their height move nothing in view
  const shift = await tab.evaluate(() => window.__shift);
  check(
    "the page-3 start records no layout shift",
    shift < 0.01,
    shift.toFixed(3),
  );
  // a new width keeps the first tile in view at its top, however the rows reflow
  for (const width of [800, 1440]) {
    const before = await stablePlace(tab);
    await tab.setViewportSize({ width, height: 900 });
    await tab.waitForTimeout(800);
    const after = before && (await placeOf(tab, before.id));
    check(
      `a resize to ${width} px keeps the place`,
      !!after && sameRow(before, after),
      `${show(before)} -> ${show(after)}`,
    );
  }
  await tab.close();

  // 7. a shared item link
  const shared = await context.newPage();
  await shared.goto(new URL(`/?item=${deep.id}`, origin).href, {
    waitUntil: "load",
  });
  await shared.waitForFunction(
    () => document.querySelector("dialog[open]"),
    null,
    {
      timeout: 15000,
    },
  );
  const entries = await shared.evaluate(() => history.length);
  await shared.keyboard.press("Escape");
  await shared.waitForFunction(() => !document.querySelector("dialog[open]"));
  check(
    "closing a shared item link drops item= without a history entry",
    !param(shared, "item") &&
      (await shared.evaluate(() => history.length)) === entries,
    shared.url(),
  );

  await browser.close();
  console.log(failures.length ? `FAILED: ${failures.length}` : "all ok");
  process.exit(failures.length ? 1 : 0);
})().catch((e) => {
  console.error(e);
  // a failed step usually leaves the next one nothing to wait for: that is the failure, not a broken run
  process.exit(failures.length ? 1 : 2);
});
