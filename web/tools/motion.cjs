// Motion check (modelled on doona apps/lite/scripts/motion.cjs): a first screen that comes with the first paint
// never animates; with the search held back by the slow-API proxy the arriving results enter once; a filter
// change, scrolling into the next page and Back from an item never replay it; under reduced motion nothing runs at all. Refresh timing
// (lib/use-delayed): a refresh answered in under 150 ms shows no indicator and fades nothing in; a slow one shows
// the indicator from 150 ms, keeps it at least 300 ms, and fades the new results in once.
//   node tools/motion.cjs <page url> <proxy url> [delay ms=2500]
const { launch, lab, RECORDER, scrollToPage, streams } = require("./lab.cjs");

const [
  url = "http://127.0.0.1:3203/",
  proxy = "http://127.0.0.1:3298",
  delay = "2500",
] = process.argv.slice(2);
const ENTRANCE = ["page-enter", "row-enter"];
// A filter change as a visitor makes it: the toolbar's sort menu, switched between Trending and Newest. Opening the
// menu is separate from picking the item, so a timed refresh starts at the pick. A text search's sort menu has
// neither, so these steps need a URL without `q`.
const openSort = async (p) => {
  const button = p.getByRole("button", { name: /^(Trending|Newest)$/ });
  const next =
    (await button.textContent()).trim() === "Trending" ? "Newest" : "Trending";
  await button.click();
  const item = p.getByRole("menuitemradio", { name: next });
  await item.waitFor();
  return () => item.click();
};
const changeSort = async (p) => (await openSort(p))();

