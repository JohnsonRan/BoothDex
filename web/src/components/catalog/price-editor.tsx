"use client";
// The price clause's editor, laid out as the table-filter template's Quote popup (heading and value, the
// control, a Divider, then Clear and Done) with the Popover Filter Panel block's staged Apply: nothing reaches
// the URL until Apply, which also swaps reversed bounds.

import { Button } from "@astryxdesign/core/Button";
import { CheckboxInput } from "@astryxdesign/core/CheckboxInput";
import { Divider } from "@astryxdesign/core/Divider";
import { RadioList, RadioListItem } from "@astryxdesign/core/RadioList";
import { HStack, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { TextInput } from "@astryxdesign/core/TextInput";
import { useState } from "react";
import { useUi } from "@/components/providers";
import {
  digits,
  presetKey as key,
  PRICE_PRESETS as PRESETS,
  priceText,
} from "./clause-text";
import { EDITOR_WIDTH } from "./filter-style";
import type { Price } from "./price-clause";

export function PriceEditor({
  value,
  onApply,
}: {
  value: Price;
  onApply: (next: Price) => void;
}) {
  const { t } = useUi();
  const [draft, setDraft] = useState(value);
  const set = (patch: Partial<Price>) => setDraft((d) => ({ ...d, ...patch }));
  const preset = PRESETS.find(
    ([lo, hi]) => draft.pmin === lo && draft.pmax === hi,
  );
  const any = !draft.free && !draft.pmin && !draft.pmax;
  const apply = () => {
    const swap = draft.pmax > 0 && draft.pmin > draft.pmax;
    onApply({
      free: draft.free,
      pmin: draft.free ? 0 : swap ? draft.pmax : draft.pmin,
      pmax: draft.free ? 0 : swap ? draft.pmin : draft.pmax,
    });
  };

  return (
    // the popup floors its width at the trigger's; the presets and two inputs need this much
    <VStack gap={4} style={{ width: EDITOR_WIDTH }}>
      <VStack gap={0}>
        <Text type="label">{t("filter.price")}</Text>
        <Text type="large" hasTabularNumbers>
          {draft.free
            ? t("filter.free")
            : priceText(draft) || t("filter.timeAny")}
        </Text>
      </VStack>
      <CheckboxInput
        label={t("filter.free")}
        value={draft.free}
        onChange={(free) => set({ free })}
      />
      {!draft.free && (
        <>
          <RadioList
            label={t("filter.price")}
            isLabelHidden
            value={preset ? key(preset) : any ? "any" : ""}
            onChange={(v) => {
              const p = PRESETS.find((x) => key(x) === v);
              set({ pmin: p?.[0] ?? 0, pmax: p?.[1] ?? 0 });
            }}
          >
            <RadioListItem value="any" label={t("filter.timeAny")} />
            {PRESETS.map((p) => (
              <RadioListItem
                key={key(p)}
                value={key(p)}
                label={priceText({ pmin: p[0], pmax: p[1] })}
              />
            ))}
          </RadioList>
          <HStack gap={2}>
            <TextInput
              label={t("filter.min")}
              placeholder={`¥ ${t("filter.min")}`}
              inputMode="numeric"
              value={draft.pmin ? String(draft.pmin) : ""}
              onChange={(v) => set({ pmin: digits(v) })}
              onEnter={apply}
            />
            <TextInput
              label={t("filter.max")}
              placeholder={`¥ ${t("filter.max")}`}
              inputMode="numeric"
              value={draft.pmax ? String(draft.pmax) : ""}
              onChange={(v) => set({ pmax: digits(v) })}
              onEnter={apply}
            />
          </HStack>
        </>
      )}
      <Divider />
      <HStack gap={2} hAlign="between" vAlign="center">
        <Button
          label={t("filter.clear")}
          variant="ghost"
          size="sm"
          isDisabled={!value.free && !value.pmin && !value.pmax && any}
          onClick={() => onApply({ free: false, pmin: 0, pmax: 0 })}
        />
        <Button
          label={t("filter.apply")}
          variant="secondary"
          size="sm"
          onClick={apply}
        />
      </HStack>
    </VStack>
  );
}
