// One result tile, adapted from the Product Gallery template's ProductCard (templates/pages/product-gallery):
// square media, then title, a secondary line (the shop, with its icon) and the price in large bold; the price
// row ends with the favourite button, and NEW and feature tags follow as Badges (Badge Category Tags block). Not over the cover: Overlay is a client component,
// and ~70 of them cost ~1.3 s of Lighthouse TBT on hydration. No directive: the server renders the first screen
// of tiles and LaterTiles renders the rest in the browser, both from this module. For the same reason a tile
// carries no handler of its own: one listener answers every favourite button (fav-toast.tsx), and the shop
// icon is server markup rather than a client Avatar.

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
import {
  formatPrice,
  itemText,
  type Rates,
  type Settings,
} from "@/lib/settings-core";

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
): Tile => ({
  id: item.id,
  cover: imgUrl(item, 0),
  title: itemText(item, "title", settings.content),
  shop: item.author_name,
  shopIcon: imageSrc(item.author_icon),
  price: formatPrice(priceOf(item), settings, rates),
  marks: [
    ...(isNew(item) ? ["NEW"] : []),
    ...item.features.slice(0, 2).map((f) => FEATURES[f as Feature] ?? f),
  ],
});

// Reflows to two columns on a phone; the floor is the narrowest tile whose title still reads.
export const GRID_COLUMNS = { minWidth: 160 } as const;
// Tiles in the server HTML: more than a first screen at 390x844 (2 columns) and 1440x900 (8 columns), and a
// whole number of rows at 2, 3, 4, 6 and 8 columns.
export const FIRST_TILES = 24;

// Roughly the first screen (two columns on phones, five or six on a desktop): those covers load at once,
// the first few with high priority since one of them is the likely LCP image.
const EAGER = 12;
const HIGH = 4;

/** The shop's icon at Avatar's xsm size: a round crop, a muted disc when the shop has none. */
function ShopIcon({ src }: { src: string }) {
  return (
    <span
      aria-hidden
      style={{
        display: "inline-flex",
        flex: "none",
        inlineSize: 20,
        blockSize: 20,
        borderRadius: "var(--radius-full)",
        overflow: "hidden",
        background: "var(--color-background-muted)",
      }}
    >
      {src && (
        <img
          src={src}
          alt=""
          width={20}
          height={20}
          loading="lazy"
          decoding="async"
          style={{ objectFit: "cover" }}
        />
      )}
    </span>
  );
}

export function ItemTile({
  tile,
  index,
  favLabel,
}: {
  tile: Tile;
  index: number;
  /** the favourite button's name, in the visitor's language */
  favLabel: string;
}) {
  return (
    // opened by item-host.tsx's one listener: a click anywhere on the tile but the favourite button
    <VStack gap={3} data-item={String(tile.id)} style={{ cursor: "pointer" }}>
      <Card padding={0}>
        <AspectRatio ratio={1} fit="cover">
          <img
            src={tile.cover}
            alt=""
            loading={index < EAGER ? "eager" : "lazy"}
            fetchPriority={index < HIGH ? "high" : undefined}
            decoding="async"
          />
        </AspectRatio>
      </Card>
      <VStack gap={1}>
        {/* level 4 is the body-sized heading; tiles still sit at outline level 2, under the page title */}
        <Heading
          level={4}
          accessibilityLevel={2}
          maxLines={2}
          hasTruncateTooltip={false}
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
            // answered by fav-toast.tsx
            data-fav=""
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
