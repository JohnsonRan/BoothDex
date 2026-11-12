"use client";
// The dialog while its item loads: the same Grid as the loaded dialog, with Skeletons in the boxes the first
// screen fills (hero image, counter row, language tabs, a two-line title, likes, price, marks and the two
// action rows), each text bar in its text's line box (as catalog/silhouette.tsx does for tiles). The boxes are
// reserved at once and the bars appear only after the wait has lasted (lib/use-delayed), so a quick answer
// shows no flash and nothing moves when it arrives. An item opened from a tile shows at once what the tile
// showed, in the loaded dialog's places: the cover as the hero, the title and the price.

import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Card } from "@astryxdesign/core/Card";
import { Grid } from "@astryxdesign/core/Grid";
import { Skeleton } from "@astryxdesign/core/Skeleton";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Heading, Text } from "@astryxdesign/core/Text";
import type { Tile } from "@/components/catalog/results/item-tile";
import { TextSilhouette } from "@/components/catalog/silhouette";
import { useUi } from "@/components/providers";
import { DIALOG_MAX_HEIGHT } from "@/lib/dialog-height";
import type { remColumns } from "@/lib/grid";
import { HERO } from "./item-data";

// what the tile showed is on screen at once, inside a silhouette still hidden for the wait
const SHOWN = { visibility: "visible" } as const;

/** The price as ItemInfo and PhoneBar draw it. */
const Price = ({ price }: { price: string }) => (
  <Text type="large" weight="bold" hasTabularNumbers style={SHOWN}>
    {price}
  </Text>
);

/** A control's box: Astryx element heights (sm 28, md 32, lg 36 px). */
const Control = ({
  size,
  width,
}: {
  size: "sm" | "md" | "lg";
  width: string;
}) => <Skeleton height={`var(--size-element-${size})`} width={width} />;

// Avatar size md
const AVATAR = 36;

/** The shop's avatar while loading; the header's text lines stay blank. */
export const HeaderSilhouette = ({ shown }: { shown: boolean }) => (
  <Skeleton
    width={AVATAR}
    height={AVATAR}
    radius="rounded"
    style={{ visibility: shown ? "visible" : "hidden" }}
  />
);

/** The phones' footer bar (PhoneBar): price, favourite, Buy on Booth. */
export const PhoneBarSilhouette = ({
  shown,
  price,
}: {
  shown: boolean;
  /** the tile's, when the item was opened from one */
  price?: string;
}) => (
  <HStack
    gap={2}
    vAlign="center"
    style={{ visibility: shown ? "visible" : "hidden" }}
  >
    <VStack style={{ flex: 1 }}>
      {price ? (
        <Price price={price} />
      ) : (
        <TextSilhouette type="large" width="4rem" />
      )}
    </VStack>
    <Control size="lg" width="var(--size-element-lg)" />
    <Control size="lg" width="8rem" />
  </HStack>
);

export function DetailSilhouette({
  shown,
  columns,
  isWide,
  tile,
}: {
  shown: boolean;
  /** the info column's action rows exist from lg up (item-info.tsx) */
  isWide: boolean;
  /** the loaded dialog's columns (lib/grid) */
  columns: ReturnType<typeof remColumns>;
  /** what the item's tile showed, when it was opened from one */
  tile?: Tile;
}) {
  const { t } = useUi();
  // the title's type (item-info.tsx)
  const title = isWide ? "display-2" : "heading-1";
  return (
    <Grid
      gap={5}
      aria-busy
      aria-label={t("detail.loading")}
      // as tall as the dialog allows, as a loaded item is: the centred dialog does not move when it arrives
      style={{
        ...columns.style,
        visibility: shown ? "visible" : "hidden",
        minBlockSize: DIALOG_MAX_HEIGHT,
        // the rows keep their own heights (one column on a phone), so the info starts where it will
        alignContent: "start",
      }}
    >
      <VStack gap={3}>
        <AspectRatio ratio={1}>
          {tile?.cover ? (
            // the gallery's hero card (item-gallery.tsx), with the tile's cover, which the browser already has
            <Card
              variant="muted"
              padding={0}
              width="100%"
              height="100%"
              style={SHOWN}
            >
              <img style={HERO} src={tile.cover} alt="" />
            </Card>
          ) : (
            <Skeleton />
          )}
        </AspectRatio>
        <HStack gap={2} vAlign="center">
          <Control size="md" width="var(--size-element-md)" />
        </HStack>
      </VStack>
      <VStack gap={5}>
        <VStack gap={2}>
          <Control size="sm" width="12rem" />
          {tile && isWide ? (
            <Text type="display-2" as="h1" style={SHOWN}>
              {tile.title}
            </Text>
          ) : tile ? (
            <Heading level={1} style={SHOWN}>
              {tile.title}
            </Heading>
          ) : (
            <VStack>
              <TextSilhouette type={title} lines={2} width="55%" />
            </VStack>
          )}
          <TextSilhouette type="body" width="3rem" />
          {tile ? (
            <Price price={tile.price} />
          ) : (
            <TextSilhouette type="large" width="6rem" />
          )}
        </VStack>
        <HStack gap={1}>
          <Skeleton width="4rem" height="var(--spacing-5)" />
          <Skeleton width="4rem" height="var(--spacing-5)" />
        </HStack>
        {isWide && (
          <VStack gap={2}>
            <Control size="lg" width="100%" />
            <Control size="lg" width="100%" />
          </VStack>
        )}
      </VStack>
    </Grid>
  );
}
