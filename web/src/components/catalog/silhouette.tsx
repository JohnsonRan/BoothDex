// Loading silhouettes of text, of a result tile and under an image. Each line box is the real text's line box (its size times
// its leading), and the tile silhouette has the tile's parts in the tile's order (item-tile.tsx), so nothing
// moves when the real content replaces it (`tools/loading-layout.cjs` checks this). Shared by the server's
// loading states (loading.tsx), the results' next page (result-pages.tsx) and LaterTiles.

import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Card } from "@astryxdesign/core/Card";
import { Skeleton } from "@astryxdesign/core/Skeleton";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import type { ComponentProps, CSSProperties, Ref } from "react";

type TextType =
  | "display-2"
  | "heading-1"
  | "heading-4"
  | "supporting"
  | "body"
  | "large";

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
      height={`calc(var(--text-${type}-leading) * var(--text-${type}-size))`}
      vAlign="center"
    >
      <Skeleton
        height={`var(--text-${type}-size)`}
        width={i < lines - 1 ? "100%" : width}
      />
    </VStack>
  ));
}

/** One result tile: square cover, two title lines, shop, price (item-tile.tsx; its badges sit on the cover). */
export function TileSilhouette({
  ref,
  style,
}: {
  ref?: Ref<HTMLDivElement>;
  /** where it stands out of the flow (result-pages.tsx: the boxes' mask probe) */
  style?: CSSProperties;
}) {
  return (
    <VStack ref={ref} gap={3} aria-hidden style={style}>
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
        <HStack vAlign="center" minHeight="var(--size-element-sm)">
          <StackItem size="fill">
            <TextSilhouette type="large" width="35%" />
          </StackItem>
        </HStack>
      </VStack>
    </VStack>
  );
}

/**
 * One tile's silhouette as a mask for a box of tiles not drawn (result-pages.tsx, app/catalog.css `.page-box`):
 * each Skeleton of a laid-out TileSilhouette as a rect at its place (the cover with its card's corners), in an
 * image of the tile's column width and row pitch that the box repeats across its columns and down its rows. The
 * box shows the tiles it stands for with no element per tile and nothing running.
 */
export function silhouetteMask(
  tile: HTMLElement,
  grid: HTMLElement,
): CSSProperties {
  const box = tile.getBoundingClientRect();
  const n = (v: number) => Math.round(v * 100) / 100;
  const rects = [...tile.querySelectorAll<HTMLElement>(".astryx-skeleton")].map(
    (s, i) => {
      const r = s.getBoundingClientRect();
      const corner = getComputedStyle(
        (i === 0 && tile.firstElementChild) || s,
      ).borderTopLeftRadius;
      const rx = Math.min(
        Number.parseFloat(corner) || 0,
        r.width / 2,
        r.height / 2,
      );
      return `<rect x="${n(r.left - box.left)}" y="${n(r.top - box.top)}" width="${n(r.width)}" height="${n(r.height)}" rx="${n(rx)}"/>`;
    },
  );
  const w = n(box.width);
  const h = n(box.height + Number.parseFloat(getComputedStyle(grid).rowGap));
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}">${rects.join("")}</svg>`;
  return {
    "--_tile": `url("data:image/svg+xml,${encodeURIComponent(svg)}")`,
    "--_tile-size": `${w}px ${h}px`,
  } as CSSProperties;
}

// Product Detail's thumbImage in a selectable card (avatar strip, item gallery), inset by the card's padding: the
// card draws its selection ring as an inset shadow (2 px), which an edge-to-edge image covers, so the image leaves
// the ring a frame. The padding is twice the ring's width, so a band of the page's surface (the card is
// transparent) parts the ring from the image: flush against it, the ring vanished on an image whose edge is its
// colour, as a light avatar under dark mode's near-white ring (owner). The corner stays concentric with the card:
// its radius minus that padding.
export const THUMB_PADDING = 1;
export const THUMB_RADIUS = "calc(var(--radius-container) - var(--spacing-1))";

/** An image filling and covering its box over a Skeleton, which shows until the image paints over it: no
 *  handler and no state, so server markup needs no script. `radius` rounds both. The image is positioned so it
 *  paints above the Skeleton, whose animation makes it a layer of its own; once the image can paint, the Skeleton
 *  goes (SKELETON_PAINTED). */
export function SkeletonImage({
  radius,
  alt = "",
  ...img
}: Omit<ComponentProps<"img">, "style"> & { radius?: string }) {
  return (
    <span
      data-skeleton=""
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
        // a new image is a new element: the old one's data-painted would hide the Skeleton while this one loads
        key={String(img.src)}
        alt={alt}
        {...img}
        style={{
          gridArea: "1 / 1",
          position: "relative",
          width: "100%",
          height: "100%",
          objectFit: "cover",
          display: "block",
        }}
      />
    </span>
  );
}

/**
 * Marks an image over a Skeleton once it is decoded and can paint, and the Skeleton under it is dropped
 * (app/catalog.css). A running Skeleton is a compositor layer, and so is every image over one: about 300 on a
 * screen of results, whose layerizing took most of every scrolled frame. One listener for the whole document, in
 * the head (app/layout.tsx) before any image; a load does not bubble, so it listens in the capture phase. An image
 * that fails to decode keeps its Skeleton.
 */
export const SKELETON_PAINTED = `;document.addEventListener("load",function(e){var t=e.target,p=t.parentNode;
if(t.tagName!=="IMG"||!p||!p.hasAttribute||!p.hasAttribute("data-skeleton"))return;
function f(){t.setAttribute("data-painted","")}t.decode?t.decode().then(f,function(){}):f()},true);`;
