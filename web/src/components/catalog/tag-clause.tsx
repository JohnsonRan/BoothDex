"use client";
// The tag clause: a ComplexSelector (the table-filter template's Quote shell, page.tsx:3335-3345) whose editor is
// the tag picker (search, include, exclude). Set, the trigger reads as the template's multi-selectors do (first value and a count, :3315-3319, display names);
// the picker's code loads after the page (lib/lazy).

import { ComplexSelector } from "@astryxdesign/core/ComplexSelector";
import { useEffect } from "react";
import { useUi } from "@/components/providers";
import { Lazy, lazySurface, preloadWhenIdle } from "@/lib/lazy";
import { CONTROL_SIZE, FILTER_FILL } from "./filter-style";
import { useFilterUrl } from "./filter-url";

const TagPicker = lazySurface(() =>
  import("./tag-picker").then((m) => m.TagPicker),
);

export type TagSet = { tags: string[]; extags: string[] };

/** `label`: the set clause's text (clause-text.ts); undefined while unset, so the trigger shows the field, or
 *  `placeholder` where a heading names it (filter-rail.tsx). */
export function TagClause({
  label,
  placeholder,
}: {
  label?: string;
  placeholder?: string;
}) {
  const { t } = useUi();
  const { filters: f, apply } = useFilterUrl();
  useEffect(() => preloadWhenIdle(TagPicker.preload), []);
  return (
    <ComplexSelector<TagSet>
      label={t("filter.tags")}
      isLabelHidden
      placeholder={placeholder ?? t("filter.tags")}
      triggerLabel={label}
      size={CONTROL_SIZE}
      value={{ tags: f.tags, extags: f.extags }}
      onChange={(v) => apply(v)}
      controlStyle={label ? FILTER_FILL : undefined}
    >
      {(v, onChange, close, state) =>
        state.isOpen && (
          <Lazy
            surface={TagPicker}
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
