// Motion check (modelled on doona apps/lite/scripts/motion.cjs): a first screen that comes with the first paint
// never animates; with the search held back by the slow-API proxy the arriving results enter once; a filter
// change, a page change and Back never replay it; under reduced motion nothing runs at all.
//   node tools/motion.cjs <page url> <proxy url> [delay ms=2500]
const { launch, lab, RECORDER, nextPageButton } = require("./lab.cjs");

const [
  url = "http://127.0.0.1:3203/",
  proxy = "http://127.0.0.1:3298",
  delay = "2500",
] = process.argv.slice(2);
const ENTRANCE = ["page-enter", "row-enter"];

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
      .waitForSelector('[role="progressbar"]:visible', { timeout: 3000 })
      .catch(() => {});
    await p.waitForSelector('[role="progressbar"]', {
      state: "hidden",
      timeout: Number(delay) + 15000,
    });
    await p.waitForTimeout(800);
  };
  await lab(proxy, { delay: 0, fail: 0 });
  await fetch(url);

  for (const reduced of [false, true]) {
    const ctx = await browser.newContext({
      viewport: { width: 1440, height: 900 },
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
    else
      check(
        running > 0 && rows > 0 && sections > 0,
        `${tag}: slow load enters once: ${sections} sections, ${rows} rows, ${running} running after arrival`,
      );
    await p.waitForTimeout(1200);

    await lab(proxy, { delay: 1200 });
    // the second category tab (toggle buttons carry aria-pressed)
    await p.locator('button[aria-pressed="false"]:visible').first().click();
    await refreshed(p);
    check(
      (await entrances(p)).length === first.length,
      `${tag}: filter change does not replay (${new URL(p.url()).search})`,
    );

    await (await nextPageButton(p)).click();
    await refreshed(p);
    check(
      (await entrances(p)).length === first.length,
      `${tag}: page change does not replay (${new URL(p.url()).search})`,
    );

    await p.goBack();
    await refreshed(p);
    check(
      (await entrances(p)).length === first.length,
      `${tag}: Back does not replay`,
    );

    if (!reduced) {
      // a failed refresh keeps the results and offers a retry
      await lab(proxy, { delay: 0, fail: 1 });
      await p.locator('button[aria-pressed="false"]:visible').first().click();
      const banner = p.getByText("Couldn't load items");
      await banner.waitFor({ timeout: 10000 }).catch(() => {});
      const kept = await p.locator(".astryx-grid img").count();
      check(
        (await banner.isVisible()) && kept > 0,
        `${tag}: failed refresh keeps ${kept} covers with a Banner`,
      );
      await lab(proxy, { delay: 0, fail: 0 });
      await p.getByRole("button", { name: "Try again" }).click();
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
