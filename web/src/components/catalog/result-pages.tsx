"use client";
// The results as one endless grid (the table-filter template's useInfiniteBatches, page.tsx:1160-1237, with our
// /api/search pages for its timer). The server renders the page in the URL, its first screen of tiles in the HTML;
// the rest of that page and the next pages are read here and drawn in the same grid, so rows run on across pages.
// A start past page 1 puts the pages before it above it as boxes of a page's estimated height (its rows at the row
// pitch measured from the server's tiles), the window scrolled by as much before the first paint, so the
// scrollbar shows the place and scrolling up just works: the page in view is read into its box, whichever it is.
// Only ABOVE pages are put up at once; more join as the reader nears the top of them, which keeps the document
// within every engine's element height (2,000 pages on a two-column phone would be about 30 million px).
// The address bar names the page in view (replaced, never pushed), and the history entry keeps the first tile in
// view, so a reload or a return lands on it with the pages it had.
// Pages more than SCREENS screens from view give their tiles up for boxes of the same heights (a page not read
// yet is one box; near view, silhouette tiles); a box paints the tiles' silhouettes (silhouette.tsx
// silhouetteMask), so no speed of scrolling shows an empty page, and every change keeps the first tile in view
// where it was (the grid opts out of the browser's own scroll anchoring, which not every engine has, so all of
// them move the same way). A change above the reader waits for scrolling to stop: moving the scroll position
// during an iOS fling stops the fling dead. A page that arrives or comes back below the reader, or gives its tiles
// up for boxes of their heights, does it a few tiles a frame (PageTiles), so a fling never waits on a whole page's
// layout. Every place is measured from the filter band's bottom edge (band.tsx covered()), so the tile in view is
// the first one below the band, not one under it.
// The parts: the reads and the changes they bring (use-page-reads.ts), the place kept through them
// (use-result-place.ts), what scrolling does (use-result-frame.ts), a page's tiles (page-tiles.tsx) and the page in
// view for the readout (page-in-view.tsx); their decisions are lib/result-pages.ts and lib/result-frame.ts.

