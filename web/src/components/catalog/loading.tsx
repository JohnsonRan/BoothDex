"use client";
// Loading states of the home page. The real frame (title, search, avatar strip, toolbar) streams first; these
// hold the places of what waits for the search, the result count and the grid, so nothing moves when the
// results replace them (`tools/loading-layout.cjs` checks this). Marked PLACEHOLDER: what replaces them enters.
// A client module only so the RSC payload carries one reference instead of every Skeleton of the grid.

import { Grid } from "@astryxdesign/core/Grid";
import { VStack } from "@astryxdesign/core/Stack";
import { PLACEHOLDER } from "@/lib/motion";
import { FIRST_TILES, GRID } from "./item-tile";
import { TextSilhouette, TileSilhouette } from "./silhouette";

/** The result count beside the title. */
export function CountSilhouette() {
  return (
    <VStack {...PLACEHOLDER} style={{ width: "6em" }}>
      <TextSilhouette type="supporting" width="100%" />
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
