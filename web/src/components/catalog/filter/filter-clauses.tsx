"use client";
// The Filterable Table template's clause controls (page table-filter `filterControls`, page.tsx:3278-3406): one
// closed `sm` trigger per filter field that doubles as its own chip — the bare field name unset, the whole clause
// once set (`renderValue`, :3250), with a clear (×) and the filled surface (:3253). One-of-many fields are
// Selectors (`renderFilterControl`, :3232-3271), the features a MultiSelector (:3302-3326), price and tags
// ComplexSelectors as the template's Quote (:3335-3405), the view switches preset ToggleButtons (:3279-3300).

import { MultiSelector } from "@astryxdesign/core/MultiSelector";
import { Selector } from "@astryxdesign/core/Selector";
import { ToggleButton } from "@astryxdesign/core/ToggleButton";
import type { ReactElement } from "react";
import { useUi } from "@/components/providers";
import { FEATURES, type Feature, type Filters, RANGES } from "@/lib/catalog";
import { LOCALES } from "@/lib/settings-core";
import {
  CLAUSE_CLEAR,
  type ClauseKey,
  type ClauseNames,
  clauseTexts,
} from "./clause-text";
import {
  CONTROL_SIZE,
  type Facets,
  FILTER_FILL,
  toggleStyle,
} from "./filter-style";
import { useFilterUrl } from "./filter-url";
import { PriceClause } from "./price-clause";
import { TagClause } from "./tag-clause";

type Option = { value: string; label: string; description?: string };

/** The row's clauses for the current view, in order; each element is keyed (OverflowList folds by index). */
export function useClauses(
  facets: Facets | null,
  names: ClauseNames,
): ReactElement[] {
  const { t, ui } = useUi();
  const { filters: f, apply } = useFilterUrl();
  const texts = clauseTexts(f, names, t);
  const count = (v: number | undefined) =>
    facets
      ? t("title.count", { n: (v ?? 0).toLocaleString(LOCALES[ui]) })
      : undefined;
  const avatars = f.view === "avatars";

  // renderFilterControl: unset the trigger reads as the field; set, as the clause
  const one = (
    key: ClauseKey,
    field: string,
    options: Option[],
    value: string,
    onChange: (v: string) => void,
    hasSearch?: boolean,
  ) => (
    <Selector
      key={key}
      label={field}
      isLabelHidden
      placeholder={field}
      size={CONTROL_SIZE}
      hasClear
      hasSearch={hasSearch}
      options={options}
      value={value || null}
      renderValue={() => texts[key]}
      controlStyle={value ? FILTER_FILL : undefined}
      onChange={(v) => (v == null ? apply(CLAUSE_CLEAR[key]) : onChange(v))}
    />
  );

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
      style={toggleStyle(isPressed)}
      onPressedChange={onPressed}
    />
  );

  const out: (ReactElement | false)[] = [
    // the shop has no field of its own: a link sets it, and its clause is how it is removed
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
          if (v == null) apply(CLAUSE_CLEAR.author);
        }}
      />
    ),
    avatars
      ? one(
          "gender",
          t("filter.gender"),
          [
            { value: "female", label: t("filter.female") },
            { value: "male", label: t("filter.male") },
          ],
          f.gender,
          (v) => apply({ gender: v as Filters["gender"] }),
        )
      : one(
          "sc",
          t("filter.categories"),
          names.categories.map(([value, label]) => ({
            value,
            label,
            description: count(facets?.smart[value]),
          })),
          f.sc,
          (sc) => apply({ sc }),
        ),
    // an avatar body is not fitted to another avatar
    !avatars &&
      one(
        "avatar",
        t("filter.avatar"),
        names.avatars.map(([value, label]) => ({ value, label })),
        f.avatar,
        (avatar) => apply({ avatar }),
        true,
      ),
    // two ratings with the default unset, so the trigger reads as the bare field until "All" is chosen
    one(
      "adult",
      t("filter.rating"),
      [
        { value: "safe", label: t("filter.safe") },
        { value: "include", label: t("filter.all") },
      ],
      f.adult === "include" ? "include" : "",
      (v) => apply({ adult: v === "include" ? "include" : "safe" }),
    ),
    <PriceClause key="price" label={texts.price} />,
    one(
      "range",
      t("filter.time"),
      RANGES.map((r) => ({ value: r, label: t(`range.${r}`) })),
      f.range,
      (v) => apply({ range: v as Filters["range"] }),
    ),
    <MultiSelector
      key="features"
      label={t("filter.features")}
      isLabelHidden
      placeholder={t("filter.features")}
      size={CONTROL_SIZE}
      hasClear
      triggerDisplay="labels"
      formatValue={() => texts.features ?? ""}
      options={(Object.keys(FEATURES) as Feature[]).map((x) => ({
        value: x,
        label: FEATURES[x],
        description: count(facets?.features[x]),
      }))}
      value={f.features}
      controlStyle={f.features.length ? FILTER_FILL : undefined}
      onChange={(v) => apply({ features: v as Feature[] })}
    />,
    <TagClause key="tags" label={texts.tags} />,
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
  ];
  return out.filter((x): x is ReactElement => !!x);
}
