"use client";
// The gallery column, from the product-detail template's ImageGallery (page.tsx:116-153): the hero image in an
// AspectRatio above a Grid of thumbnails, each thumbnail a transparent SelectableCard with padding 0 filling a
// square AspectRatio (page.tsx:131-150). Master's gallery features on top of it:
// - the hero is a snapping Carousel (block CarouselSnap): swipe on touch, the Carousel's own prev/next buttons
//   elsewhere, ←/→ keys while the dialog is open;
// - a click on the image opens it full size in a Lightbox gallery (block LightboxGallery, with zoom);
// - under it, download (DropdownMenu of formats) and the counter "n / N".

import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Carousel, type CarouselHandle } from "@astryxdesign/core/Carousel";
import { ClickableCard } from "@astryxdesign/core/ClickableCard";
import {
  DropdownMenu,
  DropdownMenuGroup,
  DropdownMenuItem,
} from "@astryxdesign/core/DropdownMenu";
import { Grid } from "@astryxdesign/core/Grid";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { Lightbox } from "@astryxdesign/core/Lightbox";
import { SelectableCard } from "@astryxdesign/core/SelectableCard";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { useToast } from "@astryxdesign/core/Toast";
import { type CSSProperties, useEffect, useRef, useState } from "react";
import { useUi } from "@/components/providers";
import type { ItemResponse } from "@/lib/api-types";
import { imageCount, imgUrl } from "@/lib/catalog";
import {
  downloadFormats,
  downloadName,
  downloadUrl,
  type Format,
  STICKY,
} from "./item-data";

// template heroImage, but contained: a product photo is never cropped (the card's muted surface shows around a
// non-square one)
const HERO: CSSProperties = {
  width: "100%",
  height: "100%",
  objectFit: "contain",
  display: "block",
};
// template thumbImage
const THUMB: CSSProperties = {
  width: "100%",
  height: "100%",
  objectFit: "cover",
  display: "block",
};
// the thumbnails' floor width: master's 64 px grid cells
const THUMBS = { minWidth: 64, repeat: "fill" } as const;
// a slide is the current one once most of it is in view
const SEEN = 0.6;

export function ItemGallery({ item }: { item: ItemResponse }) {
  const { t } = useUi();
  const count = Math.max(imageCount(item), 1);
  const [index, setIndex] = useState(0);
  const [zoom, setZoom] = useState(false);
  const carousel = useRef<CarouselHandle>(null);
  const strip = useRef<HTMLDivElement>(null);

  const go = (i: number) => {
    const n = (i + count) % count;
    carousel.current?.scrollTo(n);
    setIndex(n);
  };

  // the current slide follows swipes and the Carousel's buttons; observed again when the whole gallery
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
    const key = (e: KeyboardEvent) => keys.current(e);
    addEventListener("keydown", key);
    return () => removeEventListener("keydown", key);
  }, []);

  const images = Array.from({ length: count }, (_, i) => i);
  const alt = (i: number) => t("detail.image", { n: i + 1 });

  return (
    <VStack gap={3} style={STICKY}>
      <Carousel
        gap={0}
        hasSnap
        hasEdgeFade={false}
        handleRef={carousel}
        ref={strip}
        aria-label={t("detail.label")}
      >
        {images.map((i) => (
          <VStack
            key={i}
            data-slide={String(i)}
            width="100%"
            style={{ flex: "none" }}
          >
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
                <img
                  style={HERO}
                  src={imgUrl(item, i, "620x620")}
                  alt={alt(i)}
                  loading={i < 2 ? "eager" : "lazy"}
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
          <Text type="supporting" color="secondary" hasTabularNumbers>
            {`${index + 1} / ${count}`}
          </Text>
        )}
      </HStack>
      {count > 1 && (
        <Grid columns={THUMBS} gap={2}>
          {images.map((i) => (
            <AspectRatio key={i} ratio={1}>
              <SelectableCard
                label={alt(i)}
                isSelected={index === i}
                onChange={() => go(i)}
                variant="transparent"
                padding={0}
                width="100%"
                height="100%"
              >
                <img
                  style={THUMB}
                  src={imgUrl(item, i, "300x300_a2_g5")}
                  alt=""
                  loading="lazy"
                  decoding="async"
                />
              </SelectableCard>
            </AspectRatio>
          ))}
        </Grid>
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

/** The current image at full size, re-encoded by the image proxy (master's DownloadMenu). */
function Download({ item, index }: { item: ItemResponse; index: number }) {
  const { t } = useUi();
  const toast = useToast();
  const orig = imgUrl(item, index, "orig");
  if (!orig) return null;
  const save = async (format: Format) => {
    try {
      const res = await fetch(downloadUrl(orig, format));
      if (!res.ok) throw new Error(String(res.status));
      const a = document.createElement("a");
      a.href = URL.createObjectURL(await res.blob());
      a.download = downloadName(item.id, index, format);
      a.click();
      setTimeout(() => URL.revokeObjectURL(a.href), 10_000);
    } catch {
      toast({
        body: t("list.loadFailed"),
        type: "error",
        uniqueID: "download",
      });
    }
  };
  return (
    <DropdownMenu
      placement="below"
      alignment="start"
      renderTrigger={(props) => (
        <IconButton
          {...props}
          label={t("detail.download")}
          tooltip={t("detail.download")}
          variant="ghost"
          icon={<Icon icon="site:download" size="sm" />}
        />
      )}
    >
      <DropdownMenuGroup title={t("detail.download")}>
        {downloadFormats(orig).map((f) => (
          <DropdownMenuItem
            key={f}
            label={t("detail.downloadAs", { format: f.toUpperCase() })}
            onClick={() => save(f)}
          />
        ))}
      </DropdownMenuGroup>
    </DropdownMenu>
  );
}
