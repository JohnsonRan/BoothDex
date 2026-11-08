"use client";
// The dialog while its item loads: the same Grid as the loaded dialog, with Skeletons in the boxes the first
// screen fills (hero image, counter row, language tabs, a two-line title, likes, price, marks and the two
// action rows), each text bar in its text's line box (as catalog/silhouette.tsx does for tiles). The boxes are
// reserved at once and the bars appear only after the wait has lasted (lib/use-delayed), so a quick answer
// shows no flash and nothing moves when it arrives.

import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Grid } from "@astryxdesign/core/Grid";
import { Skeleton } from "@astryxdesign/core/Skeleton";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { useUi } from "@/components/providers";
import { DIALOG_MAX_HEIGHT } from "@/lib/dialog-height";
import type { remColumns } from "@/lib/grid";

type TextType = "display-2" | "heading-1" | "body" | "large" | "supporting";

function Line({ type, width }: { type: TextType; width: string }) {
  return (
    <VStack
      style={{
        height: `calc(var(--text-${type}-leading) * var(--text-${type}-size))`,
        justifyContent: "center",
      }}
    >
      <Skeleton height={`var(--text-${type}-size)`} width={width} />
    </VStack>
  );
}

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
export const PhoneBarSilhouette = ({ shown }: { shown: boolean }) => (
  <HStack
    gap={2}
    vAlign="center"
    style={{ visibility: shown ? "visible" : "hidden" }}
  >
    <VStack style={{ flex: 1 }}>
      <Line type="large" width="4rem" />
    </VStack>
    <Control size="lg" width="var(--size-element-lg)" />
    <Control size="lg" width="8rem" />
  </HStack>
);

export function DetailSilhouette({
  shown,
  columns,
  isWide,
}: {
  shown: boolean;
  /** the info column's action rows exist from lg up (item-info.tsx) */
  isWide: boolean;
  /** the loaded dialog's columns (lib/grid) */
  columns: ReturnType<typeof remColumns>;
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
      }}
    >
      <VStack gap={3}>
        <AspectRatio ratio={1}>
          <Skeleton />
        </AspectRatio>
        <HStack gap={2} vAlign="center">
          <Control size="md" width="var(--size-element-md)" />
        </HStack>
      </VStack>
      <VStack gap={5}>
        <VStack gap={2}>
          <Control size="sm" width="12rem" />
          <VStack>
            <Line type={title} width="100%" />
            <Line type={title} width="55%" />
          </VStack>
          <Line type="body" width="3rem" />
          <Line type="large" width="6rem" />
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
