"use client";

import { useEffect } from "react";
import {
  coverPainted,
  FONTS_CSS,
  FONTS_USED,
  FONTS_USED_MAX,
} from "@/lib/fonts";

let started = false;

/** Starts loading MiSans and remembers its stylesheet for the next visit; later calls do nothing. */
function loadFonts() {
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
  document.fonts.addEventListener("loadingdone", rememberSlices);
}

/** Adds the slices drawn so far to the ones the next visit preloads (FONTS_EARLY). */
function rememberSlices() {
  try {
    const used = new Set<string>(
      JSON.parse(localStorage.getItem(FONTS_USED) ?? "[]"),
    );
    for (const e of performance.getEntriesByType("resource"))
      if (/MiSans-VF\.\d+\.[^/]*woff2$/.test(e.name)) used.add(e.name);
    localStorage.setItem(
      FONTS_USED,
      JSON.stringify([...used].slice(-FONTS_USED_MAX)),
    );
  } catch {}
}

function rules(s: CSSStyleSheet) {
  try {
    return s.cssRules;
  } catch {
    return [];
  }
}

/**
 * Loads MiSans once the page has loaded and a first-screen cover (the eager images, rendered on the server) is
 * the reported largest paint, so its slices never compete with the LCP image (see lib/fonts).
 */
export function FontLoader() {
  useEffect(() => {
    const start = () =>
      coverPainted([
        ...document.querySelectorAll<HTMLImageElement>('img[loading="eager"]'),
      ]).then(loadFonts);
    if (document.readyState === "complete") start();
    else addEventListener("load", start, { once: true });
    return () => removeEventListener("load", start);
  }, []);
  return null;
}
