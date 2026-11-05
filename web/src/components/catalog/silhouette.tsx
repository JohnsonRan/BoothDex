// Loading silhouettes of text and of a result tile. Each line box is the real text's line box (its size times
// its leading), and the tile silhouette has the tile's parts in the tile's order (item-tile.tsx), so nothing
// moves when the real content replaces it (`tools/loading-layout.cjs` checks this). Shared by the server's
// loading states (loading.tsx) and LaterTiles.

import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Card } from "@astryxdesign/core/Card";
import { Skeleton } from "@astryxdesign/core/Skeleton";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import type { Ref } from "react";

type TextType = "heading-1" | "heading-4" | "supporting" | "body";

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
    <VStack ref={ref} gap={2} aria-hidden>
      <Card padding={0}>
        <AspectRatio ratio={1}>
          <Skeleton radius="none" />
        </AspectRatio>
      </Card>
      <VStack gap={1}>
        <VStack>
          <TextSilhouette type="heading-4" lines={2} width="60%" />
        </VStack>
        <TextSilhouette type="supporting" width="50%" />
        <TextSilhouette type="body" width="35%" />
        <HStack style={{ minBlockSize: "var(--spacing-5)" }}>
          <Skeleton height="var(--spacing-5)" width="40%" radius="rounded" />
        </HStack>
      </VStack>
    </VStack>
  );
}
