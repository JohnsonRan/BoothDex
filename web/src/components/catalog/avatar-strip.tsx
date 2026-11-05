"use client";
// Avatar picker (Carousel Snap template block): a snapping row of selectable cards with prev/next buttons.
// The first page load renders only the cards a wide screen shows; the rest join on the first pointer, touch,
// focus or scroll in the strip, before any of them can come into view.

import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Carousel, type CarouselHandle } from "@astryxdesign/core/Carousel";
import { Icon } from "@astryxdesign/core/Icon";
import { SelectableCard } from "@astryxdesign/core/SelectableCard";
import { VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { LayoutGrid } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useUi } from "@/components/providers";
import { type Avatar, avatarName, imageSrc } from "@/lib/catalog";
import { useFilterUrl } from "./filter-url";

// structural width of one card; the cover keeps Booth's portrait thumbnails uncropped
const CARD_WIDTH = 104;
// cards rendered before the strip is used: more than fit across 1440 px (with the "all" card)
const FIRST_CARDS = 14;
const WAKE = ["pointerover", "touchstart", "focusin", "scroll"] as const;

export function AvatarStrip({
  avatars,
  value,
}: {
  avatars: Avatar[];
  value: string;
}) {
  const { t, ui } = useUi();
  const { apply } = useFilterUrl();
  const carousel = useRef<CarouselHandle>(null);
  const strip = useRef<HTMLDivElement>(null);
  const pick = (slug: string) => apply({ avatar: slug });

  // keep the chosen avatar in view, also when the page opens on one far down the list
  const index = avatars.findIndex((a) => a.slug === value) + 1;
  const [used, setUsed] = useState(false);
  const all = used || index >= FIRST_CARDS;
  useEffect(() => {
    if (index > 0) carousel.current?.scrollTo(index);
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
        <AspectRatio ratio={3 / 4} fit="center">
          <Icon icon={LayoutGrid} size="lg" color="secondary" />
        </AspectRatio>
      </Tile>
      {(all ? avatars : avatars.slice(0, FIRST_CARDS - 1)).map((a) => (
        <Tile
          key={a.slug}
          label={avatarName(a, ui)}
          selected={a.slug === value}
          onSelect={() => pick(a.slug === value ? "" : a.slug)}
        >
          <AspectRatio ratio={3 / 4} fit="cover">
            {a.image_url ? (
              <img
                src={imageSrc(a.image_url, "300x300_a2_g5")}
                alt=""
                loading="lazy"
                decoding="async"
              />
            ) : (
              <Icon icon={LayoutGrid} size="lg" color="secondary" />
            )}
          </AspectRatio>
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
    <SelectableCard
      label={label}
      isSelected={selected}
      onChange={onSelect}
      padding={1}
      width={CARD_WIDTH}
    >
      <VStack gap={1}>
        {children}
        <Text
          type="supporting"
          maxLines={1}
          justify="center"
          hasTruncateTooltip={false}
        >
          {label}
        </Text>
      </VStack>
    </SelectableCard>
  );
}
