"use client";
// The info column, from the product-detail template's ProductInfo (page.tsx:156-273), in its order: a VStack
// gap 5 holding the title block (gap 2: title as display-2, the rating row, the price row with its Badge),
// option controls, the two large action buttons, then the CollapsibleGroup of sections (item-sections.tsx).
// BoothDex fills it with master's details: the language tabs as the template's SegmentedControl, likes in the
// rating row, "listed at … why different?" in the price row, rating, features, Quest, Delisted and Relisted as
// Badges, the avatar-fit Banner, and Buy on Booth / favourite / copy link as the actions. Phones get the
// actions in the dialog's footer instead (PhoneBar).

import { Avatar } from "@astryxdesign/core/Avatar";
import { Badge } from "@astryxdesign/core/Badge";
import { Banner } from "@astryxdesign/core/Banner";
import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import {
  SegmentedControl,
  SegmentedControlItem,
} from "@astryxdesign/core/SegmentedControl";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { useToast } from "@astryxdesign/core/Toast";
import { Tooltip } from "@astryxdesign/core/Tooltip";
import { useEffect, useState } from "react";
import { useFilterUrl } from "@/components/catalog/filter-url";
import { useUi } from "@/components/providers";
import { api } from "@/lib/api";
import type { ContentRating, ItemResponse } from "@/lib/api-types";
import {
  type Avatar as AvatarRecord,
  avatarName,
  FEATURES,
  type Feature,
  imageSrc,
  priceOf,
} from "@/lib/catalog";
import {
  type ContentLang,
  formatPrice,
  itemText,
  LOCALES,
  type Rates,
} from "@/lib/settings-core";
import {
  boothUrl,
  initialLang,
  LANG_LABELS,
  langTabs,
  STICKY,
} from "./item-data";
import { useItemOpener } from "./item-host";
import { ItemSections } from "./item-sections";

const RATING: Record<ContentRating, "success" | "warning" | "error"> = {
  SFW: "success",
  Suggestive: "warning",
  NSFW: "error",
};

export function ItemInfo({
  item,
  rates,
  hasActions,
}: {
  item: ItemResponse;
  rates: Rates;
  /** the action buttons (from lg up; phones have PhoneBar) */
  hasActions: boolean;
}) {
  const { t, settings } = useUi();
  const [lang, setLang] = useState<ContentLang>(() =>
    initialLang(item, settings.content),
  );
  const tabs = langTabs(item);
  const money = (jpy: number | null) => formatPrice(jpy, settings, rates);
  const listed =
    item.smart_price != null && item.smart_price !== item.price_jpy;

  return (
    <VStack gap={5} style={STICKY}>
      <VStack gap={2}>
        {tabs.length > 1 && (
          <VStack hAlign="start">
            <SegmentedControl
              value={lang}
              onChange={(v) => setLang(v as ContentLang)}
              label={t("detail.language")}
              size="sm"
            >
              {tabs.map((l) => (
                <SegmentedControlItem
                  key={l}
                  value={l}
                  label={
                    l === "original" ? t("detail.original") : LANG_LABELS[l]
                  }
                />
              ))}
            </SegmentedControl>
          </VStack>
        )}
        <Text type="display-2" as="h1">
          {itemText(item, "title", lang)}
        </Text>
        {/* the template's rating row (StarRating, page.tsx:55-72) */}
        <HStack gap={1} vAlign="center">
          <Icon icon="site:favourite" size="sm" color="secondary" />
          <Text type="body" color="secondary" hasTabularNumbers>
            {item.likes.toLocaleString(LOCALES[settings.ui])}
          </Text>
        </HStack>
        <HStack gap={2} vAlign="center" wrap="wrap">
          <Text type="large" weight="bold" hasTabularNumbers>
            {money(priceOf(item))}
          </Text>
          {listed && (
            <Tooltip content={item.smart_price_reason ?? ""}>
              <Text type="body" color="secondary">
                {t("price.listed", { price: money(item.price_jpy) })}
              </Text>
            </Tooltip>
          )}
        </HStack>
      </VStack>
      <Marks item={item} />
      <AvatarFit item={item} />
      {hasActions && (
        <VStack gap={2}>
          <Button
            label={t("detail.buy")}
            variant="primary"
            size="lg"
            href={boothUrl(item.id)}
            target="_blank"
            endContent={<Icon icon="externalLink" />}
          />
          <HStack gap={2}>
            <StackItem size="fill">
              <Button
                label={t("fav.add")}
                size="lg"
                width="100%"
                // answered by fav-toast.tsx, as the tiles' buttons are
                data-fav=""
                icon={<Icon icon="site:favourite" />}
              />
            </StackItem>
            <StackItem size="fill">
              <CopyLink id={item.id} />
            </StackItem>
          </HStack>
        </VStack>
      )}
      <ItemSections item={item} lang={lang} money={money} />
    </VStack>
  );
}

