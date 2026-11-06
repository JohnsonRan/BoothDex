"use client";
// The tag clause: a ComplexSelector (the table-filter template's Quote shell, page.tsx:3335-3345) whose editor is
// the tag picker (search, include, exclude). Set, the trigger reads as the template's multi-selectors do (first value and a count, :3315-3319, display names);
// the picker's code loads after the page (lib/preload).

import { ComplexSelector } from "@astryxdesign/core/ComplexSelector";
import dynamic from "next/dynamic";
import { useEffect } from "react";
import { useUi } from "@/components/providers";
import { preloadWhenIdle } from "@/lib/preload";
import { CONTROL_SIZE, FILTER_FILL } from "./filter-style";
import { useFilterUrl } from "./filter-url";

const loadPicker = () => import("./tag-picker");
const TagPicker = dynamic(() => loadPicker().then((m) => m.TagPicker), {
  ssr: false,
});

export type TagSet = { tags: string[]; extags: string[] };

/** `label`: the set clause's text (clause-text.ts); undefined while unset, so the trigger shows the field. */
export function TagClause({ label }: { label?: string }) {
  const { t } = useUi();
  const { filters: f, apply } = useFilterUrl();
  useEffect(() => preloadWhenIdle(loadPicker), []);
  return (
    <ComplexSelector<TagSet>
      label={t("filter.tags")}
      isLabelHidden
      placeholder={t("filter.tags")}
      triggerLabel={label}
      size={CONTROL_SIZE}
      value={{ tags: f.tags, extags: f.extags }}
      onChange={(v) => apply(v)}
      controlStyle={label ? FILTER_FILL : undefined}
    >
      {(v, onChange, close, state) =>
        state.isOpen && (
          <TagPicker
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
