"use client";
// Loading states of a text search on the home page (home-screen.tsx, Pending; other pages come in one paint). The
// real frame (title, search, avatar strip, toolbar) streams first; these hold the places of what waits for the
// search, the result count and the grid, so nothing moves when the results replace them (`tools/loading-layout.cjs`
// checks this, given a URL with `q`). Marked PLACEHOLDER: what replaces them enters.
// A client module only so the RSC payload carries one reference instead of every Skeleton of the grid.

import { Grid } from "@astryxdesign/core/Grid";
import { VStack } from "@astryxdesign/core/Stack";
import { FIRST_TILES, GRID } from "@/components/catalog/results/item-tile";
import { PLACEHOLDER } from "@/lib/motion";
import { TextSilhouette, TileSilhouette } from "./silhouette";

/** The result count beside the title. */
export function CountSilhouette() {
  return (
    // the bar, not the box, is 6em wide: in the filter row's readout the box takes the readout's free width and
    // sets the bar at its end, as it does the count (app/filter-row.css)
    <VStack {...PLACEHOLDER}>
      <TextSilhouette type="supporting" width="6em" />
    </VStack>
  );
}

const TILES = Array.from({ length: FIRST_TILES }, (_, i) => i);

/** The grid while the first results load: as many tiles as the server renders, in the same grid. */
export function ResultsSilhouette({ label }: { label: string }) {
  return (
    <VStack {...PLACEHOLDER} gap={6} role="status" aria-label={label}>
      <Grid {...GRID}>
        {TILES.map((i) => (
          <TileSilhouette key={i} />
        ))}
      </Grid>
    </VStack>
  );
}
