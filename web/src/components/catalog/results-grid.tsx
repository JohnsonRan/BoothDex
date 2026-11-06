// The result grid. The server renders the first screen of tiles, so their covers are in the HTML from the
// start; the rest of the page's results are drawn in the browser near the viewport (LaterTiles), fetched
// again from `search` when the server renders, passed as data when the browser does (the recent view).
// Tiles: item-tile.tsx, drawn by tiles.tsx; the loading silhouette: loading.tsx.

import { Grid } from "@astryxdesign/core/Grid";
import type { ItemResponse } from "@/lib/api-types";
import { ROWS } from "@/lib/motion";
import type { Rates, Settings, Translate } from "@/lib/settings-core";
import { FIRST_TILES, GRID, tileOf } from "./item-tile";
import { LaterTiles } from "./later-tiles";
import { Tiles } from "./tiles";

export function ResultsGrid({
  items,
  settings,
  rates,
  t,
  markViewed = true,
  search,
}: {
  items: ItemResponse[];
  settings: Settings;
  rates: Rates;
  /** in the visitor's language: the server's translator, or useUi's in the browser */
  t: Translate;
  /** off in the recent view, where every item was opened */
  markViewed?: boolean;
  /** the /api/search request that answered `items`: the browser repeats it for the later tiles */
  search?: string;
}) {
  const first = items.slice(0, FIRST_TILES);
  const later = items.length > FIRST_TILES;
  const favLabel = t("fav.add");
  return (
    <Grid {...ROWS} {...GRID}>
      <Tiles
        tiles={first.map((item) => tileOf(item, settings, rates, t))}
        markViewed={markViewed}
        favLabel={favLabel}
      />
      {later && (
        <LaterTiles
          // a new search starts over
          key={search}
          source={
            search
              ? { path: search, skip: first.map((i) => i.id), settings, rates }
              : undefined
          }
          tiles={
            search
              ? undefined
              : items
                  .slice(FIRST_TILES)
                  .map((item) => tileOf(item, settings, rates, t))
          }
          from={FIRST_TILES}
          markViewed={markViewed}
          favLabel={favLabel}
        />
      )}
    </Grid>
  );
}
