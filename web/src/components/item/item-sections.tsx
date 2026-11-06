"use client";
// The info column's sections, as the product-detail template ends it (page.tsx:248-270): a CollapsibleGroup
// of type multiple, a Divider before and after every Collapsible, each trigger a level-3 Heading. All start
// closed, so the item's facts above them come first. BoothDex's sections, in master's order: description (show more / less), variations (block
// ListBasicList), fitting avatars (Tokens, first 12 then show all), Booth tags (Tokens) and the info list
// (block MetadataListBasicMetadata).

import { Button } from "@astryxdesign/core/Button";
import { Collapsible, CollapsibleGroup } from "@astryxdesign/core/Collapsible";
import { Divider } from "@astryxdesign/core/Divider";
import {
  MetadataList,
  MetadataListItem,
} from "@astryxdesign/core/MetadataList";
import { VStack } from "@astryxdesign/core/Stack";
import { Heading, Text } from "@astryxdesign/core/Text";
import { Fragment, useLayoutEffect, useRef, useState } from "react";
import { useUi } from "@/components/providers";
import type { ItemResponse } from "@/lib/api-types";
import { type ContentLang, itemText, LOCALES } from "@/lib/settings-core";
import { commonPrice } from "./item-data";
import { FitsAvatars, Tags, Variations } from "./item-lists";

// the description's lines before "Show more" (master: 20rem at its 28 px leading)
const BIO_LINES = 12;

type Money = (jpy: number | null) => string;

export function ItemSections({
  item,
  lang,
  money,
  isWide,
}: {
  item: ItemResponse;
  lang: ContentLang;
  money: Money;
  /** from lg up the variation list scrolls in place; phones show it whole (no scroll inside a scroll) */
  isWide: boolean;
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
        <Variations
          item={item}
          lang={lang}
          money={money}
          shared={shared}
          isCapped={isWide}
        />
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
    <CollapsibleGroup type="multiple">
      <Divider />
      {sections.map((s) => (
        <Fragment key={s.value}>
          <Collapsible
            value={s.value}
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
