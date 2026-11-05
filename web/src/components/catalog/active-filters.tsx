"use client";
// The filters in force as removable tokens (Token Removable template block).

import { HStack } from "@astryxdesign/core/Stack";
import { Token } from "@astryxdesign/core/Token";
import { useUi } from "@/components/providers";
import { type Avatar, avatarName, FEATURES, type Filters } from "@/lib/catalog";
import type { MessageKey } from "@/lib/messages";
import { useFilterUrl } from "./filter-url";

export function ActiveFilters({
  categoryName,
  avatar,
}: {
  /** the chosen category's name in the interface language, when it is known */
  categoryName?: string;
  /** the selected avatar, when the strip has it */
  avatar?: Avatar;
}) {
  const { t, ui } = useUi();
  const { filters: f, apply } = useFilterUrl();
  const price = f.free
    ? t("filter.free")
    : f.pmin || f.pmax
      ? `¥${f.pmin.toLocaleString()}–${f.pmax ? f.pmax.toLocaleString() : ""}`
      : "";
  const tokens: [string, Partial<Filters>][] = [
    [f.q && `“${f.q}”`, { q: "" }],
    [f.author, { author: "" }],
    [f.avatar && (avatar ? avatarName(avatar, ui) : f.avatar), { avatar: "" }],
    [f.sc && (categoryName ?? f.sc), { sc: "" }],
    [
      f.adult === "include" ? `${t("filter.rating")}: ${t("filter.all")}` : "",
      { adult: "safe" },
    ],
    [price, { free: false, pmin: 0, pmax: 0 }],
    [f.range && t(`range.${f.range}` as MessageKey), { range: "" }],
    ...f.features.map((x): [string, Partial<Filters>] => [
      FEATURES[x],
      { features: f.features.filter((y) => y !== x) },
    ]),
  ];
  const shown = tokens.filter(([label]) => label);
  if (!shown.length) return null;
  return (
    <HStack gap={2} wrap="wrap">
      {shown.map(([label, patch]) => (
        <Token key={label} label={label} onRemove={() => apply(patch)} />
      ))}
    </HStack>
  );
}
