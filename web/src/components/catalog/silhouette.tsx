// Loading silhouettes of text, of a result tile and under an image. Each line box is the real text's line box (its size times
// its leading), and the tile silhouette has the tile's parts in the tile's order (item-tile.tsx), so nothing
// moves when the real content replaces it (`tools/loading-layout.cjs` checks this). Shared by the server's
// loading states (loading.tsx) and LaterTiles.

import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Card } from "@astryxdesign/core/Card";
import { Skeleton } from "@astryxdesign/core/Skeleton";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import type { ComponentProps, Ref } from "react";

type TextType = "heading-1" | "heading-4" | "supporting" | "body" | "large";

/** `lines` lines of `type` text, each a bar of the font's size centred in the text's line box. */
export function TextSilhouette({
  type,
  lines = 1,
  width,
}: {
  type: TextType;
  lines?: number;
  width: string;
}) {
  return Array.from({ length: lines }, (_, i) => (
    <VStack
      // biome-ignore lint/suspicious/noArrayIndexKey: fixed lines, never reordered
      key={i}
      style={{
        height: `calc(var(--text-${type}-leading) * var(--text-${type}-size))`,
        justifyContent: "center",
      }}
    >
      <Skeleton
        height={`var(--text-${type}-size)`}
        width={i < lines - 1 ? "100%" : width}
      />
    </VStack>
  ));
}

/** One result tile: square cover, two title lines, shop, price, badge row (item-tile.tsx). */
export function TileSilhouette({ ref }: { ref?: Ref<HTMLDivElement> }) {
  return (
    <VStack ref={ref} gap={3} aria-hidden>
      <Card padding={0}>
        <AspectRatio ratio={1}>
          <Skeleton radius="none" />
        </AspectRatio>
      </Card>
      <VStack gap={1}>
        <VStack>
          <TextSilhouette type="heading-4" lines={2} width="60%" />
        </VStack>
        <HStack gap={1.5} vAlign="center">
          <Skeleton
            height="var(--spacing-5)"
            width="var(--spacing-5)"
            radius="rounded"
          />
          <StackItem size="fill">
            <TextSilhouette type="body" width="50%" />
          </StackItem>
        </HStack>
        <HStack
          vAlign="center"
          style={{ minBlockSize: "var(--size-element-sm)" }}
        >
          <StackItem size="fill">
            <TextSilhouette type="large" width="35%" />
          </StackItem>
        </HStack>
        <HStack style={{ minBlockSize: "var(--spacing-5)" }}>
          <Skeleton height="var(--spacing-5)" width="40%" radius="rounded" />
        </HStack>
      </VStack>
    </VStack>
  );
}

/** An image filling and covering its box over a Skeleton, which shows until the image paints over it: no
 *  handler and no state, so server markup needs no script. `radius` rounds both. */
export function SkeletonImage({
  radius,
  style,
  alt = "",
  ...img
}: ComponentProps<"img"> & { radius?: string }) {
  return (
    <span
      style={{
        display: "grid",
        width: "100%",
        height: "100%",
        borderRadius: radius,
        overflow: "hidden",
      }}
    >
      <Skeleton radius="none" style={{ gridArea: "1 / 1" }} />
      <img
        alt={alt}
        {...img}
        style={{
          gridArea: "1 / 1",
          width: "100%",
          height: "100%",
          objectFit: "cover",
          display: "block",
          ...style,
        }}
      />
    </span>
  );
}
