// One result tile (Product Gallery template's ProductCard): square media, title, supporting line, price; then
// NEW and feature tags as Badges (Badge Category Tags block). Not over the cover: Overlay is a client component,
// and ~70 of them cost ~1.3 s of Lighthouse TBT on hydration. No directive: the server renders the first screen
// of tiles and LaterTiles renders the rest in the browser, both from this module.

import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Badge } from "@astryxdesign/core/Badge";
import { Card } from "@astryxdesign/core/Card";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Heading, Text } from "@astryxdesign/core/Text";
import type { ItemResponse } from "@/lib/api-types";
import { FEATURES, type Feature, imgUrl, isNew, priceOf } from "@/lib/catalog";
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
  price: formatPrice(priceOf(item), settings, rates),
  marks: [
    ...(isNew(item) ? ["NEW"] : []),
    ...item.features.slice(0, 2).map((f) => FEATURES[f as Feature] ?? f),
  ],
});

// Roughly the first screen (two columns on phones, five or six on a desktop): those covers load at once,
// the first few with high priority since one of them is the likely LCP image.
const EAGER = 12;
const HIGH = 4;

export function ItemTile({ tile, index }: { tile: Tile; index: number }) {
  return (
    <VStack gap={2}>
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
        >
          {tile.title}
        </Heading>
        <Text type="supporting" maxLines={1} hasTruncateTooltip={false}>
          {tile.shop}
        </Text>
        <Text type="body" weight="bold" hasTabularNumbers>
          {tile.price}
        </Text>
        {tile.marks.length > 0 && (
          <HStack gap={1} wrap="wrap">
            {tile.marks.map((m) => (
              <Badge
                key={m}
                label={m}
                variant={m === "NEW" ? "blue" : undefined}
              />
            ))}
          </HStack>
        )}
      </VStack>
    </VStack>
  );
}
