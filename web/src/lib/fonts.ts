// Two web fonts. Figtree (the neutral theme's Latin face) is self-hosted by next/font in app/layout.tsx and
// preloaded. MiSans covers Chinese and Japanese in ~100 unicode-range slices; a page of Booth titles pulls ~40 of
// them (~850 KiB) at the highest priority, ahead of the item covers. Its stylesheet is therefore loaded late on a
// first visit (FontLoader), once the first cover is on screen: system fonts show briefly, then MiSans swaps in
// (font-display: swap). The stylesheet's
// address is remembered, and later visits load it from the cache before first paint (FONTS_EARLY).

/** localStorage key holding the MiSans stylesheet's address once it has loaded. */
export const FONTS_CSS = "fontsCss";
/** localStorage key holding the MiSans slices this browser has drawn (newest last). */
export const FONTS_USED = "fontsUsed";
/** Slices remembered at most; a page of titles draws ~40. */
export const FONTS_USED_MAX = 60;

// Where MiSans' files are served: Next emits the imported stylesheet as a CSS chunk and its slices as media.
// Only same-origin addresses with these paths are written to or read back from localStorage, so a value
// planted there cannot load a stylesheet or a font from elsewhere.
const SHEET_PATH = String.raw`^/_next/static/chunks/[\w.-]+\.css$`;
const SLICE_PATH = String.raw`^/_next/static/media/MiSans-VF\.\d+\.[\w.-]+\.woff2$`;

const sameOrigin = (href: unknown, path: string): href is string => {
  if (typeof href !== "string") return false;
  try {
    const u = new URL(href);
    return u.origin === location.origin && new RegExp(path).test(u.pathname);
  } catch {
    return false;
  }
};

/** The MiSans stylesheet's address, if `href` may be remembered as one. */
export const isFontSheet = (href: unknown): href is string =>
  sameOrigin(href, SHEET_PATH);

/** A MiSans slice's address, if `href` may be remembered as one. */
export const isFontSlice = (href: unknown): href is string =>
  sameOrigin(href, SLICE_PATH);

/** The remembered slices that are still valid addresses (stored JSON is not trusted). */
export const fontSlices = (stored: string | null): string[] => {
  try {
    const v: unknown = JSON.parse(stored ?? "[]");
    return Array.isArray(v) ? v.filter(isFontSlice) : [];
  } catch {
    return [];
  }
};

/**
 * Inline head script: a returning visitor gets the (cached) stylesheet render-blocking and the slices it drew
 * before preloaded from the cache, so they are ready at first paint and text does not swap as the page shows.
 * It applies the same checks as isFontSheet and fontSlices.
 */
export const FONTS_EARLY = `try{var o=function(h,r){try{var u=typeof h=="string"&&new URL(h);return!!u&&u.origin===location.origin&&new RegExp(r).test(u.pathname)}catch(e){return!1}},
f=localStorage.getItem("${FONTS_CSS}");if(f&&o(f,${JSON.stringify(SHEET_PATH)})){var h=document.head,
u=JSON.parse(localStorage.getItem("${FONTS_USED}")||"[]");if(Array.isArray(u))for(var i=0;i<u.length;i++)if(o(u[i],${JSON.stringify(SLICE_PATH)})){
var p=document.createElement("link");p.rel="preload";p.as="font";p.type="font/woff2";p.crossOrigin="";p.href=u[i];h.appendChild(p)}
var l=document.createElement("link");l.rel="stylesheet";l.href=f;l.setAttribute("blocking","render");h.appendChild(l)}}catch(e){}`;

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
    let seen: PerformanceObserver | undefined;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let settled = false;
    // whichever comes first ends both: the observer and the fallback timer
    const finish = () => {
      if (settled) return;
      settled = true;
      seen?.disconnect();
      clearTimeout(timer);
      done();
    };
    if (
      PerformanceObserver.supportedEntryTypes?.includes(
        "largest-contentful-paint",
      )
    ) {
      seen = new PerformanceObserver((list) => {
        const entries = list.getEntries() as LargestContentfulPaint[];
        if (entries.some((e) => e.element?.tagName === "IMG")) finish();
      });
      seen.observe({ type: "largest-contentful-paint", buffered: true });
    }
    imagesPainted(imgs).then(() => {
      if (!settled) timer = setTimeout(finish, 3000);
    });
  });
