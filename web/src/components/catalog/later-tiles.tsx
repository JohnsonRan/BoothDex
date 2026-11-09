"use client";
// The rest of a list the browser already has (the recent view), drawn once the placeholder after the first screen
// of tiles comes within a screen or two of the viewport. They appear at once, without the entrance: they are drawn
// below the first screen. The search results grow the same way, page by page (result-pages.tsx).

import { useEffect, useRef, useState } from "react";
import type { Tile } from "./item-tile";
import { TileSilhouette } from "./silhouette";
import { Tiles } from "./tiles";

export function LaterTiles({
  tiles,
  from,
  markViewed,
  favLabel,
}: {
  tiles: Tile[];
  from: number;
  markViewed: boolean;
  favLabel: string;
}) {
  const [near, setNear] = useState(false);
  const marker = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = marker.current;
    if (!el) return;
    const seen = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) setNear(true);
      },
      { rootMargin: "0px 0px 200% 0px" },
    );
    seen.observe(el);
    return () => seen.disconnect();
  }, []);
  if (near)
    return (
      <Tiles
        tiles={tiles}
        from={from}
        markViewed={markViewed}
        favLabel={favLabel}
      />
    );
  return <TileSilhouette ref={marker} />;
}
