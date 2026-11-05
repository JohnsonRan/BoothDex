"use client";

// Smart-tag picker inside the filter panel: search, then include (+) or exclude (−); chosen tags are
// removable tokens (green included, red excluded).

import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { TextInput } from "@astryxdesign/core/TextInput";
import { Token } from "@astryxdesign/core/Token";
import { Minus, Plus } from "lucide-react";
import { useState } from "react";
import { useUi } from "@/components/providers";
import { tagName } from "@/lib/catalog";
import { useTagSearch } from "@/lib/client-data";
import { LOCALES } from "@/lib/settings-core";

export function TagPicker({
  tags,
  extags,
  onChange,
}: {
  tags: string[];
  extags: string[];
  onChange: (tags: string[], extags: string[]) => void;
}) {
  const { t, ui } = useUi();
  const [q, setQ] = useState("");
  const { found, labels } = useTagSearch(q, ui);
  const without = (list: string[], name: string) =>
    list.filter((x) => x !== name);
  const include = (name: string) =>
    onChange([...without(tags, name), name], without(extags, name));
  const exclude = (name: string) =>
    onChange(without(tags, name), [...without(extags, name), name]);
  const remove = (name: string) =>
    onChange(without(tags, name), without(extags, name));
  const label = (name: string) => labels[name] ?? name;

  return (
    <VStack gap={2}>
      {tags.length + extags.length > 0 && (
        <HStack gap={1} wrap="wrap">
          {tags.map((name) => (
            <Token
              key={name}
              label={label(name)}
              color="green"
              onRemove={() => remove(name)}
            />
          ))}
          {extags.map((name) => (
            <Token
              key={name}
              label={label(name)}
              color="red"
              description={t("filter.exclude")}
              onRemove={() => remove(name)}
            />
          ))}
        </HStack>
      )}
      <TextInput
        label={t("filter.tagSearch")}
        isLabelHidden
        placeholder={t("filter.tagSearch")}
        value={q}
        onChange={setQ}
        startIcon="search"
        size="sm"
      />
      {found
        .filter((x) => !tags.includes(x.name) && !extags.includes(x.name))
        .map((x) => (
          <HStack key={x.name} gap={1} vAlign="center">
            <StackItem size="fill">
              <Text maxLines={1}>{tagName(x, ui)}</Text>
            </StackItem>
            <Text type="supporting" hasTabularNumbers>
              {x.item_count.toLocaleString(LOCALES[ui])}
            </Text>
            <IconButton
              label={`${t("filter.include")} ${tagName(x, ui)}`}
              icon={<Icon icon={Plus} />}
              variant="ghost"
              size="sm"
              onClick={() => include(x.name)}
            />
            <IconButton
              label={`${t("filter.exclude")} ${tagName(x, ui)}`}
              icon={<Icon icon={Minus} />}
              variant="ghost"
              size="sm"
              onClick={() => exclude(x.name)}
            />
          </HStack>
        ))}
    </VStack>
  );
}