/** Rating, features, Quest, Delisted, and the way to the relisted item (template: the price row's Badge). */
function Marks({ item }: { item: ItemResponse }) {
  const { t } = useUi();
  const { open } = useItemOpener();
  const delisted = item.status === "delisted";
  const marks = [
    item.content_rating && (
      <Badge
        key="rating"
        variant={RATING[item.content_rating]}
        label={t(`detail.rating.${item.content_rating}`)}
      />
    ),
    ...item.features.map((f) => (
      <Badge key={f} label={FEATURES[f as Feature] ?? f} />
    )),
    item.quest === "Yes" && (
      <Badge key="quest" variant="info" label={t("detail.quest")} />
    ),
    delisted && (
      <Badge key="delisted" variant="error" label={t("detail.delisted")} />
    ),
  ].filter(Boolean);
  if (!marks.length) return null;
  return (
    <HStack gap={1} vAlign="center" wrap="wrap">
      {marks}
      {delisted && item.relisted_as && (
        <Button
          label={t("detail.relisted")}
          variant="ghost"
          size="sm"
          endContent={<Icon icon="site:arrowRight" />}
          onClick={() => item.relisted_as && open(item.relisted_as)}
        />
      )}
    </HStack>
  );
}

/** Whether the item fits the avatar the visitor filters by (master's banner): fits, not checked yet, or not marked. */
function AvatarFit({ item }: { item: ItemResponse }) {
  const { t, ui } = useUi();
  const { filters } = useFilterUrl();
  const slug = filters.avatar;
  const linked = item.linked_avatars.find((a) => a.slug === slug);
  const [other, setOther] = useState<AvatarRecord | null>(null);
  useEffect(() => {
    if (!slug || linked) return;
    let live = true;
    // the same list the avatar strip shows (master browse.tsx:153)
    api<{ items: AvatarRecord[] }>("/api/avatars?limit=60")
      .then(
        (r) => live && setOther(r.items.find((a) => a.slug === slug) ?? null),
      )
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [slug, linked]);
  const avatar = linked ?? other;
  if (!slug || !avatar) return null;
  const name = avatarName(avatar, ui);
  return (
    <Banner
      status={linked ? "success" : item.needs_enrichment ? "info" : "warning"}
      title={
        linked
          ? t("detail.fits", { name })
          : item.needs_enrichment
            ? t("detail.notAnalysed", { name })
            : t("detail.notListed", { name })
      }
      icon={
        avatar.image_url ? (
          <Avatar
            src={imageSrc(avatar.image_url)}
            name={name}
            size="sm"
            shape="rounded"
            tooltip={false}
          />
        ) : undefined
      }
    />
  );
}

/** Copies the Booth link and says so in a toast (master's CopyButton). */
function CopyLink({ id }: { id: number }) {
  const { t } = useUi();
  const toast = useToast();
  return (
    <Button
      label={t("detail.copy")}
      size="lg"
      width="100%"
      icon={<Icon icon="copy" />}
      onClick={() =>
        navigator.clipboard
          .writeText(boothUrl(id))
          .then(() => toast({ body: t("detail.copied"), uniqueID: "copied" }))
      }
    />
  );
}

/** The phone footer: price, favourite, Buy on Booth (master's sticky bar, item-detail.tsx:650-669). */
export function PhoneBar({
  item,
  rates,
}: {
  item: ItemResponse;
  rates: Rates;
}) {
  const { t, settings } = useUi();
  return (
    <HStack gap={2} vAlign="center">
      <StackItem size="fill">
        <Text type="large" weight="bold" hasTabularNumbers>
          {formatPrice(priceOf(item), settings, rates)}
        </Text>
      </StackItem>
      <IconButton
        data-fav=""
        label={t("fav.add")}
        size="lg"
        icon={<Icon icon="site:favourite" />}
      />
      <Button
        label={t("detail.buy")}
        variant="primary"
        size="lg"
        href={boothUrl(item.id)}
        target="_blank"
        endContent={<Icon icon="externalLink" />}
      />
    </HStack>
  );
}
