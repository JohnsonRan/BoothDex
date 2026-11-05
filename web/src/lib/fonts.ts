// Two web fonts. Figtree (the neutral theme's Latin face) is self-hosted by next/font in app/layout.tsx and
// preloaded. MiSans covers Chinese and Japanese in ~100 unicode-range slices; a page of Booth titles pulls ~40 of
// them (~850 KiB) at the highest priority, ahead of the item covers. Its stylesheet is therefore loaded late on a
// first visit (FontLoader): system fonts show briefly, then MiSans swaps in (font-display: swap). The stylesheet's
// address is remembered, and later visits load it from the cache before first paint (FONTS_EARLY).

/** localStorage key holding the MiSans stylesheet's address once it has loaded. */
export const FONTS_CSS = "fontsCss";

/** Inline head script: a returning visitor gets the (cached) stylesheet render-blocking, so text never swaps. */
export const FONTS_EARLY = `try{var f=localStorage.getItem("${FONTS_CSS}");if(f){var l=document.createElement("link");
l.rel="stylesheet";l.href=f;l.setAttribute("blocking","render");document.head.appendChild(l)}}catch(e){}`;

/**
 * Resolves once these images are on screen (or failed): loaded, decoded, then two frames later. A loaded image
 * still takes ~0.1-0.2 s to decode and paint; starting the fonts in that gap let them compete with the LCP paint.
 */
export const imagesPainted = async (imgs: HTMLImageElement[]) => {
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
  // the first frame paints the decoded images, the second starts after that paint
  await new Promise((done) =>
    requestAnimationFrame(() => requestAnimationFrame(done)),
  );
};