import { Grid, GridSpan } from "@astryxdesign/core/Grid";
import { VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import {
  type CSSProperties,
  Fragment,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { useUi } from "@/components/providers";
import { type Filters, searchQuery } from "@/lib/catalog";
import { ROWS } from "@/lib/motion";
import { type Dir, withFailed } from "@/lib/result-pages";
import { LOCALES, type Rates, type Settings } from "@/lib/settings-core";
import { FIRST_TILES, GRID, type Tile } from "./item-tile";
import { PageTiles } from "./page-tiles";
import { RefreshFailed } from "./refresh";
import { silhouetteMask, TileSilhouette } from "./silhouette";
import { type Page, usePageReads, useResultRefs } from "./use-page-reads";
import { useResultFrame } from "./use-result-frame";
import { useResultPlace } from "./use-result-place";

// a silhouette in the first column, out of the flow and unseen, measured for the boxes' mask
const PROBE: CSSProperties = {
  position: "absolute",
  gridArea: "1 / 1 / 2 / 2",
  insetInline: 0,
  insetBlockStart: 0,
  visibility: "hidden",
};

export function ResultPages({
  page,
  total,
  count,
  pageSize,
  tiles,
  partial,
  filters,
  lexical,
  settings,
  rates,
}: {
  /** the page the server rendered */
  page: number;
  total: number;
  count: number;
  pageSize: number;
  /** the server's tiles of `page` */
  tiles: Tile[];
  /** `page` has more results than `tiles` */
  partial: boolean;
  filters: Filters;
  /** the first page went without meaning-based matches: the others are read the same way */
  lexical: boolean;
  settings: Settings;
  rates: Rates;
}) {
  const { t } = useUi();
  const [pages, setPages] = useState<Page[]>(() => [
    { n: page, tiles, partial },
  ]);
  const [loading, setLoading] = useState<Dir | null>(null);
  const [failed, setFailed] = useState(false);
  const [cols, setCols] = useState(1);
  const r = useResultRefs(pages);
  // the boxes' tile mask, for the look it was measured in; a hidden silhouette stands until it is measured
  const [mask, setMask] = useState<{ look: string; style: CSSProperties }>();
  const probe = useRef<HTMLDivElement>(null);
  const tileMask = mask?.look === settings.style ? mask.style : undefined;
  useLayoutEffect(() => {
    if (tileMask || !probe.current || !r.grid.current) return;
    setMask({
      look: settings.style,
      style: silhouetteMask(probe.current, r.grid.current),
    });
  });
  useLayoutEffect(() => {
    r.now.current = { pages };
  });

  const key = `${searchQuery(filters, 1, pageSize)}${lexical ? "&sem=0" : ""}`;
  const reads = usePageReads(r, {
    filters,
    pageSize,
    lexical,
    total,
    settings,
    rates,
    setPages,
    setLoading,
    setFailed,
  });
  const { keep } = reads;
  useResultPlace(r, reads, {
    page,
    total,
    key,
    settings,
    pages,
    failed,
    loading,
    setPages,
  });

  const last = pages[pages.length - 1];
  const hasNext = !!last.partial || last.n < total;
  const hasPrev = pages[0].n > 1;
  useResultFrame(r, reads, {
    page,
    pageSize,
    key,
    lexical,
    hasNext,
    hasPrev,
    loading,
    failed,
    setPages,
    setCols,
    setMask,
  });

  const favLabel = t("fav.add");
  return (
    <>
      <Grid
        ref={r.grid}
        {...ROWS}
        gap={GRID.gap}
        style={{
          ...GRID.style,
          ...tileMask,
          position: "relative",
          overflowAnchor: "none",
        }}
      >
        {pages.map((q) => (
          <Fragment key={q.n}>
            {q.failed ? (
              <GridSpan columns="full">
                <RefreshFailed
                  starting={false}
                  onRetry={() =>
                    keep(() => setPages((p) => withFailed(p, q.n, false)))
                  }
                />
              </GridSpan>
            ) : !q.tiles.length ? (
              q.near ? (
                Array.from({ length: pageSize }, (_, i) => (
                  // biome-ignore lint/suspicious/noArrayIndexKey: a page of silhouettes
                  <TileSilhouette key={i} />
                ))
              ) : (
                <GridSpan
                  columns="full"
                  aria-hidden
                  className="page-box"
                  style={{ blockSize: q.block }}
                />
              )
            ) : (
              <PageTiles
                tiles={q.tiles}
                heights={q.heights}
                // a return draws its pages whole, so the place it restores is there to scroll to
                gradual={!!q.gradual && !r.restoring.current}
                from={q.n === page ? 0 : FIRST_TILES}
                favLabel={favLabel}
              />
            )}
          </Fragment>
        ))}
        {hasNext &&
          (failed ? (
            <GridSpan columns="full">
              <RefreshFailed
                starting={false}
                onRetry={() => keep(() => setFailed(false))}
              />
            </GridSpan>
          ) : (
            Array.from({ length: cols }, (_, i) => (
              // new ones after each page: the old ones go with the wait they stood for, instead of moving down
              // past the tiles that arrived (a layout shift)
              <TileSilhouette
                // biome-ignore lint/suspicious/noArrayIndexKey: a fixed row of silhouettes
                key={`${last.n}${last.partial ? "+" : ""}-${i}`}
                ref={i === 0 ? r.end : undefined}
              />
            ))
          ))}
        {hasPrev && (
          <GridSpan
            ref={r.top}
            aria-hidden
            style={{ position: "absolute", insetBlockStart: 0 }}
          />
        )}
        {/* last, as the sentinel: the children before them are the pages' */}
        {!tileMask && <TileSilhouette ref={probe} style={PROBE} />}
      </Grid>
      {/* template :4527-4537, once every page is on screen */}
      {!hasNext && pages.every((q) => q.tiles.length) && total > 1 && (
        <VStack minHeight={56} vAlign="center" hAlign="center">
          <Text type="supporting" color="secondary">
            {t("list.allLoaded", {
              n: count.toLocaleString(LOCALES[settings.ui]),
            })}
          </Text>
        </VStack>
      )}
    </>
  );
}
