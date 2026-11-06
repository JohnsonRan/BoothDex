"use client";
// Avatar picker (Carousel Snap template block): a snapping row of selectable cards with prev/next buttons and
// the Carousel's own edge fade. Each card is Product Detail's selectable image thumbnail as is (a transparent
// SelectableCard filling a square AspectRatio; its corner radius and selection ring are the card's own, the
// image only fills and covers, inset by the ring's width so the ring shows); the name sits under it as the caption of the Top Nav template's
// category tiles.
// A mouse wheel over the strip scrolls it sideways while it can move that way, as on the old strip; the
// Carousel maps only Shift + wheel itself (Carousel.d.ts:5-8).
// The first page load renders only the cards a wide screen shows; the rest join on the first pointer, touch,
// focus or scroll in the strip, before any of them can come into view.

import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Carousel, type CarouselHandle } from "@astryxdesign/core/Carousel";
import { Icon } from "@astryxdesign/core/Icon";
import { SelectableCard } from "@astryxdesign/core/SelectableCard";
import { VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { useEffect, useRef, useState } from "react";
import { useUi } from "@/components/providers";
import { type Avatar, avatarName, imageSrc } from "@/lib/catalog";
import { useFilterUrl } from "./filter-url";
import { SkeletonImage, THUMB_RADIUS } from "./silhouette";

// structural width of one card (104 px at a 16 px root); Booth's avatar thumbnails are square
const CARD_WIDTH = "6.5rem";
// cards rendered before the strip is used: more than fit across 1440 px (with the "all" card)
const FIRST_CARDS = 14;
const WAKE = ["pointerover", "touchstart", "focusin", "scroll"] as const;

const AllGlyph = () => (
  <VStack height="100%" hAlign="center" vAlign="center">
    <Icon icon="site:allAvatars" size="lg" color="secondary" />
  </VStack>
);

export function AvatarStrip({ avatars }: { avatars: Avatar[] }) {
  const { t, ui } = useUi();
  const { filters, apply } = useFilterUrl();
  // the picked avatar shows as selected at once, before its results arrive
  const value = filters.avatar;
  const carousel = useRef<CarouselHandle>(null);
  const strip = useRef<HTMLDivElement>(null);
  const pick = (slug: string) => apply({ avatar: slug });

  // keep the chosen avatar in view, also when the page opens on one far down the list. The handle's scrollTo
  // brings a card to the start edge (CarouselHandle.scrollTo, Carousel.d.ts:41-46), where the "previous" button
  // covers it, so the card before it goes there and the chosen one sits whole beside the button.
  const index = avatars.findIndex((a) => a.slug === value) + 1;
  const [used, setUsed] = useState(false);
  const all = used || index >= FIRST_CARDS;
  useEffect(() => {
    if (index > 0) carousel.current?.scrollTo(index - 1);
  }, [index]);
  useEffect(() => {
    const el = strip.current;
    if (all || !el) return;
    const wake = () => setUsed(true);
    for (const e of WAKE)
      el.addEventListener(e, wake, {
        once: true,
        passive: true,
        capture: true,
      });
    return () => {
      for (const e of WAKE) el.removeEventListener(e, wake, { capture: true });
    };
  }, [all]);
  // non-passive, so the page stays put; trackpad sideways deltas and Shift + wheel stay the Carousel's. The
  // Carousel exposes no scroller, only its edge queries, so the scroll goes to its documented scroll-area
  // group, by the same scrollBy its Shift + wheel uses, which keeps the mandatory snap stepping a card at a time.
  useEffect(() => {
    const el = strip.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      const c = carousel.current;
      if (e.shiftKey || Math.abs(e.deltaY) <= Math.abs(e.deltaX) || !c) return;
      if (!(e.deltaY > 0 ? c.canScrollNext() : c.canScrollPrev())) return;
      e.preventDefault();
      el.querySelector(':scope > [role="group"]')?.scrollBy({
        left: e.deltaY,
        behavior: "auto",
      });
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, []);

  return (
    <Carousel
      gap={2}
      hasSnap
      handleRef={carousel}
      ref={strip}
      aria-label={t("strip.label")}
    >
      <Tile
        label={t("strip.all")}
        selected={!value}
        onSelect={() => pick("")}
        hasImage={false}
      >
        <AllGlyph />
      </Tile>
      {(all ? avatars : avatars.slice(0, FIRST_CARDS - 1)).map((a) => (
        <Tile
          key={a.slug}
          label={avatarName(a, ui)}
          selected={a.slug === value}
          onSelect={() => pick(a.slug === value ? "" : a.slug)}
          hasImage={!!a.image_url}
        >
          {a.image_url ? (
            <SkeletonImage
              radius={THUMB_RADIUS}
              src={imageSrc(a.image_url, "300x300_a2_g5")}
              loading="lazy"
              decoding="async"
            />
          ) : (
            <AllGlyph />
          )}
        </Tile>
      ))}
    </Carousel>
  );
}

function Tile({
  label,
  selected,
  onSelect,
  hasImage,
  children,
}: {
  label: string;
  selected: boolean;
  onSelect: () => void;
  /** a glyph instead of a picture sits on the muted card surface, so the card still reads as one */
  hasImage: boolean;
  children: React.ReactNode;
}) {
  return (
    <VStack gap={2} width={CARD_WIDTH}>
      <AspectRatio ratio={1}>
        <SelectableCard
          label={label}
          isSelected={selected}
          onChange={onSelect}
          variant={hasImage ? "transparent" : "muted"}
          padding={0.5}
          width="100%"
          height="100%"
        >
          {children}
        </SelectableCard>
      </AspectRatio>
      <Text
        type="supporting"
        maxLines={1}
        justify="center"
        hasTruncateTooltip={false}
      >
        {label}
      </Text>
    </VStack>
  );
}
