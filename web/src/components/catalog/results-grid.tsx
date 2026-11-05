// The result grid. The server renders the first screen of tiles, so their covers are in the HTML from the
// start; the rest of the page's results travel as tile data and are drawn in the browser near the viewport
// (LaterTiles). Tiles: item-tile.tsx; the loading silhouette: loading.tsx.

import { Grid } from "@astryxdesign/core/Grid";
import type { ItemResponse } from "@/lib/api-types";
import { ROWS } from "@/lib/motion";
import { type Rates, type Settings, translator } from "@/lib/settings-core";
import { FIRST_TILES, GRID_COLUMNS, ItemTile, tileOf } from "./item-tile";
import { LaterTiles } from "./later-tiles";

export function ResultsGrid({
  items,
  settings,
  rates,
}: {
  items: ItemResponse[];
  settings: Settings;
  rates: Rates;
}) {
  const tiles = items.map((item) => tileOf(item, settings, rates));
  const later = tiles.slice(FIRST_TILES);
  const favLabel = translator(settings.ui)("fav.add");
  return (
    <Grid {...ROWS} columns={GRID_COLUMNS} gap={4}>
      {tiles.slice(0, FIRST_TILES).map((tile, i) => (
        <ItemTile key={tile.id} tile={tile} index={i} favLabel={favLabel} />
      ))}
      {later.length > 0 && (
        <LaterTiles tiles={later} from={FIRST_TILES} favLabel={favLabel} />
      )}
    </Grid>
  );
}
