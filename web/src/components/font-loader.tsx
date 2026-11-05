"use client";

import { useEffect } from "react";
import { FONTS_CSS, HOLD_FONTS } from "@/lib/fonts";

let started = false;

/** Starts loading MiSans and remembers its stylesheet for the next visit; later calls do nothing. */
export function loadFonts() {
  if (started) return;
  started = true;
  import("subsetted-fonts/MiSans-VF/MiSans-VF.css").then(() => {
    const sheet = [...document.styleSheets].find(
      (s) =>
        s.href &&
        [...rules(s)].some(
          (r) => r instanceof CSSFontFaceRule && /MiSans/.test(r.cssText),
        ),
    );
    if (sheet?.href) localStorage.setItem(FONTS_CSS, sheet.href);
  });
}

function rules(s: CSSStyleSheet) {
  try {
    return s.cssRules;
  } catch {
    return [];
  }
}

/**
 * Loads MiSans once the page has loaded (see lib/fonts). Pages that hold it for their images (the browse grid)
 * get a few seconds before it loads anyway, so a stalled image can't keep the site in system fonts.
 */
export function FontLoader() {
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const start = () => {
      timer = setTimeout(
        loadFonts,
        document.querySelector(`[${HOLD_FONTS}]`) ? 4000 : 0,
      );
    };
    if (document.readyState === "complete") start();
    else addEventListener("load", start, { once: true });
    return () => {
      removeEventListener("load", start);
      clearTimeout(timer);
    };
  }, []);
  return null;
}
