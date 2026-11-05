// The result grid. The server renders the first screen of tiles, so their covers are in the HTML from the
// start; the rest of the page's results travel as tile data and are drawn in the browser near the viewport
// (LaterTiles). Tiles: item-tile.tsx.

import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Grid } from "@astryxdesign/core/Grid";
import { Skeleton } from "@astryxdesign/core/Skeleton";
import { VStack } from "@astryxdesign/core/Stack";
import type { ItemResponse } from "@/lib/api-types";
import type { Rates, Settings } from "@/lib/settings-core";
import { ItemTile, tileOf } from "./item-tile";
import { LaterTiles } from "./later-tiles";

// Reflows to two columns on a phone; the floor is the narrowest tile whose title still reads.
const COLUMNS = { minWidth: 160 } as const;
// Tiles in the server HTML: more than a first screen at 390x844 (2 columns) and 1440x900 (8 columns), and a
// whole number of rows at 2, 3, 4, 6 and 8 columns.
export const FIRST_TILES = 24;

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
  return (
    <Grid columns={COLUMNS} gap={4}>
      {tiles.slice(0, FIRST_TILES).map((tile, i) => (
        <ItemTile key={tile.id} tile={tile} index={i} />
      ))}
      {later.length > 0 && <LaterTiles tiles={later} from={FIRST_TILES} />}
    </Grid>
  );
}

const PLACEHOLDERS = Array.from({ length: 12 }, (_, i) => i);

/** Shown while the first results load: the grid's geometry, so nothing moves when they arrive. */
export function ResultsSkeleton() {
  return (
    <VStack gap={6} padding={6}>
      <Skeleton width="40%" height={32} />
      <Grid columns={COLUMNS} gap={4}>
        {PLACEHOLDERS.map((i) => (
          <VStack key={i} gap={2}>
            <AspectRatio ratio={1}>
              <Skeleton index={i} />
            </AspectRatio>
            <Skeleton index={i} height={16} />
          </VStack>
        ))}
      </Grid>
    </VStack>
  );
}
