"use client";
// The bodies of the variations, avatars and tags sections (item-sections.tsx): block ListBasicList for the
// variations, Token chips for avatars and tags.

import { Avatar } from "@astryxdesign/core/Avatar";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { List, ListItem } from "@astryxdesign/core/List";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { Token } from "@astryxdesign/core/Token";
import { useState } from "react";
import { useFilterUrl } from "@/components/catalog/filter-url";
import { useUi } from "@/components/providers";
import type { ItemResponse } from "@/lib/api-types";
import { avatarName, imageSrc } from "@/lib/catalog";
import type { ContentLang } from "@/lib/settings-core";
import { variationName, variationState } from "./item-data";
import { useItemOpener } from "./item-host";

// master: first 12 avatars, then "Show all n"
const AVATARS_SHOWN = 12;
// master: at most 40 Booth tags
const TAGS_SHOWN = 40;
// long variation lists scroll in place (master: max-h-72, 18rem)
const VARIATIONS_HEIGHT = "18rem";

type Money = (jpy: number | null) => string;

export function Variations({
  item,
  lang,
  money,
  shared,
  isCapped,
}: {
  item: ItemResponse;
  lang: ContentLang;
  money: Money;
  shared: number | null;
  isCapped: boolean;
}) {
  const { t } = useUi();
  return (
    <VStack
      // a scrolling region is reachable by keyboard (master: the list is focusable)
      tabIndex={isCapped ? 0 : undefined}
      aria-label={t("detail.variations", { n: item.variations.length })}
      style={
        isCapped
          ? {
              maxBlockSize: VARIATIONS_HEIGHT,
              overflowY: "auto",
              // no bar, as in the popup lists (globals.css); still scrolls by wheel, touch and keyboard
              scrollbarWidth: "none",
            }
          : undefined
      }
    >
      <List>
        {item.variations.map((v) => {
          const state = variationState(v.status);
          return (
            <ListItem
              key={v.id}
              label={
                variationName(item, v.name, lang) ||
                t("detail.defaultVariation")
              }
              isDisabled={state !== "available"}
              endContent={
                <HStack gap={2} vAlign="center">
                  {state !== "available" && (
                    <Badge
                      label={
                        state === "soldOut"
                          ? t("detail.soldOut")
                          : t("detail.unavailable")
                      }
                    />
                  )}
                  {shared == null && (
                    <Text type="body" weight="bold" hasTabularNumbers>
                      {money(v.price_jpy ?? 0)}
                    </Text>
                  )}
                </HStack>
              }
            />
          );
        })}
      </List>
    </VStack>
  );
}

/** Avatar chips: picking one filters by it and closes the dialog; the current one is green. */
export function FitsAvatars({ item }: { item: ItemResponse }) {
  const { t, ui } = useUi();
  const { filters, apply } = useFilterUrl();
  const { close } = useItemOpener();
  const [all, setAll] = useState(false);
  const total = item.linked_avatars.length;
  const shown = all
    ? item.linked_avatars
    : item.linked_avatars.slice(0, AVATARS_SHOWN);
  return (
    <HStack gap={1} wrap="wrap" vAlign="center">
      {shown.map((a) => (
        <Token
          key={a.slug}
          label={avatarName(a, ui)}
          color={a.slug === filters.avatar ? "green" : "default"}
          icon={
            a.image_url ? (
              <Avatar
                src={imageSrc(a.image_url)}
                name={avatarName(a, ui)}
                size="xsm"
                tooltip={false}
              />
            ) : undefined
          }
          onClick={() => close(() => apply({ avatar: a.slug }))}
        />
      ))}
      {total > AVATARS_SHOWN && (
        <Button
          label={all ? t("detail.less") : t("detail.allAvatars", { n: total })}
          variant="ghost"
          size="sm"
          aria-expanded={all}
          onClick={() => setAll((o) => !o)}
        />
      )}
    </HStack>
  );
}

/** Booth's own tags; one searches the catalogue for it. */
export function Tags({ tags }: { tags: string[] }) {
  const { apply } = useFilterUrl();
  const { close } = useItemOpener();
  return (
    <HStack gap={1} wrap="wrap">
      {tags.slice(0, TAGS_SHOWN).map((tag) => (
        <Token
          key={tag}
          label={tag}
          onClick={() => close(() => apply({ q: tag }))}
        />
      ))}
    </HStack>
  );
}
