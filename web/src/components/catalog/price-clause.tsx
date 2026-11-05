"use client";
// The price clause: a ComplexSelector, as the table-filter template's Quote range. The trigger is the chip;
// the editor's (free, presets, min/max) code loads after the page (lib/preload).

import { ComplexSelector } from "@astryxdesign/core/ComplexSelector";
import dynamic from "next/dynamic";
import { useEffect } from "react";
import { useUi } from "@/components/providers";
import { preloadWhenIdle } from "@/lib/preload";
import { CONTROL_SIZE, clause, FILTER_FILL } from "./filter-style";
import { useFilterUrl } from "./filter-url";

const loadEditor = () => import("./price-editor");
const PriceEditor = dynamic(() => loadEditor().then((m) => m.PriceEditor), {
  ssr: false,
});

export type Price = { free: boolean; pmin: number; pmax: number };

/** "¥500–1,500", "¥3,000+", "≤ ¥500", or "" for no bound. */
export function priceText({ pmin, pmax }: Price) {
  const yen = (v: number) => `¥${v.toLocaleString("en")}`;
  if (pmin && pmax) return `${yen(pmin)}–${pmax.toLocaleString("en")}`;
  if (pmin) return `${yen(pmin)}+`;
  return pmax ? `≤ ${yen(pmax)}` : "";
}

export function PriceClause() {
  const { t } = useUi();
  const { filters: f, apply } = useFilterUrl();
  useEffect(() => preloadWhenIdle(loadEditor), []);
  const value: Price = { free: f.free, pmin: f.pmin, pmax: f.pmax };
  const set = f.free || !!f.pmin || !!f.pmax;
  return (
    <ComplexSelector<Price>
      label={t("filter.price")}
      isLabelHidden
      placeholder={t("filter.price")}
      triggerLabel={
        set
          ? clause(
              t("filter.price"),
              f.free ? t("filter.free") : priceText(value),
            )
          : undefined
      }
      size={CONTROL_SIZE}
      value={value}
      onChange={(v) => apply(v)}
      controlStyle={set ? FILTER_FILL : undefined}
    >
      {(v, onChange, close, state) =>
        state.isOpen && (
          <PriceEditor
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
