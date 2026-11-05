// One result tile, adapted from the Product Gallery template's ProductCard (templates/pages/product-gallery):
// square media, then title, a secondary line (the shop, with its icon) and the price in large bold; the price
// row ends with the favourite button, and NEW and feature tags follow as Badges (Badge Category Tags block). Not over the cover: Overlay is a client component,
// and ~70 of them cost ~1.3 s of Lighthouse TBT on hydration; only a viewed tile mounts one (viewed-mark.tsx).
// No directive: the server renders the first screen of tiles and LaterTiles renders the rest in the browser,
// both from this module. For the same reason the favourite button carries no handler of its own: one listener
// answers every one (account/favourites.tsx). The viewed mark is the tile's only client part; the shop icon is
// server markup rather than a client Avatar.

import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Badge } from "@astryxdesign/core/Badge";
import { Card } from "@astryxdesign/core/Card";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import { Heading, Text } from "@astryxdesign/core/Text";
import type { ItemResponse } from "@/lib/api-types";
import {
  FEATURES,
  type Feature,
  imageSrc,
  imgUrl,
  isNew,
  priceOf,
} from "@/lib/catalog";
import { remColumns } from "@/lib/grid";
import {
  formatPrice,
  itemText,
  type Rates,
  type Settings,
  type Translate,
} from "@/lib/settings-core";
import { SkeletonImage } from "./silhouette";
import { ViewedMark } from "./viewed-mark";

/** What a tile shows, already in the visitor's language and currency (small enough to send to the browser). */
export type Tile = {
  id: number;
  cover: string;
  title: string;
  shop: string;
  shopIcon: string;
  price: string;
  /** NEW first, then up to two feature tags */
  marks: string[];
};

export const tileOf = (
  item: ItemResponse,
  settings: Settings,
  rates: Rates,
  t: Translate,
): Tile => ({
  id: item.id,
  cover: imgUrl(item, 0),
  title: itemText(item, "title", settings.content),
  shop: item.author_name,
  shopIcon: imageSrc(item.author_icon),
  price: formatPrice(priceOf(item), settings, rates, t),
  marks: [
    ...(isNew(item) ? ["NEW"] : []),
    ...item.features.slice(0, 2).map((f) => FEATURES[f as Feature] ?? f),
  ],
});

// Template product-gallery (page.tsx:148-149): `<Grid columns={{minWidth: 300}} gap={6}>`, "reflows 3 -> 2 -> 1",
// with the floor in rem (300 px at a 16 px root).
export const GRID = { gap: 6, ...remColumns("18.75rem") } as const;
// Tiles in the server HTML: a whole number of rows at 1 to 5 columns (390 to 1920 px wide at the 115 % root),
// and more than a first screen at each.
export const FIRST_TILES = 60;

// The first screen: two rows at five columns load at once, the first row at three columns (1280-1440 px) with
// high priority, since one of them is the likely LCP image.
const EAGER = 10;
const HIGH = 3;

/** The shop's icon at Avatar's xsm size (20 px at a 16 px root): a round crop, a muted disc when the shop has none. */
function ShopIcon({ src }: { src: string }) {
  return (
    <span
      aria-hidden
      style={{
        display: "inline-flex",
        flex: "none",
        inlineSize: "var(--spacing-5)",
        blockSize: "var(--spacing-5)",
        borderRadius: "var(--radius-full)",
        overflow: "hidden",
        background: "var(--color-background-muted)",
      }}
    >
      {src && (
        <img
          src={src}
          alt=""
          loading="lazy"
          decoding="async"
          style={{ width: "100%", height: "100%", objectFit: "cover" }}
        />
      )}
    </span>
  );
}

export function ItemTile({
  tile,
  index,
  markViewed,
  favLabel,
}: {
  tile: Tile;
  index: number;
  /** dims the cover of an item the visitor opened (not in the recent view, where every item was) */
  markViewed: boolean;
  /** the favourite button's name, in the visitor's language */
  favLabel: string;
}) {
  const cover = (
    <Card padding={0}>
      <AspectRatio ratio={1}>
        <SkeletonImage
          src={tile.cover}
          loading={index < EAGER ? "eager" : "lazy"}
          fetchPriority={index < HIGH ? "high" : undefined}
          decoding="async"
        />
      </AspectRatio>
    </Card>
  );
  return (
    // opened by item-host.tsx's one listener: a click anywhere on the tile but the favourite button
    <VStack gap={3} data-item={String(tile.id)} style={{ cursor: "pointer" }}>
      {markViewed ? <ViewedMark id={tile.id}>{cover}</ViewedMark> : cover}
      <VStack gap={1}>
        {/* level 4 is the body-sized heading; tiles still sit at outline level 2, under the page title */}
        <Heading
          level={4}
          accessibilityLevel={2}
          maxLines={2}
          hasTruncateTooltip={false}
          // regular weight, as the tiles were approved (docs/TEMPLATE-MAP.md)
          weight="normal"
          // always two lines tall, like its loading silhouette (silhouette.tsx), so tiles never shift
          style={{ minBlockSize: "2lh" }}
        >
          {/* the tile's link: keyboard focus, and a new tab on a modified click */}
          <a
            href={`?item=${tile.id}`}
            style={{ color: "inherit", textDecoration: "none" }}
          >
            {tile.title}
          </a>
        </Heading>
        <HStack gap={1.5} vAlign="center">
          <ShopIcon src={tile.shopIcon} />
          <StackItem size="fill" style={{ minWidth: 0 }}>
            <Text
              type="body"
              color="secondary"
              maxLines={1}
              hasTruncateTooltip={false}
            >
              {tile.shop}
            </Text>
          </StackItem>
        </HStack>
        {/* as tall as the favourite button, in the silhouette too */}
        <HStack
          gap={2}
          vAlign="center"
          style={{ minBlockSize: "var(--size-element-sm)" }}
        >
          <StackItem size="fill">
            <Text type="large" weight="bold" hasTabularNumbers>
              {tile.price}
            </Text>
          </StackItem>
          <IconButton
            // answered, and marked pressed while saved, by account/favourites.tsx
            data-fav={String(tile.id)}
            aria-pressed={false}
            label={favLabel}
            variant="ghost"
            size="sm"
            icon={<Icon icon="site:favourite" size="sm" />}
          />
        </HStack>
        {/* kept when empty: one badge tall, as in the silhouette */}
        <HStack
          gap={1}
          wrap="wrap"
          style={{ minBlockSize: "var(--spacing-5)" }}
        >
          {tile.marks.map((m) => (
            <Badge
              key={m}
              label={m}
              variant={m === "NEW" ? "blue" : undefined}
            />
          ))}
        </HStack>
      </VStack>
    </VStack>
  );
}
