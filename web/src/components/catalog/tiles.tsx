"use client";
// A run of result tiles from their data, from the server's first screen (results-grid.tsx) or the browser's later
// tiles (later-tiles.tsx). A client module so the RSC payload carries each tile's data, about 0.3 KB, instead of
// its rendered Astryx elements and class names, about 4.5 KB; the server HTML is the same.

import { ItemTile, type Tile } from "./item-tile";

export function Tiles({
  tiles,
  from = 0,
  markViewed,
  favLabel,
}: {
  tiles: Tile[];
  /** the first tile's place in the page, which decides how eagerly its cover loads */
  from?: number;
  markViewed: boolean;
  favLabel: string;
}) {
  return tiles.map((tile, i) => (
    <ItemTile
      key={tile.id}
      tile={tile}
      index={from + i}
      markViewed={markViewed}
      favLabel={favLabel}
    />
  ));
}
