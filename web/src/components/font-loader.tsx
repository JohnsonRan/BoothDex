"use client";

import { useEffect } from "react";
import { FONTS_CSS, imagesPainted } from "@/lib/fonts";

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
}

function rules(s: CSSStyleSheet) {
  try {
    return s.cssRules;
  } catch {
    return [];
  }
}

/**
 * Loads MiSans once the page has loaded and its first-screen covers (the eager images, rendered on the server)
 * are painted, so its slices never compete with the LCP image (see lib/fonts).
 */
export function FontLoader() {
  useEffect(() => {
    const start = () =>
      imagesPainted([
        ...document.querySelectorAll<HTMLImageElement>('img[loading="eager"]'),
      ]).then(loadFonts);
    if (document.readyState === "complete") start();
    else addEventListener("load", start, { once: true });
    return () => removeEventListener("load", start);
  }, []);
  return null;
}
