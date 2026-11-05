"use client";
// The Filterable Table template's clause controls (page table-filter, `filterControls`): one closed trigger
// per filter field that doubles as its own chip — the bare field name unset, the whole clause once set, with
// a clear (×) beside it. Presets are ToggleButtons, one-of-many fields Selectors, many-of-many MultiSelectors,
// and fields with their own editor (price, tags) ComplexSelectors, as in the template.

import { MultiSelector } from "@astryxdesign/core/MultiSelector";
import { Selector } from "@astryxdesign/core/Selector";
import { ToggleButton } from "@astryxdesign/core/ToggleButton";
import type { ReactElement } from "react";
import { useUi } from "@/components/providers";
import { FEATURES, type Feature, type Filters, RANGES } from "@/lib/catalog";
import type { MessageKey } from "@/lib/messages";
import { LOCALES } from "@/lib/settings-core";
import {
  CONTROL_SIZE,
  clause,
  type Facets,
  FILTER_CHROME,
  FILTER_FILL,
  FILTER_SURFACE,
} from "./filter-style";
import { useFilterUrl } from "./filter-url";
import { PriceClause } from "./price-clause";
import { TagClause } from "./tag-clause";

/** The row's clauses for the current view, in order; each element is keyed (OverflowList folds by index). */
export function useClauses(facets: Facets | null): ReactElement[] {
  const { t, ui } = useUi();
  const { filters: f, apply } = useFilterUrl();
  const n = (v: number | undefined) => (v ?? 0).toLocaleString(LOCALES[ui]);
  const avatars = f.view === "avatars";

  const preset = (
    key: string,
    label: string,
    isPressed: boolean,
    onPressed: (on: boolean) => void,
  ) => (
    <ToggleButton
      key={key}
      label={label}
      size={CONTROL_SIZE}
      isPressed={isPressed}
      style={
        isPressed ? FILTER_CHROME : { ...FILTER_CHROME, ...FILTER_SURFACE }
      }
      onPressedChange={onPressed}
    />
  );

  const rangeLabel = (r: string) => t(`range.${r}` as MessageKey);
  const out: (ReactElement | false)[] = [
    // the shop has no field of its own: a palette pick sets it, and its clause is how it is removed
    !!f.author && (
      <Selector
        key="author"
        label={f.author}
        isLabelHidden
        size={CONTROL_SIZE}
        hasClear
        options={[f.author]}
        value={f.author}
        controlStyle={FILTER_FILL}
        onChange={(v) => {
          if (v == null) apply({ author: "" });
        }}
      />
    ),
    f.view !== "favourites" &&
      preset("view", t("view.avatars"), avatars, (on) =>
        apply({
          view: on ? "avatars" : "items",
          avatar: "",
          sc: "",
          gender: "",
        }),
      ),
    avatars &&
      preset("asup", t("filter.supportedOnly"), f.asup === "supported", (on) =>
        apply({ asup: on ? "supported" : "all" }),
      ),
    preset(
      "adult",
      clause(t("filter.rating"), t("filter.all")),
      f.adult === "include",
      (on) => apply({ adult: on ? "include" : "safe" }),
    ),
    <PriceClause key="price" />,
    <Selector
      key="range"
      label={t("filter.time")}
      isLabelHidden
      placeholder={t("filter.time")}
      size={CONTROL_SIZE}
      hasClear
      options={RANGES.map((r) => ({ value: r, label: rangeLabel(r) }))}
      value={f.range || null}
      renderValue={(o) => clause(t("filter.time"), rangeLabel(o.value))}
      controlStyle={f.range ? FILTER_FILL : undefined}
      onChange={(v) => apply({ range: (v ?? "") as Filters["range"] })}
    />,
    <MultiSelector
      key="features"
      label={t("filter.features")}
      isLabelHidden
      placeholder={t("filter.features")}
      size={CONTROL_SIZE}
      hasClear
      triggerDisplay="labels"
      // one name and a count, as the template's multi-selectors: three labels would fold the rest of the row
      formatValue={(items) =>
        items.length > 1
          ? `${items[0].label}, +${items.length - 1}`
          : String(items[0]?.label ?? "")
      }
      options={(Object.keys(FEATURES) as Feature[]).map((x) => ({
        value: x,
        label: FEATURES[x],
        description: facets
          ? t("title.count", { n: n(facets.features[x]) })
          : undefined,
      }))}
      value={f.features}
      controlStyle={f.features.length ? FILTER_FILL : undefined}
      onChange={(v) => apply({ features: v as Feature[] })}
    />,
    <TagClause key="tags" />,
  ];
  return out.filter((x): x is ReactElement => !!x);
}
