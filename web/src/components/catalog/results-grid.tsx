// The result grid, rendered on the server so every cover is in the HTML. Tiles follow the Product Gallery
// template's ProductCard: square media, title, supporting line, price; NEW and feature markers sit on the cover
// as in the Overlay Bottom Strip block.

import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Badge } from "@astryxdesign/core/Badge";
import { Card } from "@astryxdesign/core/Card";
import { Grid } from "@astryxdesign/core/Grid";
import { Overlay } from "@astryxdesign/core/Overlay";
import { Skeleton } from "@astryxdesign/core/Skeleton";
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

// Reflows to two columns on a phone; the floor is the narrowest tile whose title still reads.
const COLUMNS = { minWidth: 160 } as const;
// Roughly the first screen (two columns on phones, five or six on a desktop): those covers load at once,
// the first few with high priority since one of them is the likely LCP image.
const EAGER = 12;
const HIGH = 4;

/** NEW, then up to two feature tags; null when there are none, so those covers get no overlay at all. */
function markersOf(item: ItemResponse) {
  const features = item.features.slice(0, 2);
  if (!isNew(item) && features.length === 0) return null;
  return (
    <HStack gap={1} wrap="wrap">
      {isNew(item) && <Badge label="NEW" variant="blue" />}
      {features.map((f) => (
        <Badge key={f} label={FEATURES[f as Feature] ?? f} />
      ))}
    </HStack>
  );
}

function ItemTile({
  item,
  index,
  settings,
  rates,
}: {
  item: ItemResponse;
  index: number;
  settings: Settings;
  rates: Rates;
}) {
  const cover = (
    <AspectRatio ratio={1} fit="cover">
      <img
        src={imgUrl(item, 0)}
        alt=""
        loading={index < EAGER ? "eager" : "lazy"}
        fetchPriority={index < HIGH ? "high" : undefined}
        decoding="async"
      />
    </AspectRatio>
  );
  const markers = markersOf(item);
  return (
    <VStack gap={2}>
      <Card padding={0}>
        {markers ? (
          <Overlay position="top" align="start" scrim={false} content={markers}>
            {cover}
          </Overlay>
        ) : (
          cover
        )}
      </Card>
      <VStack gap={1}>
        {/* level 4 is the body-sized heading; tiles still sit at outline level 2, under the page title */}
        <Heading
          level={4}
          accessibilityLevel={2}
          maxLines={2}
          hasTruncateTooltip={false}
        >
          {itemText(item, "title", settings.content)}
        </Heading>
        <Text type="supporting" maxLines={1} hasTruncateTooltip={false}>
          {item.author_name}
        </Text>
        <Text type="body" weight="bold" hasTabularNumbers>
          {formatPrice(priceOf(item), settings, rates)}
        </Text>
      </VStack>
    </VStack>
  );
}

export function ResultsGrid({
  items,
  settings,
  rates,
}: {
  items: ItemResponse[];
  settings: Settings;
  rates: Rates;
}) {
  return (
    <Grid columns={COLUMNS} gap={4}>
      {items.map((item, i) => (
        <ItemTile
          key={item.id}
          item={item}
          index={i}
          settings={settings}
          rates={rates}
        />
      ))}
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
