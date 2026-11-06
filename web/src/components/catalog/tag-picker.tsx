"use client";

// The tag clause's editor: search, then include (+) or exclude (−); chosen tags are removable tokens (green
// included, red excluded). Staged as the Popover Filter Panel block: the URL changes only on Apply.

import { Button } from "@astryxdesign/core/Button";
import { Divider } from "@astryxdesign/core/Divider";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { TextInput } from "@astryxdesign/core/TextInput";
import { Token } from "@astryxdesign/core/Token";
import { useState } from "react";
import { useUi } from "@/components/providers";
import { tagName } from "@/lib/catalog";
import { useTagSearch } from "@/lib/client-data";
import { LOCALES } from "@/lib/settings-core";
import "@/lib/icons-on-demand";
import { EDITOR_WIDTH } from "./filter-style";
import type { TagSet } from "./tag-clause";

export function TagPicker({
  value,
  onApply,
}: {
  value: TagSet;
  onApply: (next: TagSet) => void;
}) {
  const { t, ui } = useUi();
  const [{ tags, extags }, setDraft] = useState(value);
  const [q, setQ] = useState("");
  const { found, labels } = useTagSearch(q, ui);
  const without = (list: string[], name: string) =>
    list.filter((x) => x !== name);
  const include = (name: string) =>
    setDraft({
      tags: [...without(tags, name), name],
      extags: without(extags, name),
    });
  const exclude = (name: string) =>
    setDraft({
      tags: without(tags, name),
      extags: [...without(extags, name), name],
    });
  const remove = (name: string) =>
    setDraft({ tags: without(tags, name), extags: without(extags, name) });
  const label = (name: string) => labels[name] ?? name;

  return (
    // the popup floors its width at the trigger's; tag rows need this much
    <VStack gap={3} style={{ width: EDITOR_WIDTH }}>
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
      />
      <VStack gap={0}>
        {found
          .filter((x) => !tags.includes(x.name) && !extags.includes(x.name))
          .map((x) => (
            <HStack key={x.name} gap={1} vAlign="center">
              <StackItem size="fill">
                <Text maxLines={1}>{tagName(x, ui)}</Text>
              </StackItem>
              <Text type="supporting" color="secondary" hasTabularNumbers>
                {x.item_count.toLocaleString(LOCALES[ui])}
              </Text>
              <IconButton
                label={`${t("filter.include")} ${tagName(x, ui)}`}
                icon={<Icon icon="site:add" size="sm" />}
                variant="ghost"
                size="sm"
                onClick={() => include(x.name)}
              />
              <IconButton
                label={`${t("filter.exclude")} ${tagName(x, ui)}`}
                icon={<Icon icon="site:remove" size="sm" />}
                variant="ghost"
                size="sm"
                onClick={() => exclude(x.name)}
              />
            </HStack>
          ))}
      </VStack>
      <Divider />
      <HStack gap={2} hAlign="between" vAlign="center">
        <Button
          label={t("filter.clear")}
          variant="ghost"
          size="sm"
          isDisabled={
            !tags.length &&
            !extags.length &&
            !value.tags.length &&
            !value.extags.length
          }
          onClick={() => onApply({ tags: [], extags: [] })}
        />
        <Button
          label={t("filter.apply")}
          variant="secondary"
          size="sm"
          onClick={() => onApply({ tags, extags })}
        />
      </HStack>
    </VStack>
  );
}