(async () => {
  const browser = await launch();
  let failed = false;
  const check = (ok, text) => {
    failed ||= !ok;
    console.log(`${ok ? "ok  " : "FAIL"} ${text}`);
  };
  const entrances = (p) =>
    p.evaluate(
      (ids) => window.__anims.filter((a) => ids.includes(a.id)),
      ENTRANCE,
    );
  const arrive = (p) =>
    // streamed content waits in a hidden container until React reveals it, so only a visible cover counts
    p.waitForFunction(
      () =>
        [...document.querySelectorAll(".astryx-grid img")].some((i) =>
          i.checkVisibility(),
        ),
      null,
      {
        timeout: Number(delay) + 15000,
      },
    );
  const refreshed = async (p) => {
    await p
      .waitForSelector('[data-refresh="shown"]', { timeout: 3000 })
      .catch(() => {});
    await p.waitForSelector('[data-refresh="idle"]', {
      state: "attached",
      timeout: Number(delay) + 15000,
    });
    await p.waitForTimeout(800);
  };
  // changes the sort and reports the indicator's intervals (ms after the pick) and fades
  const timedRefresh = async (p) => {
    const pick = await openSort(p);
    await p.evaluate(() => {
      window.__refresh = [];
      window.__anims = window.__anims.filter((a) => a.id !== "swap-in");
    });
    const before = p.url();
    // the frame in which the URL changes: the new results commit with it
    await p.evaluate(() => {
      const from = location.href;
      window.__navAt = 0;
      // Playwright waits for the menu item to stop moving before it clicks, so the pick is timed in the page
      addEventListener(
        "click",
        () => {
          window.__clickAt = performance.now();
        },
        { capture: true, once: true },
      );
      const look = () => {
        if (location.href !== from) window.__navAt = performance.now();
        else requestAnimationFrame(look);
      };
      requestAnimationFrame(look);
    });
    await pick();
    await p.waitForURL((u) => u.toString() !== before, { timeout: 20000 });
    await p.waitForSelector('[data-refresh="idle"]', {
      state: "attached",
      timeout: 20000,
    });
    await p.waitForTimeout(600);
    return p.evaluate(() => {
      const t0 = window.__clickAt;
      const shown = [];
      for (const [t, v] of window.__refresh)
        if (v === "shown") shown.push([Math.round(t - t0), null]);
        else if (shown.length && shown.at(-1)[1] === null)
          shown.at(-1)[1] = Math.round(t - t0);
      return {
        navMs: Math.round(window.__navAt - t0),
        shown,
        fades: window.__anims.filter((a) => a.id === "swap-in").length,
      };
    });
  };
  await lab(proxy, { delay: 0, fail: 0, cache: 0 });
  await fetch(url);

  for (const reduced of [false, true]) {
    const ctx = await browser.newContext({
      viewport: { width: 1440, height: 900 },
      // the check names buttons and messages by their English text
      locale: "en-US",
      reducedMotion: reduced ? "reduce" : "no-preference",
    });
    const p = await ctx.newPage();
    await p.addInitScript(RECORDER);
    const tag = reduced ? "reduced" : "motion";

    await lab(proxy, { delay: 0 });
    await p.goto(url, { waitUntil: "load" });
    await p.waitForTimeout(1500);
    check(
      (await entrances(p)).length === 0,
      `${tag}: fast first load animates nothing (${(await entrances(p)).length} entrance animations)`,
    );

    await lab(proxy, { delay });
    await p.goto(url, { waitUntil: "commit" });
    await arrive(p);
    await p.waitForTimeout(100);
    const running = await p.evaluate(
      (ids) =>
        document
          .getAnimations()
          .filter((a) => ids.includes(a.id) && a.playState === "running")
          .length,
      ENTRANCE,
    );
    const first = await entrances(p);
    const rows = first.filter((a) => a.id === "row-enter").length;
    const sections = first.length - rows;
    const anyRunning = await p.evaluate(() =>
      document
        .getAnimations()
        .filter((a) => a.playState === "running")
        .map((a) => a.animationName ?? a.id),
    );
    if (reduced)
      check(
        first.length === 0 && anyRunning.length === 0,
        `${tag}: slow load runs nothing (${first.length} entrances, running ${JSON.stringify(anyRunning.slice(0, 5))})`,
      );
    else if (streams(url))
      check(
        running > 0 && rows > 0 && sections > 0,
        `${tag}: slow load enters once: ${sections} sections, ${rows} rows, ${running} running after arrival`,
      );
    // without a text search the results come with the frame's first paint: nothing was seen to enter
    else
      check(
        first.length === 0,
        `${tag}: slow load arrives whole, nothing enters (${first.length} entrances)`,
      );
    await p.waitForTimeout(1200);

    if (!reduced) {
      // a quick refresh: the search for the other sort is asked once, then answered from the proxy's memory
      await lab(proxy, { delay: 0, cache: 1 });
      await timedRefresh(p); // to Newest
      await timedRefresh(p); // back to Trending (filters replace the URL, so not Back)
      const quick = await timedRefresh(p); // to Newest again, answered from memory
      await lab(proxy, { cache: 0 });
      // under 150 ms nothing may show; a slower answer may show the indicator, but only from 150 ms and only
      // while it is still awaited (never first drawn as the results arrive)
      const [[qFrom] = []] = quick.shown;
      check(
        quick.navMs < 150
          ? quick.shown.length === 0 && quick.fades === 0
          : quick.shown.length === 0 || (qFrom >= 150 && qFrom < quick.navMs),
        `${tag}: quick refresh (${quick.navMs} ms): indicator ${JSON.stringify(quick.shown)}, ${quick.fades} fades`,
      );
      await lab(proxy, { delay: 1200 });
      const slow = await timedRefresh(p);
      const [[from, to] = []] = slow.shown;
      check(
        slow.shown.length === 1 &&
          from >= 150 &&
          to - from >= 300 &&
          slow.fades >= 1,
        `${tag}: slow refresh: indicator ${from}-${to} ms, ${slow.fades} fades`,
      );
    }

    await lab(proxy, { delay: 1200 });
    await changeSort(p);
    await refreshed(p);
    check(
      (await entrances(p)).length === first.length,
      `${tag}: filter change does not replay (${new URL(p.url()).search})`,
    );

    await scrollToPage(p, 2);
    check(
      (await entrances(p)).length === first.length,
      `${tag}: the next page does not replay (${new URL(p.url()).search})`,
    );

    // a tile in view below the sticky filter band, clicked where a visitor would: its cover
    const cover = await p.evaluate(() => {
      const band = document
        .querySelector(".filter-toolbar")
        .getBoundingClientRect().bottom;
      const box = [...document.querySelectorAll("[data-item]")]
        .map((t) => t.getBoundingClientRect())
        .find((r) => r.top > band && r.bottom < innerHeight);
      // the cover is the tile's top square: half the tile's width down is its middle
      return { x: box.x + box.width / 2, y: box.y + box.width / 2 };
    });
    await p.mouse.click(cover.x, cover.y);
    await p.waitForURL(/item=/);
    await p.goBack();
    await p.waitForTimeout(300);
    check(
      (await entrances(p)).length === first.length,
      `${tag}: Back from an item does not replay`,
    );

    if (!reduced) {
      // A failed refresh keeps the results and offers a retry. Kept means the same grid: results mounted anew read
      // their page again, which fails too and puts up a second Banner and retry at the grid's end.
      await p.evaluate(() => {
        window.__grid = document.querySelector(".astryx-grid");
      });
      await lab(proxy, { delay: 0, fail: 1 });
      await changeSort(p);
      const banner = p.getByText("Couldn't load items");
      await banner
        .first()
        .waitFor({ timeout: 10000 })
        .catch(() => {});
      const kept = await p.locator(".astryx-grid img").count();
      const banners = await banner.count();
      const same = await p.evaluate(() => window.__grid.isConnected);
      check(
        banners === 1 && same && kept > 0,
        `${tag}: failed refresh keeps the grid (${same ? "same" : "mounted anew"}) and ${kept} covers with ${banners} Banner(s)`,
      );
      await lab(proxy, { delay: 0, fail: 0 });
      // first: a second Banner already failed the check above
      await p.getByRole("button", { name: "Try again" }).first().click();
      await banner.waitFor({ state: "hidden", timeout: 10000 }).catch(() => {});
      check(
        !(await banner.isVisible()),
        `${tag}: retry clears the Banner (${new URL(p.url()).search})`,
      );
    }

    if (reduced) {
      const samples = [];
      for (let i = 0; i < 5; i++) {
        samples.push(
          await p.evaluate(() =>
            document
              .getAnimations()
              .filter((a) => a.playState === "running")
              .map((a) => a.animationName ?? a.id),
          ),
        );
        await p.waitForTimeout(200);
      }
      check(
        samples.every((s) => s.length === 0),
        `${tag}: zero running animations at rest (${JSON.stringify(samples.flat().slice(0, 5))})`,
      );
    }
    await ctx.close();
  }
  await lab(proxy, { delay: 0 });
  await browser.close();
  console.log(failed ? "motion: FAIL" : "motion: ok");
  process.exit(failed ? 1 : 0);
})();
