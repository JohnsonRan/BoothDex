"use client";
// Popover Filter Panel template block: changes are staged here and reach the URL (one server render) on
// Apply; Reset clears the staged filters.

import { Button } from "@astryxdesign/core/Button";
import { CheckboxInput } from "@astryxdesign/core/CheckboxInput";
import { Divider } from "@astryxdesign/core/Divider";
import {
  SegmentedControl,
  SegmentedControlItem,
} from "@astryxdesign/core/SegmentedControl";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Heading } from "@astryxdesign/core/Text";
import { TextInput } from "@astryxdesign/core/TextInput";
import {
  ToggleButton,
  ToggleButtonGroup,
} from "@astryxdesign/core/ToggleButton";
import { use, useState } from "react";
import { useUi } from "@/components/providers";
import {
  FEATURES,
  type Feature,
  FILTER_RESET,
  type Filters,
  RANGES,
} from "@/lib/catalog";
import type { MessageKey } from "@/lib/messages";
import { LOCALES } from "@/lib/settings-core";
import { useFilterUrl } from "./filter-url";
import { TagPicker } from "./tag-picker";

// presets in yen, matching how Booth items are usually priced
const PRICE_PRESETS = [
  [0, 500, "≤ ¥500"],
  [500, 1500, "¥500–1,500"],
  [1500, 3000, "¥1,500–3,000"],
  [3000, 0, "¥3,000+"],
] as const;

const digits = (v: string) => Number.parseInt(v.replace(/\D/g, ""), 10) || 0;

export function FilterPanel({
  featureCounts,
  onDone,
}: {
  /** per feature tag, for the filter panel; resolves with the search */
  featureCounts: Promise<Record<string, number>>;
  onDone: () => void;
}) {
  const { t, ui } = useUi();
  const { filters, apply } = useFilterUrl();
  const counts = use(featureCounts);
  const [draft, setDraft] = useState(filters);
  const set = (patch: Partial<Filters>) =>
    setDraft((d) => ({ ...d, ...patch }));
  const n = (v: number | undefined) => (v ?? 0).toLocaleString(LOCALES[ui]);
  const preset = PRICE_PRESETS.find(
    ([lo, hi]) => draft.pmin === lo && draft.pmax === hi,
  );
  const commit = () => {
    const swap = draft.pmax > 0 && draft.pmin > draft.pmax;
    apply({
      ...draft,
      pmin: swap ? draft.pmax : draft.pmin,
      pmax: swap ? draft.pmin : draft.pmax,
    });
    onDone();
  };

  return (
    <VStack gap={3}>
      {filters.view !== "favourites" && (
        <SegmentedControl
          label={t("view.items")}
          layout="fill"
          value={draft.view}
          onChange={(v) =>
            set({ view: v as Filters["view"], avatar: "", sc: "", gender: "" })
          }
        >
          <SegmentedControlItem value="items" label={t("view.items")} />
          <SegmentedControlItem value="avatars" label={t("view.avatars")} />
        </SegmentedControl>
      )}
      <Heading level={4}>{t("filter.rating")}</Heading>
      <SegmentedControl
        label={t("filter.rating")}
        layout="fill"
        value={draft.adult}
        onChange={(v) => set({ adult: v as Filters["adult"] })}
      >
        <SegmentedControlItem value="safe" label={t("filter.safe")} />
        <SegmentedControlItem value="include" label={t("filter.all")} />
      </SegmentedControl>
      {draft.view === "avatars" && (
        <CheckboxInput
          label={t("filter.supportedOnly")}
          value={draft.asup === "supported"}
          onChange={(on) => set({ asup: on ? "supported" : "all" })}
        />
      )}
      <Divider />
      <Heading level={4}>{t("filter.price")}</Heading>
      <CheckboxInput
        label={t("filter.free")}
        value={draft.free}
        onChange={(free) => set({ free })}
      />
      {!draft.free && (
        <>
          <ToggleButtonGroup
            label={t("filter.price")}
            value={preset ? preset[2] : null}
            onChange={(v) => {
              const p = PRICE_PRESETS.find((x) => x[2] === v);
              set({ pmin: p?.[0] ?? 0, pmax: p?.[1] ?? 0 });
            }}
          >
            <HStack gap={1} wrap="wrap">
              {PRICE_PRESETS.map(([, , label]) => (
                <ToggleButton
                  key={label}
                  label={label}
                  value={label}
                  size="sm"
                />
              ))}
            </HStack>
          </ToggleButtonGroup>
          <HStack gap={2}>
            <TextInput
              label={t("filter.min")}
              placeholder={`¥ ${t("filter.min")}`}
              inputMode="numeric"
              value={draft.pmin ? String(draft.pmin) : ""}
              onChange={(v) => set({ pmin: digits(v) })}
              size="sm"
            />
            <TextInput
              label={t("filter.max")}
              placeholder={`¥ ${t("filter.max")}`}
              inputMode="numeric"
              value={draft.pmax ? String(draft.pmax) : ""}
              onChange={(v) => set({ pmax: digits(v) })}
              size="sm"
            />
          </HStack>
        </>
      )}
      <Divider />
      <Heading level={4}>{t("filter.time")}</Heading>
      <ToggleButtonGroup
        label={t("filter.time")}
        value={draft.range}
        onChange={(v) => set({ range: (v ?? "") as Filters["range"] })}
      >
        <HStack gap={1} wrap="wrap">
          {(["", ...RANGES] as const).map((r) => (
            <ToggleButton
              key={r || "any"}
              label={r ? t(`range.${r}` as MessageKey) : t("filter.timeAny")}
              value={r}
              size="sm"
            />
          ))}
        </HStack>
      </ToggleButtonGroup>
      <Divider />
      <Heading level={4}>{t("filter.features")}</Heading>
      {(Object.keys(FEATURES) as Feature[]).map((f) => (
        <CheckboxInput
          key={f}
          label={`${FEATURES[f]} (${n(counts[f])})`}
          value={draft.features.includes(f)}
          onChange={(on) =>
            set({
              features: on
                ? [...draft.features, f]
                : draft.features.filter((x) => x !== f),
            })
          }
        />
      ))}
      <Divider />
      <Heading level={4}>{t("filter.tags")}</Heading>
      <TagPicker
        tags={draft.tags}
        extags={draft.extags}
        onChange={(tags, extags) => set({ tags, extags })}
      />
      <Divider />
      <HStack gap={2} hAlign="end">
        <Button label={t("filter.apply")} variant="primary" onClick={commit} />
        <Button
          label={t("filter.clear")}
          variant="ghost"
          onClick={() => set(FILTER_RESET)}
        />
      </HStack>
    </VStack>
  );
}
