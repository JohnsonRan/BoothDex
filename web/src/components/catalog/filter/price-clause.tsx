"use client";
// The price clause: a ComplexSelector, as the table-filter template's Quote range (page.tsx:3335-3345). The
// trigger is the chip; the editor's (free, presets, min/max) code loads after the page (lib/lazy).

import { ComplexSelector } from "@astryxdesign/core/ComplexSelector";
import { useEffect } from "react";
import { useUi } from "@/components/providers";
import { Lazy, lazySurface, preloadWhenIdle } from "@/lib/lazy";
import { CONTROL_SIZE, FILTER_FILL } from "./filter-style";
import { useFilterUrl } from "./filter-url";

const PriceEditor = lazySurface(() =>
  import("./price-editor").then((m) => m.PriceEditor),
);

export type Price = { free: boolean; pmin: number; pmax: number };

/** `label`: the set clause's text (clause-text.ts); undefined while unset, so the trigger shows the field. */
export function PriceClause({ label }: { label?: string }) {
  const { t } = useUi();
  const { filters: f, apply } = useFilterUrl();
  useEffect(() => preloadWhenIdle(PriceEditor.preload), []);
  const value: Price = { free: f.free, pmin: f.pmin, pmax: f.pmax };
  return (
    <ComplexSelector<Price>
      label={t("filter.price")}
      isLabelHidden
      placeholder={t("filter.price")}
      triggerLabel={label}
      size={CONTROL_SIZE}
      value={value}
      onChange={(v) => apply(v)}
      controlStyle={label ? FILTER_FILL : undefined}
    >
      {(v, onChange, close, state) =>
        state.isOpen && (
          <Lazy
            surface={PriceEditor}
            value={v}
            onApply={(next) => {
              onChange(next);
              close();
            }}
          />
        )
      }
    </ComplexSelector>
  );
}
