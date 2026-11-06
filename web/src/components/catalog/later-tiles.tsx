"use client";
// The rest of a results page, drawn once the placeholder after the last server tile comes within a screen or
// two of the viewport. The server renders only the first screen of tiles; the rest are not in the HTML at
// all (as data they were ~11 KB gzip, the difference between FCP 1.81 and 1.66 s): the browser repeats the
// page's search, which the API answers from its cache, and draws what the server did not. In the recent view
// the browser has the items already and passes them in. They appear at once, without the entrance: they
// are drawn below the first screen. A failed read leaves the site's inline retry notice in their place.

import { GridSpan } from "@astryxdesign/core/Grid";
import { startTransition, useEffect, useRef, useState } from "react";
import { useUi } from "@/components/providers";
import { api } from "@/lib/api";
import type { ItemResponse, SearchResponse } from "@/lib/api-types";
import type { Rates, Settings } from "@/lib/settings-core";
import { ItemTile, type Tile, tileOf } from "./item-tile";
import { RefreshFailed } from "./refresh";
import { TileSilhouette } from "./silhouette";

/** Where the browser gets the later tiles: the page's search and what the server already drew. */
type LaterSource = {
  /** the server's /api/search request for this page */
  path: string;
  /** the server tiles' items, left out if the answer repeats them */
  skip: number[];
  settings: Settings;
  rates: Rates;
};

export function LaterTiles({
  tiles,
  source,
  from,
  markViewed,
  favLabel,
}: {
  /** the tiles, when the browser has them already */
  tiles?: Tile[];
  source?: LaterSource;
  from: number;
  markViewed: boolean;
  favLabel: string;
}) {
  const { t } = useUi();
  const [near, setNear] = useState(false);
  const [items, setItems] = useState<ItemResponse[] | null>(null);
  const [failed, setFailed] = useState(false);
  const marker = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = marker.current;
    if (!el) return;
    const seen = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) setNear(true);
      },
      { rootMargin: "0px 0px 200% 0px" },
    );
    seen.observe(el);
    return () => seen.disconnect();
  }, []);
  useEffect(() => {
    if (!near || !source || items || failed) return;
    let live = true;
    api<SearchResponse>(source.path).then(
      (r) => {
        const skip = new Set(source.skip);
        // a transition: drawing them yields to the page between tiles instead of holding it in one task
        if (live)
          startTransition(() =>
            setItems(r.items.filter((i) => !skip.has(i.id))),
          );
      },
      () => live && setFailed(true),
    );
    return () => {
      live = false;
    };
  }, [near, source, items, failed]);

  const shown =
    tiles ??
    (source && items?.map((i) => tileOf(i, source.settings, source.rates, t)));
  if (near && shown)
    return shown.map((tile, i) => (
      <ItemTile
        key={tile.id}
        tile={tile}
        index={from + i}
        markViewed={markViewed}
        favLabel={favLabel}
      />
    ));
  if (failed)
    return (
      <GridSpan columns="full">
        <RefreshFailed starting={false} onRetry={() => setFailed(false)} />
      </GridSpan>
    );
  return <TileSilhouette ref={marker} />;
}
