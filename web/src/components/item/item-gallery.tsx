"use client";
// The gallery column, from the product-detail template's ImageGallery (page.tsx:116-153): the hero image in an
// AspectRatio above a Grid of thumbnails, each thumbnail a transparent SelectableCard filling a square
// AspectRatio (page.tsx:131-150), with the avatar strip's padding so the image leaves the selection ring
// visible (catalog/silhouette.tsx, THUMB_PADDING). Master's gallery features on top of it:
// - the hero is a snapping Carousel (block CarouselSnap) without its own buttons, which float in the top layer
//   over the page behind the dialog: swipe on touch, ←/→ keys while the dialog is open;
// - a click on the image opens it full size in a Lightbox gallery (block LightboxGallery, with zoom);
// - under it, download (DropdownMenu of formats), then previous, the counter "n / N" and next, ghost
//   IconButtons like download's.

import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Carousel, type CarouselHandle } from "@astryxdesign/core/Carousel";
import { ClickableCard } from "@astryxdesign/core/ClickableCard";
import { Grid } from "@astryxdesign/core/Grid";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { Lightbox } from "@astryxdesign/core/Lightbox";
import { SelectableCard } from "@astryxdesign/core/SelectableCard";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { useEffect, useRef, useState } from "react";
import {
  SkeletonImage,
  THUMB_PADDING,
  THUMB_RADIUS,
} from "@/components/catalog/silhouette";
import { useUi } from "@/components/providers";
import type { ItemResponse } from "@/lib/api-types";
import { imageCount, imgUrl } from "@/lib/catalog";
import { remColumns } from "@/lib/grid";
import { HERO, STICKY } from "./item-data";
import { Download } from "./item-download";

// the thumbnails' floor width: master's 64 px grid cells, in rem
const THUMBS = remColumns("4rem");
// about three rows of thumbnails at the dialog's width
const THUMBS_HEIGHT = "15rem";
// a slide is the current one once most of it is in view
const SEEN = 0.6;

