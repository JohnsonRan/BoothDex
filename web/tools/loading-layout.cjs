// Loading-layout check (modelled on doona apps/lite/scripts/loading-layout.cjs): with the search held back by the
// slow-API proxy, the loading frame must have the final frame's geometry. At 1440x900 and 390x844 it records
// the boxes of the header, count, search, avatar strip, controls and the first screen of tiles with their
// parts (lab.cjs GEOMETRY) while loading and after the results arrive; every box must stay within 2 px and tiles
// per row equal. Then it scrolls on into page 2, read as slowly, and no layout shift may be recorded throughout. A control
// that first appears in a place the loading frame already holds (the fold trigger at hydration) is no failure by
// itself: it moves no other box and shifts nothing.
//   node tools/loading-layout.cjs <page url> <proxy url> [delay ms=2500]
const {
  launch,
  lab,
  RECORDER,
  GEOMETRY,
  compare,
  scrollToPage,
} = require("./lab.cjs");

const [
  url = "http://127.0.0.1:3203/",
  proxy = "http://127.0.0.1:3298",
  delay = "2500",
] = process.argv.slice(2);
const SIZES = [
  [1440, 900],
  [390, 844],
];

const settle = (p) =>
  p.waitForFunction(
    () =>
      document
        .getAnimations()
        .every(
          (a) =>
            a.playState !== "running" ||
            a.effect?.getTiming().iterations === Infinity,
        ),
    null,
    { timeout: 5000 },
  );

(async () => {
  const browser = await launch();
  let failed = false;
  const report = (name, result, extra = "") => {
    failed ||= result.bad.length > 0;
    console.log(
      `${name}: worst ${result.worst.toFixed(2)} px, ${result.bad.length ? `FAIL ${result.bad.slice(0, 8).join("; ")}` : "ok"}${extra}`,
    );
  };
  await lab(proxy, { delay: 0, fail: 0 });
  await fetch(url); // warms the server's avatar and category caches
  for (const [width, height] of SIZES) {
    const p = await browser.newPage({ viewport: { width, height } });
    await p.addInitScript(RECORDER);
    await lab(proxy, { delay });
    await p.goto(url, { waitUntil: "commit" });
    await p.waitForSelector(".astryx-grid", { state: "attached" });
    await p.waitForTimeout(300);
    const loading = await p.evaluate(GEOMETRY);
    await p.waitForFunction(
      () => document.querySelector(".astryx-grid img"),
      null,
      { timeout: Number(delay) + 15000 },
    );
    await settle(p);
    await p.waitForTimeout(500);
    const ready = await p.evaluate(GEOMETRY);
    const tiles = Object.keys(ready).filter((k) => /^tile\d+$/.test(k)).length;
    if (!loading.placeholder) {
      failed = true;
      console.log(`${width}x${height}: FAIL no loading silhouette seen`);
    }
    report(
      `${width}x${height} first load`,
      compare(ready, loading),
      `; ${tiles} tiles, ${ready.perRow} per row (loading ${loading.perRow})`,
    );

    // the next page, read while the reader scrolls: silhouettes hold the rows it arrives in
    await lab(proxy, { delay });
    await scrollToPage(p, 2);
    console.log(`${width}x${height} scrolled to ${new URL(p.url()).search}`);

    const shifts = await p.evaluate(() => window.__shifts);
    failed ||= shifts.length > 0;
    console.log(
      `${width}x${height} layout shifts: ${shifts.length}${shifts.length ? ` ${JSON.stringify(shifts.slice(0, 5))}` : ""}`,
    );
    await p.close();
  }
  await lab(proxy, { delay: 0 });
  await browser.close();
  console.log(failed ? "loading-layout: FAIL" : "loading-layout: ok");
  process.exit(failed ? 1 : 0);
})();
