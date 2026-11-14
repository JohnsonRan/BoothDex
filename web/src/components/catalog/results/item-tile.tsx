// One result tile, adapted from the Product Gallery template's ProductCard (@astryxdesign/cli assets/templates/pages/product-gallery):
// square media, then title, a secondary line (the shop, with its icon) and the price in large bold; the price
// row ends with the favourite button. NEW and feature tags sit over the cover's lower edge as Badges (master's
// place; Badge Category Tags block), stacked in the cover's grid rather than in an Overlay: Overlay is a client
// component, and ~70 of them cost ~1.3 s of Lighthouse TBT on hydration; only a viewed tile mounts one
// (viewed-mark.tsx). A pointer resting on the tile shows the item's second image (app/catalog.css .tile-second),
// whose address item-host.tsx's one listener sets on the first hover, so no tile downloads it unasked.
// No directive: the server builds the tiles' data (tileOf) and Tiles (tiles.tsx) draws them, from the server's
// first screen and the browser's later tiles. The favourite button carries no handler of its own: one listener
// answers every one (account/favourites.tsx). The viewed mark is the tile's only stateful part; the shop icon is
// plain markup rather than an Avatar.

import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Badge } from "@astryxdesign/core/Badge";
import { Card } from "@astryxdesign/core/Card";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import { Heading, Text } from "@astryxdesign/core/Text";
import { SkeletonImage } from "@/components/catalog/silhouette";
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
  contentLang,
  formatPrice,
  itemText,
  type Rates,
  type Settings,
  type Translate,
} from "@/lib/settings-core";
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
  /** the second image, shown while a pointer rests on the tile ("" when the item has none) */
  second: string;
};

export const tileOf = (
  item: ItemResponse,
  settings: Settings,
  rates: Rates,
  t: Translate,
): Tile => ({
  id: item.id,
  cover: imgUrl(item, 0),
  title: itemText(item, "title", contentLang(settings)),
  shop: item.author_name,
  shopIcon: imageSrc(item.author_icon),
  price: formatPrice(priceOf(item), settings, rates, t),
  marks: [
    ...(isNew(item) ? ["NEW"] : []),
    ...item.features.slice(0, 2).map((f) => FEATURES[f as Feature] ?? f),
  ],
  second: imgUrl(item, 1),
});

// Template product-gallery's Grid (page.tsx:148-149), with master's density (owner: 「太大了。為什麼一頁只有4張圖了」):
// master's 160 px floor and gap 4, in rem so it follows the root scale, and always two columns on a phone.
const GAP = 4;
export const GRID = { gap: GAP, ...remColumns("10rem", "fill", GAP) } as const;
// Tiles in the server HTML: at least a first screen up to 1920 px (24 there, 8 columns with the root at 106 %), in whole
// rows at 2, 4, 6 and 8 columns (5 at 1280 px leave a short last row); wider, the first screen shows more (33 at
// 2560 px) and the browser draws the rest. Measured 2026-11-13 with the filter rail at its default width.
export const FIRST_TILES = 24;

// The first screen: two rows at nine columns (1920 px) load at once, the first row at six columns (1280-1440 px)
// with high priority, since one of them is the likely LCP image.
const EAGER = 18;
const HIGH = 6;

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
        {/* one grid cell holding the cover, the second image over it and the badges at its lower edge */}
        <span className="tile-cover">
          <SkeletonImage
            src={tile.cover}
            loading={index < EAGER ? "eager" : "lazy"}
            fetchPriority={index < HIGH ? "high" : undefined}
            decoding="async"
          />
          {tile.second && (
            <img
              className="tile-second"
              data-second={tile.second}
              alt=""
              decoding="async"
            />
          )}
          {tile.marks.length > 0 && (
            <HStack gap={1} wrap="wrap" className="tile-marks">
              {tile.marks.map((m) => (
                <Badge
                  key={m}
                  label={m}
                  variant={m === "NEW" ? "blue" : undefined}
                />
              ))}
            </HStack>
          )}
        </span>
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
          <StackItem size="fill">
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
        <HStack gap={2} vAlign="center" minHeight="var(--size-element-sm)">
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
      </VStack>
    </VStack>
  );
}
