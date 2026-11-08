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
// Compact (folded into the filter band, band.tsx), each card is 40 px, Avatar's size scale, with its name beside
// it; a chevron after the row folds or unfolds it, pointing as Collapsible's (down to unfold, up to fold).

import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Carousel, type CarouselHandle } from "@astryxdesign/core/Carousel";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { SelectableCard } from "@astryxdesign/core/SelectableCard";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { useEffect, useRef, useState } from "react";
import { useUi } from "@/components/providers";
import { type Avatar, avatarName, imageSrc } from "@/lib/catalog";
import { useFilterUrl } from "./filter-url";
import { SkeletonImage, THUMB_RADIUS } from "./silhouette";

// structural width of one card (104 px at a 16 px root, 40 px compact); Booth's avatar thumbnails are square
const CARD_WIDTH = "6.5rem";
const COMPACT_WIDTH = "2.5rem";
// cards rendered before the strip is used: more than fit across 1440 px (with the "all" card)
const FIRST_CARDS = 14;
const WAKE = ["pointerover", "touchstart", "focusin", "scroll"] as const;
// A touch screen swipes the strip, so it gets no prev/next buttons. They also break iOS Safari 26's toolbar: they
// sit in an always-open fixed top-layer popover, which Safari's toolbar sampling finds at the bottom edge
// (WebKit LocalFrameView::fixedContainerEdges), so it painted the toolbar solid instead of letting the page show
// through. Without buttons the Carousel hides that layer (display: none), which the sampling skips.
const TOUCH = "(hover: none) and (pointer: coarse)";

const AllGlyph = () => (
  <VStack height="100%" hAlign="center" vAlign="center">
    <Icon icon="site:allAvatars" size="lg" color="secondary" />
  </VStack>
);

export function AvatarStrip({
  avatars,
  compact,
  controls,
  onToggle,
  onPick,
}: {
  avatars: Avatar[];
  compact: boolean;
  /** the id of the strip at full size, which the chevron shows or hides */
  controls: string;
  onToggle: () => void;
  onPick: () => void;
}) {
  const { t, ui } = useUi();
  const { filters, apply } = useFilterUrl();
  // the picked avatar shows as selected at once, before its results arrive
  const value = filters.avatar;
  const carousel = useRef<CarouselHandle>(null);
  const strip = useRef<HTMLDivElement>(null);
  const pick = (slug: string) => {
    onPick();
    apply({ avatar: slug });
  };
  const touch = useMediaQuery(TOUCH);

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
    <HStack gap={2} vAlign="center">
      <StackItem size="fill">
        <Carousel
          gap={2}
          hasSnap
          hasButtons={!touch}
          handleRef={carousel}
          ref={strip}
          aria-label={t("strip.label")}
        >
          <Tile
            label={t("strip.all")}
            selected={!value}
            onSelect={() => pick("")}
            hasImage={false}
            compact={compact}
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
              compact={compact}
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
      </StackItem>
      <IconButton
        label={t(compact ? "strip.expand" : "strip.collapse")}
        icon={
          // Collapsible's chevron: down while folded, turned to point up while open
          <Icon
            icon="chevronDown"
            size="sm"
            style={compact ? undefined : { transform: "rotate(180deg)" }}
          />
        }
        variant="ghost"
        size="sm"
        aria-expanded={!compact}
        aria-controls={controls}
        onClick={onToggle}
      />
    </HStack>
  );
}

function Tile({
  label,
  selected,
  onSelect,
  hasImage,
  compact,
  children,
}: {
  label: string;
  selected: boolean;
  onSelect: () => void;
  /** a glyph instead of a picture sits on the muted card surface, so the card still reads as one */
  hasImage: boolean;
  compact: boolean;
  children: React.ReactNode;
}) {
  const card = (
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
  );
  return compact ? (
    <HStack gap={2} vAlign="center" maxWidth="10rem">
      <VStack width={COMPACT_WIDTH}>{card}</VStack>
      <StackItem size="fill">
        <Text type="supporting" maxLines={1} hasTruncateTooltip={false}>
          {label}
        </Text>
      </StackItem>
    </HStack>
  ) : (
    <VStack gap={2} width={CARD_WIDTH}>
      {card}
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
