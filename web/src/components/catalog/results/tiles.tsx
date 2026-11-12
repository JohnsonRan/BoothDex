"use client";
// A run of result tiles from their data: a page of the results (result-pages.tsx), the server's first screen
// among them, or the recent view's (results-grid.tsx). A client module so the RSC payload carries each tile's
// data, about 0.3 KB, instead of its rendered Astryx elements and class names, about 4.5 KB; the server HTML is
// the same.

import { memo, useEffect } from "react";
import { keepTiles } from "@/components/item/item-cache";
import { ItemTile, type Tile } from "./item-tile";

// a tile renders again only when its own data changes: a page drawn a few tiles a frame (page-tiles.tsx
// PageTiles), or any change to the pages, would otherwise render every tile already drawn, again, each time
const Item = memo(ItemTile);

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
  // the item card draws them while its record loads
  useEffect(() => keepTiles(tiles), [tiles]);
  return tiles.map((tile, i) => (
    <Item
      key={tile.id}
      tile={tile}
      index={from + i}
      markViewed={markViewed}
      favLabel={favLabel}
    />
  ));
}
