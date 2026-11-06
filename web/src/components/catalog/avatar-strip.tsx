"use client";
// Avatar picker (Carousel Snap template block): a snapping row of selectable cards with prev/next buttons and
// the Carousel's own edge fade. Each card is Product Detail's selectable image thumbnail as is (a transparent
// SelectableCard filling a square AspectRatio; its corner radius and selection ring are the card's own, the
// image only fills and covers, inset by the ring's width so the ring shows); the name sits under it as the caption of the Top Nav template's
// category tiles.
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

  return (
    <Carousel
      gap={2}
      hasSnap
      handleRef={carousel}
      ref={strip}
      aria-label={t("strip.label")}
    >
      <Tile label={t("strip.all")} selected={!value} onSelect={() => pick("")}>
        <AllGlyph />
      </Tile>
      {(all ? avatars : avatars.slice(0, FIRST_CARDS - 1)).map((a) => (
        <Tile
          key={a.slug}
          label={avatarName(a, ui)}
          selected={a.slug === value}
          onSelect={() => pick(a.slug === value ? "" : a.slug)}
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
  children,
}: {
  label: string;
  selected: boolean;
  onSelect: () => void;
  children: React.ReactNode;
}) {
  return (
    <VStack gap={2} width={CARD_WIDTH}>
      <AspectRatio ratio={1}>
        <SelectableCard
          label={label}
          isSelected={selected}
          onChange={onSelect}
          variant="transparent"
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