export function ItemGallery({
  item,
  isWide,
  isOpen,
}: {
  item: ItemResponse;
  /** from lg up: thumbnails, arrow buttons and the sticky column (phones swipe, master) */
  isWide: boolean;
  /** the dialog is open; a closed one keeps its content, so the keys must leave the page alone */
  isOpen: boolean;
}) {
  const { t } = useUi();
  const count = Math.max(imageCount(item), 1);
  const [index, setIndex] = useState(0);
  const [zoom, setZoom] = useState(false);
  // the first image once it has loaded, which then needs no cover behind it
  const hero = imgUrl(item, 0, "orig");
  const [loaded, setLoaded] = useState("");
  const carousel = useRef<CarouselHandle>(null);
  const strip = useRef<HTMLDivElement>(null);

  const go = (i: number) => {
    const n = (i + count) % count;
    carousel.current?.scrollTo(n);
    setIndex(n);
  };

  // the current slide follows swipes; observed again when the whole gallery
  // arrives after the first images
  // biome-ignore lint/correctness/useExhaustiveDependencies: count changes the slides observed
  useEffect(() => {
    const root = strip.current;
    if (!root) return;
    const seen = new IntersectionObserver(
      (entries) => {
        for (const e of entries)
          if (e.isIntersecting)
            setIndex(Number((e.target as HTMLElement).dataset.slide));
      },
      { root, threshold: SEEN },
    );
    for (const el of root.querySelectorAll("[data-slide]")) seen.observe(el);
    return () => seen.disconnect();
  }, [count]);

  // ←/→ page the images; not while typing, in a radio group (the language tabs use arrows), or zoomed (the
  // Lightbox pages itself)
  const keys = useRef((_: KeyboardEvent) => {});
  keys.current = (e) => {
    if (zoom || count < 2) return;
    if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
    const el = e.target instanceof Element ? e.target : null;
    if (el?.closest("input, textarea, [role=radiogroup], [role=menu]")) return;
    e.preventDefault();
    go(index + (e.key === "ArrowRight" ? 1 : -1));
  };
  useEffect(() => {
    if (!isOpen) return;
    const key = (e: KeyboardEvent) => keys.current(e);
    addEventListener("keydown", key);
    return () => removeEventListener("keydown", key);
  }, [isOpen]);

  const images = Array.from({ length: count }, (_, i) => i);
  const alt = (i: number) => t("detail.image", { n: i + 1 });

  return (
    <VStack
      gap={3}
      style={{ ...(isWide && STICKY), containerType: "inline-size" }}
    >
      <Carousel
        gap={0}
        hasSnap
        hasButtons={false}
        hasEdgeFade={false}
        handleRef={carousel}
        ref={strip}
        aria-label={t("detail.label")}
      >
        {images.map((i) => (
          // one slide is the gallery's width (the Carousel sizes slides by their content)
          <VStack key={i} data-slide={String(i)} width="100cqi">
            <AspectRatio ratio={1}>
              <ClickableCard
                label={alt(i)}
                variant="muted"
                padding={0}
                width="100%"
                height="100%"
                onClick={() => {
                  setIndex(i);
                  setZoom(true);
                }}
              >
                {/* The original (1024 px at most): Booth's 620 px variant is stretched by any screen denser than
                    1x, and the Lightbox below asks for the original of the current image even while closed, so
                    one file serves both. The first image is what a shared link paints largest: asked for ahead
                    of the grid behind. */}
                {/* The first one shows the tile's cover behind it until it has loaded: the browser has that
                    copy, and the card drew it while the record loaded (item-silhouette.tsx). */}
                <img
                  style={
                    i === 0 && loaded !== hero
                      ? {
                          ...HERO,
                          background: `url("${imgUrl(item, 0)}") center / contain no-repeat`,
                        }
                      : HERO
                  }
                  onLoad={i === 0 ? () => setLoaded(hero) : undefined}
                  src={imgUrl(item, i, "orig")}
                  alt={alt(i)}
                  loading={i < 2 ? "eager" : "lazy"}
                  fetchPriority={i === 0 ? "high" : undefined}
                  decoding="async"
                />
              </ClickableCard>
            </AspectRatio>
          </VStack>
        ))}
      </Carousel>
      <HStack gap={2} vAlign="center">
        <Download item={item} index={index} />
        <StackItem size="fill" />
        {count > 1 && (
          <>
            {isWide && (
              <IconButton
                label={t("detail.prev")}
                tooltip={t("detail.prev")}
                variant="ghost"
                icon={<Icon icon="chevronLeft" size="sm" />}
                onClick={() => go(index - 1)}
              />
            )}
            <Text type="supporting" color="secondary" hasTabularNumbers>
              {`${index + 1} / ${count}`}
            </Text>
            {isWide && (
              <IconButton
                label={t("detail.next")}
                tooltip={t("detail.next")}
                variant="ghost"
                icon={<Icon icon="chevronRight" size="sm" />}
                onClick={() => go(index + 1)}
              />
            )}
          </>
        )}
      </HStack>
      {isWide && count > 1 && (
        // a long gallery scrolls in place (master: max-h-40), so the sticky column keeps the hero in view
        <VStack
          style={{
            maxBlockSize: THUMBS_HEIGHT,
            overflowY: "auto",
            // no bar beside the thumbnails, as in the popup lists (globals.css); the cut-off row shows there is more
            scrollbarWidth: "none",
            padding: "var(--spacing-1)",
            margin: "calc(-1 * var(--spacing-1))",
          }}
        >
          <Grid {...THUMBS} gap={2}>
            {images.map((i) => (
              <AspectRatio key={i} ratio={1}>
                <SelectableCard
                  label={alt(i)}
                  isSelected={index === i}
                  onChange={() => go(i)}
                  variant="transparent"
                  padding={THUMB_PADDING}
                  width="100%"
                  height="100%"
                >
                  {/* template thumbImage, over its loading Skeleton, inside the selection ring */}
                  <SkeletonImage
                    radius={THUMB_RADIUS}
                    src={imgUrl(item, i, "300x300_a2_g5")}
                    loading="lazy"
                    decoding="async"
                  />
                </SelectableCard>
              </AspectRatio>
            ))}
          </Grid>
        </VStack>
      )}
      <Lightbox
        isOpen={zoom}
        onOpenChange={setZoom}
        media={images.map((i) => ({
          src: imgUrl(item, i, "orig"),
          alt: alt(i),
        }))}
        index={index}
        onIndexChange={go}
        hasZoom
      />
    </VStack>
  );
}
