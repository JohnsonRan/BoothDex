"use client";
// Parts of the info column beside the template's own: the avatar-fit Banner, the copy-link Button and the
// phones' footer bar (item-info.tsx).

import { Avatar } from "@astryxdesign/core/Avatar";
import { Banner } from "@astryxdesign/core/Banner";
import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { HStack, StackItem } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { useToast } from "@astryxdesign/core/Toast";
import { useEffect, useState } from "react";
import { useFilterUrl } from "@/components/catalog/filter-url";
import { useUi } from "@/components/providers";
import { api } from "@/lib/api";
import type { ItemResponse } from "@/lib/api-types";
import {
  type Avatar as AvatarRecord,
  avatarName,
  imageSrc,
  priceOf,
} from "@/lib/catalog";
import { copyText } from "@/lib/clipboard";
import { formatPrice, type Rates } from "@/lib/settings-core";
import { boothUrl } from "./item-data";

/** Whether the item fits the avatar the visitor filters by (master's banner): fits, not checked yet, or not marked. */
export function AvatarFit({ item }: { item: ItemResponse }) {
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
export function CopyLink({ id }: { id: number }) {
  const { t } = useUi();
  const toast = useToast();
  return (
    <Button
      label={t("detail.copy")}
      size="lg"
      width="100%"
      icon={<Icon icon="copy" />}
      onClick={() =>
        copyText(boothUrl(id)).then(
          () => toast({ body: t("detail.copied"), uniqueID: "copied" }),
          () =>
            toast({
              type: "error",
              body: t("detail.copyFailed"),
              uniqueID: "copied",
            }),
        )
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
          {formatPrice(priceOf(item), settings, rates, t)}
        </Text>
      </StackItem>
      <IconButton
        // answered by account/favourites.tsx, as the tiles' buttons are
        data-fav={String(item.id)}
        aria-pressed={false}
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
