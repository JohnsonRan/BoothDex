// Two web fonts. Figtree (the neutral theme's Latin face) is self-hosted by next/font in app/layout.tsx and
// preloaded. MiSans covers Chinese and Japanese in ~100 unicode-range slices; a page of Booth titles pulls ~40 of
// them (~850 KiB) at the highest priority, ahead of the item covers. Its stylesheet is therefore loaded late on a
// first visit (FontLoader), once the first cover is on screen: system fonts show briefly, then MiSans swaps in
// (font-display: swap). The stylesheet's
// address is remembered, and later visits load it from the cache before first paint (FONTS_EARLY).

/** localStorage key holding the MiSans stylesheet's address once it has loaded. */
export const FONTS_CSS = "fontsCss";

/** Inline head script: a returning visitor gets the (cached) stylesheet render-blocking, so text never swaps. */
export const FONTS_EARLY = `try{var f=localStorage.getItem("${FONTS_CSS}");if(f){var l=document.createElement("link");
l.rel="stylesheet";l.href=f;l.setAttribute("blocking","render");document.head.appendChild(l)}}catch(e){}`;

/** Resolves once these images are loaded, decoded and two frames have passed (or they failed). */
const imagesPainted = async (imgs: HTMLImageElement[]) => {
  await Promise.all(
    imgs.map((img) =>
      (img.complete
        ? Promise.resolve()
        : new Promise((done) => {
            img.addEventListener("load", done, { once: true });
            img.addEventListener("error", done, { once: true });
          })
      )
        // decode() rejects for broken images; those count as settled
        .then(() => img.decode())
        .catch(() => {}),
    ),
  );
  await new Promise((done) =>
    requestAnimationFrame(() => requestAnimationFrame(done)),
  );
};

/**
 * Resolves once the browser has reported a cover as the largest contentful paint. Two frames after decoding were
 * not enough: while hydration kept the main thread busy, the cover reached the screen ~50 ms after the fonts were
 * requested, and Lighthouse then counted all ~1 MB of slices towards LCP (10.4 s instead of 4.5 s, 5 of 10 runs).
 * Where LCP is not reported (Safari), or no cover becomes the largest paint (empty results), it settles 3 s after
 * the images are painted.
 */
export const coverPainted = (imgs: HTMLImageElement[]) =>
  new Promise<void>((done) => {
    const fallback = () =>
      imagesPainted(imgs).then(() => setTimeout(done, 3000));
    if (
      !PerformanceObserver.supportedEntryTypes?.includes(
        "largest-contentful-paint",
      )
    )
      return void fallback();
    const seen = new PerformanceObserver((list) => {
      const entries = list.getEntries() as LargestContentfulPaint[];
      if (entries.some((e) => e.element?.tagName === "IMG")) {
        seen.disconnect();
        done();
      }
    });
    seen.observe({ type: "largest-contentful-paint", buffered: true });
    fallback();
  });
