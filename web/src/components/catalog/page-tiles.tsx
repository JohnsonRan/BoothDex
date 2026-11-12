"use client";
// One page of the results grid (result-pages.tsx) as its tiles, or as boxes of their heights while it is far from
// view, going from one to the other a few tiles a frame.

import { GridSpan } from "@astryxdesign/core/Grid";
import { useEffect, useRef, useState } from "react";
import type { Tile } from "./item-tile";
import { Tiles } from "./tiles";

// tiles a gradual page draws or gives up a frame: four rows at six columns. Each step lays the whole grid out
// again, so twelve a frame (nine steps a page) held the main thread over 100 ms more often than the page at once
const STEP = 24;

/**
 * A page's tiles, or while it is far from view a box of each tile's height in its place. A gradual page goes from
 * one to the other STEP tiles a frame, the rest standing as boxes (one not drawn yet as tall as a measured tile,
 * `--_tile-h`): a page of 100 tiles drawn in one commit held the main thread for its style and layout, about 0.2 s
 * (over 1 s at 4x CPU), mid-fling. Only a change that moves nothing in view is gradual (Page.gradual).
 */
export function PageTiles({
  tiles,
  heights,
  gradual,
  from,
  favLabel,
}: {
  tiles: Tile[];
  heights?: number[];
  gradual: boolean;
  from: number;
  favLabel: string;
}) {
  const target = heights ? 0 : tiles.length;
  const [drawn, setDrawn] = useState(gradual ? 0 : target);
  if (!gradual && drawn !== target) setDrawn(target);
  // the heights the page last stood at, for its boxes as it is drawn again
  const last = useRef(heights);
  useEffect(() => {
    if (heights) last.current = heights;
  }, [heights]);
  useEffect(() => {
    if (drawn === target) return;
    const frame = requestAnimationFrame(() =>
      setDrawn((d) =>
        d < target ? Math.min(target, d + STEP) : Math.max(target, d - STEP),
      ),
    );
    return () => cancelAnimationFrame(frame);
  }, [drawn, target]);
  const box = heights ?? last.current;
  return (
    <>
      <Tiles
        tiles={drawn < tiles.length ? tiles.slice(0, drawn) : tiles}
        from={from}
        markViewed
        favLabel={favLabel}
      />
      {tiles.slice(drawn).map((tile, i) => (
        <GridSpan
          key={tile.id}
          aria-hidden
          className="page-box"
          style={{ blockSize: box?.[drawn + i] ?? "var(--_tile-h)" }}
        />
      ))}
    </>
  );
}
