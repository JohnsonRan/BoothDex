// The grid of a list the browser has whole (the recent view): the first screen of tiles at once, the rest near the
// viewport (LaterTiles). The search results are their own grid, growing page by page (result-pages.tsx).
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
}: {
  items: ItemResponse[];
  settings: Settings;
  rates: Rates;
  /** in the visitor's language (useUi's) */
  t: Translate;
  /** off in the recent view, where every item was opened */
  markViewed?: boolean;
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
          tiles={items
            .slice(FIRST_TILES)
            .map((item) => tileOf(item, settings, rates, t))}
          from={FIRST_TILES}
          markViewed={markViewed}
          favLabel={favLabel}
        />
      )}
    </Grid>
  );
}
