"use client";
// The filters in a rail left of the results, for a visitor who puts them at the side (Settings, settings-core
// FILTER_PLACES): the page Layout's start panel (Layout `start` with LayoutPanel, Layout.doc "Page with a
// sidebar"), the row's clauses one under another at its width, a set field laid open (filter-clauses.tsx
// `expand`). The search, the count and the sort stay in the toolbar (filter-bar.tsx). It sticks under the top bar
// as the band does (band.tsx), scrolling on its own when taller than the window. A compact surface has no room for
// it: the row keeps the clauses there (app/filter-row.css hides the server's rail before this knows).

import { useMediaQuery } from "@astryxdesign/core/hooks";
import { LayoutPanel } from "@astryxdesign/core/Layout";
import { StackItem, VStack } from "@astryxdesign/core/Stack";
import type { CSSProperties } from "react";
import { useUi } from "@/components/providers";
import type { ClauseNames } from "./clause-text";
import { useClauses } from "./filter-clauses";
import { COMPACT_SURFACE_QUERY, type Facets, useFacets } from "./filter-style";

// the top bar's height, as band.tsx reads it
const TOP = "var(--_app-shell-header-height, 0px)";
const RAIL: CSSProperties = {
  position: "sticky",
  insetBlockStart: TOP,
  // its own height, not the row's, so it can stick; the window's, so its divider runs the window's height
  alignSelf: "flex-start",
  blockSize: `calc(100dvh - ${TOP})`,
};
// each control takes the rail's width, as the folded clauses fill their panel (filter-bar.tsx)
const FILL: CSSProperties = { display: "flex", flexDirection: "column" };

export function FilterRail({
  facets,
  names,
}: {
  facets: Promise<Facets>;
  names: ClauseNames;
}) {
  const { t } = useUi();
  const isCompactSurface = useMediaQuery(COMPACT_SURFACE_QUERY);
  const clauses = useClauses(useFacets(facets), names, true);
  if (isCompactSurface) return null;
  return (
    <LayoutPanel
      hasDivider
      role="region"
      label={t("filter.rail")}
      width="15rem"
      padding={0}
      className="filter-rail"
      style={RAIL}
    >
      {/* the band's toolbar inset above (band.tsx), so the first control lines up with the search */}
      <VStack
        gap={3}
        paddingInlineStart={6}
        paddingInlineEnd={4}
        paddingBlockStart={4}
        paddingBlockEnd={6}
      >
        {clauses.map((c) => (
          <StackItem key={c.key} style={FILL}>
            {c}
          </StackItem>
        ))}
      </VStack>
    </LayoutPanel>
  );
}
