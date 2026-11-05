"use client";
// The info column's sections, as the product-detail template ends it (page.tsx:248-270): a CollapsibleGroup
// of type multiple, a Divider before and after every Collapsible, each trigger a level-3 Heading, the first
// open by default. BoothDex's sections, in master's order: description (show more / less), variations (block
// ListBasicList), fitting avatars (Tokens, first 12 then show all), Booth tags (Tokens) and the info list
// (block MetadataListBasicMetadata).

import { Avatar } from "@astryxdesign/core/Avatar";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { Collapsible, CollapsibleGroup } from "@astryxdesign/core/Collapsible";
import { Divider } from "@astryxdesign/core/Divider";
import { List, ListItem } from "@astryxdesign/core/List";
import {
  MetadataList,
  MetadataListItem,
} from "@astryxdesign/core/MetadataList";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Heading, Text } from "@astryxdesign/core/Text";
import { Token } from "@astryxdesign/core/Token";
import { Fragment, useLayoutEffect, useRef, useState } from "react";
import { useFilterUrl } from "@/components/catalog/filter-url";
import { useUi } from "@/components/providers";
import type { ItemResponse } from "@/lib/api-types";
import { avatarName, imageSrc } from "@/lib/catalog";
import { type ContentLang, itemText, LOCALES } from "@/lib/settings-core";
import { commonPrice, variationName, variationState } from "./item-data";
import { useItemOpener } from "./item-host";

// master: first 12 avatars, then "Show all n"
const AVATARS_SHOWN = 12;
// master: at most 40 Booth tags
const TAGS_SHOWN = 40;
// the description's lines before "Show more" (master: 20rem at its 28 px leading)
const BIO_LINES = 12;
// long variation lists scroll in place (master: max-h-72, 18rem)
const VARIATIONS_HEIGHT = "18rem";

type Money = (jpy: number | null) => string;

export function ItemSections({
  item,
  lang,
  money,
}: {
  item: ItemResponse;
  lang: ContentLang;
  money: Money;
}) {
  const { t, ui } = useUi();
  const locale = LOCALES[ui];
  const shared = commonPrice(item.variations);
  const sections = [
    {
      value: "bio",
      title: t("detail.bio"),
      body: <Bio text={itemText(item, "bio", lang)} />,
    },
    item.variations.length > 0 && {
      value: "variations",
      title: [
        t("detail.variations", { n: item.variations.length }),
        shared != null && money(shared),
      ]
        .filter(Boolean)
        .join(" · "),
      body: (
        <Variations item={item} lang={lang} money={money} shared={shared} />
      ),
    },
    item.linked_avatars.length > 0 && {
      value: "avatars",
      title: t("detail.avatars", { n: item.linked_avatars.length }),
      body: <FitsAvatars item={item} />,
    },
    item.booth_tags.length > 0 && {
      value: "tags",
      title: t("detail.tags"),
      body: <Tags tags={item.booth_tags} />,
    },
    {
      value: "info",
      title: t("detail.info"),
      body: (
        <MetadataList>
          <MetadataListItem label={t("detail.itemId")}>
            {String(item.id)}
          </MetadataListItem>
          <MetadataListItem label={t("detail.published")}>
            {item.published_at
              ? new Date(item.published_at).toLocaleDateString(locale)
              : "—"}
          </MetadataListItem>
          <MetadataListItem label={t("detail.boothCategory")}>
            {item.booth_category ?? "—"}
          </MetadataListItem>
          <MetadataListItem label={t("detail.scraped")}>
            {new Date(item.scraped_at).toLocaleString(locale)}
          </MetadataListItem>
        </MetadataList>
      ),
    },
  ].filter((s) => !!s);

  return (
    <CollapsibleGroup type="multiple" defaultValue={["bio"]}>
      <Divider />
      {sections.map((s) => (
        <Fragment key={s.value}>
          <Collapsible
            value={s.value}
            defaultIsOpen={s.value === "bio"}
            trigger={<Heading level={3}>{s.title}</Heading>}
          >
            {s.body}
          </Collapsible>
          <Divider />
        </Fragment>
      ))}
    </CollapsibleGroup>
  );
}

/** The description, clamped to BIO_LINES with Show more / Show less when it is longer (master's Bio). */
function Bio({ text }: { text: string }) {
  const { t } = useUi();
  const box = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [long, setLong] = useState(false);
  // measured clamped, before paint, so the button never appears late
  // biome-ignore lint/correctness/useExhaustiveDependencies: measured again for each text
  useLayoutEffect(() => {
    const el = box.current?.firstElementChild;
    if (el) setLong(el.scrollHeight > el.clientHeight + 1);
  }, [text]);
  return (
    <VStack gap={2} hAlign="start">
      <VStack ref={box} width="100%">
        <Text
          type="body"
          as="p"
          maxLines={open ? 0 : BIO_LINES}
          hasTruncateTooltip={false}
          style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}
        >
          {text}
        </Text>
      </VStack>
      {long && (
        <Button
          label={open ? t("detail.less") : t("detail.more")}
          variant="ghost"
          size="sm"
          onClick={() => setOpen((o) => !o)}
        />
      )}
    </VStack>
  );
}

function Variations({
  item,
  lang,
  money,
  shared,
}: {
  item: ItemResponse;
  lang: ContentLang;
  money: Money;
  shared: number | null;
}) {
  const { t } = useUi();
  return (
    <VStack
      // a scrolling region is reachable by keyboard (master: the list is focusable)
      tabIndex={0}
      aria-label={t("detail.variations", { n: item.variations.length })}
      style={{ maxBlockSize: VARIATIONS_HEIGHT, overflowY: "auto" }}
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
function FitsAvatars({ item }: { item: ItemResponse }) {
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
function Tags({ tags }: { tags: string[] }) {
  const { apply } = useFilterUrl();
  const { close } = useItemOpener();
  return (
    <HStack gap={1} wrap="wrap">
      {tags.slice(0, TAGS_SHOWN).map((tag) => (
        <Token
          key={tag}
          label={tag}
          size="sm"
          onClick={() => close(() => apply({ q: tag }))}
        />
      ))}
    </HStack>
  );
}
