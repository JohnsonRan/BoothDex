"use client";
// The rest of a results page. The server renders only the first screen of tiles, so the HTML (and the RSC
// payload that repeats it) stays small; these tiles arrive as plain data with it and are drawn once the
// placeholder after the last server tile comes within a screen or two of the viewport.

import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Skeleton } from "@astryxdesign/core/Skeleton";
import { useEffect, useRef, useState } from "react";
import { ItemTile, type Tile } from "./item-tile";

export function LaterTiles({ tiles, from }: { tiles: Tile[]; from: number }) {
  const [shown, setShown] = useState(false);
  const marker = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = marker.current;
    if (!el) return;
    const seen = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) setShown(true);
      },
      { rootMargin: "0px 0px 200% 0px" },
    );
    seen.observe(el);
    return () => seen.disconnect();
  }, []);

  if (shown)
    return tiles.map((tile, i) => (
      <ItemTile key={tile.id} tile={tile} index={from + i} />
    ));
  return (
    <AspectRatio ratio={1}>
      <Skeleton ref={marker} />
    </AspectRatio>
  );
}
