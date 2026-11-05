"use client";
// "More from {shop}": the shop's 12 most liked other items in a snapping Carousel (block CarouselSnap), each
// card the avatar strip's pattern (avatar-strip.tsx): a square AspectRatio filled by a transparent card with
// padding 0 and a covering image, the caption below. A ClickableCard rather than a SelectableCard, since a
// card here opens an item instead of selecting one; the look is the same. Picking one shows it in place.
// The Carousel's own buttons live in the top layer and float over the page when the dialog scrolls the strip
// away, so from lg the heading row carries scroll buttons instead, with the Carousel's own labels.

import { AspectRatio } from "@astryxdesign/core/AspectRatio";
import { Carousel, type CarouselHandle } from "@astryxdesign/core/Carousel";
import { ClickableCard } from "@astryxdesign/core/ClickableCard";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { useTranslator } from "@astryxdesign/core/i18n";
import { Skeleton } from "@astryxdesign/core/Skeleton";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import { Heading, Text } from "@astryxdesign/core/Text";
import { type CSSProperties, useEffect, useRef, useState } from "react";
import { useFilterUrl } from "@/components/catalog/filter-url";
import { useUi } from "@/components/providers";
import { api } from "@/lib/api";
import type { ItemResponse, SearchResponse } from "@/lib/api-types";
import { imgUrl, priceOf } from "@/lib/catalog";
import { formatPrice, itemText, type Rates } from "@/lib/settings-core";
import { useDelayed } from "@/lib/use-delayed";
import { useItemOpener } from "./item-host";

// structural width of one card (master: 150 px on desktop)
const CARD_WIDTH = 150;
// master: up to 12, by likes
const SHOWN = 12;
// avatar strip THUMB
const THUMB: CSSProperties = {
  width: "100%",
  height: "100%",
  objectFit: "cover",
  display: "block",
};

export function MoreFrom({
  item,
  rates,
  isWide,
}: {
  item: ItemResponse;
  rates: Rates;
  /** scroll buttons from lg up; phones swipe */
  isWide: boolean;
}) {
  const { t, settings } = useUi();
  const { filters } = useFilterUrl();
  const { open } = useItemOpener();
  const [same, setSame] = useState<{ of: number; items: ItemResponse[] }>();
  const author = item.author_name;
  useEffect(() => {
    let live = true;
    api<SearchResponse>(
      `/api/search?authors=${encodeURIComponent(author)}&adult=${filters.adult}&sort=likes&pageSize=${SHOWN + 1}&fields=card`,
    )
      .then((r) => r.items.filter((x) => x.id !== item.id).slice(0, SHOWN))
      .catch(() => [])
      .then((items) => live && setSame({ of: item.id, items }));
    return () => {
      live = false;
    };
  }, [author, item.id, filters.adult]);
  const items = same?.of === item.id ? same.items : null;
  const shown = useDelayed(!items);
  const strip = useRef<CarouselHandle>(null);
  const library = useTranslator();

  return (
    <VStack gap={3}>
      <HStack gap={2} vAlign="center">
        <StackItem size="fill">
          <Heading level={3}>{t("detail.moreFrom", { name: author })}</Heading>
        </StackItem>
        {isWide &&
          !!items?.length &&
          (["Left", "Right"] as const).map((side) => (
            <IconButton
              key={side}
              label={library(`@astryx.carousel.scroll${side}`)}
              tooltip={library(`@astryx.carousel.scroll${side}`)}
              variant="ghost"
              icon={<Icon icon={`chevron${side}`} size="sm" />}
              onClick={() =>
                side === "Left"
                  ? strip.current?.scrollPrev()
                  : strip.current?.scrollNext()
              }
            />
          ))}
      </HStack>
      {items?.length === 0 ? (
        <Text type="body" color="secondary">
          {t("detail.noMore")}
        </Text>
      ) : (
        <Carousel
          gap={2}
          hasSnap
          hasButtons={false}
          handleRef={strip}
          aria-label={t("detail.moreFrom", { name: author })}
        >
          {items
            ? items.map((x) => (
                <VStack key={x.id} gap={2} width={CARD_WIDTH}>
                  <AspectRatio ratio={1}>
                    <ClickableCard
                      label={itemText(x, "title", settings.content)}
                      variant="transparent"
                      padding={0}
                      width="100%"
                      height="100%"
                      onClick={() => open(x.id)}
                    >
                      <img
                        style={THUMB}
                        src={imgUrl(x, 0)}
                        alt=""
                        loading="lazy"
                        decoding="async"
                      />
                    </ClickableCard>
                  </AspectRatio>
                  <Caption
                    title={itemText(x, "title", settings.content)}
                    price={formatPrice(priceOf(x), settings, rates, t)}
                  />
                </VStack>
              ))
            : Array.from({ length: 6 }, (_, i) => (
                <CardSilhouette
                  // biome-ignore lint/suspicious/noArrayIndexKey: fixed placeholders, never reordered
                  key={i}
                  shown={shown}
                  label={i === 0 ? t("detail.loading") : undefined}
                />
              ))}
        </Carousel>
      )}
    </VStack>
  );
}

/** Two lines of title, then the price: the same boxes in the card and its silhouette. */
function Caption({ title, price }: { title: string; price: string }) {
  return (
    <VStack gap={1}>
      <Text
        type="supporting"
        color="primary"
        maxLines={2}
        hasTruncateTooltip={false}
        style={{ minBlockSize: "2lh" }}
      >
        {title}
      </Text>
      <Text type="body" weight="bold" hasTabularNumbers>
        {price}
      </Text>
    </VStack>
  );
}

/** A card's boxes while the shop's items load; the bars appear only once the wait has lasted (use-delayed). */
function CardSilhouette({ shown, label }: { shown: boolean; label?: string }) {
  const bar = (type: "supporting" | "body", width: string) => (
    <VStack
      style={{
        height: `calc(var(--text-${type}-leading) * var(--text-${type}-size))`,
        justifyContent: "center",
      }}
    >
      <Skeleton height={`var(--text-${type}-size)`} width={width} />
    </VStack>
  );
  return (
    <VStack
      gap={2}
      width={CARD_WIDTH}
      aria-busy
      aria-label={label}
      style={{ visibility: shown ? "visible" : "hidden" }}
    >
      <AspectRatio ratio={1}>
        <Skeleton />
      </AspectRatio>
      <VStack gap={1}>
        <VStack>
          {bar("supporting", "100%")}
          {bar("supporting", "60%")}
        </VStack>
        {bar("body", "40%")}
      </VStack>
    </VStack>
  );
}
