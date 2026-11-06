"use client";
// Every favourite button's one handler and its saved state. The buttons carry `data-fav="<id>"` (item-tile.tsx,
// the item dialog) and no handler, so server tiles stay server markup: one document listener toggles, and the
// saved ones get `aria-pressed="true"`, which fills the heart (globals.css; the pressed icon of block
// ToggleButtonShowcase). Buttons added later (more tiles, another page, the dialog) are marked as they arrive.

import { useEffect } from "react";
import { useAccount } from "./session";

export function Favourites() {
  const { favs, toggleFav } = useAccount();

  useEffect(() => {
    const click = (e: MouseEvent) => {
      const button =
        e.target instanceof Element &&
        e.target.closest<HTMLElement>("[data-fav]");
      if (button) toggleFav(Number(button.dataset.fav));
    };
    document.addEventListener("click", click);
    return () => document.removeEventListener("click", click);
  }, [toggleFav]);

  useEffect(() => {
    const mark = () => {
      for (const b of document.querySelectorAll<HTMLElement>("[data-fav]")) {
        const saved = String(!!favs?.has(Number(b.dataset.fav)));
        if (b.getAttribute("aria-pressed") !== saved)
          b.setAttribute("aria-pressed", saved);
      }
    };
    mark();
    // one pass per frame however many nodes a render adds
    let frame = 0;
    const later = new MutationObserver(() => {
      if (!frame)
        frame = requestAnimationFrame(() => {
          frame = 0;
          mark();
        });
    });
    later.observe(document.body, { childList: true, subtree: true });
    return () => {
      later.disconnect();
      cancelAnimationFrame(frame);
    };
  }, [favs]);

  return null;
}
