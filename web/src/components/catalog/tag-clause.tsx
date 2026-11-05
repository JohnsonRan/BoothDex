"use client";
// The tag clause: a ComplexSelector whose editor is the tag picker (search, include, exclude). Set, the
// trigger reads as the clause ("Tags: A, +2"); the picker's code loads after the page (lib/lazy).

import { ComplexSelector } from "@astryxdesign/core/ComplexSelector";
import { useEffect } from "react";
import { useUi } from "@/components/providers";
import { Lazy, lazySurface, preloadWhenIdle } from "@/lib/lazy";
import { CONTROL_SIZE, clause, FILTER_FILL } from "./filter-style";
import { useFilterUrl } from "./filter-url";

const TagPicker = lazySurface(() =>
  import("./tag-picker").then((m) => m.TagPicker),
);

export type TagSet = { tags: string[]; extags: string[] };

export function TagClause() {
  const { t } = useUi();
  const { filters: f, apply } = useFilterUrl();
  useEffect(() => preloadWhenIdle(TagPicker.preload), []);
  const names = [...f.tags, ...f.extags.map((x) => `−${x}`)];
  return (
    <ComplexSelector<TagSet>
      label={t("filter.tags")}
      isLabelHidden
      placeholder={t("filter.tags")}
      triggerLabel={
        names.length
          ? clause(
              t("filter.tags"),
              names.length > 1 ? `${names[0]}, +${names.length - 1}` : names[0],
            )
          : undefined
      }
      size={CONTROL_SIZE}
      value={{ tags: f.tags, extags: f.extags }}
      onChange={(v) => apply(v)}
      controlStyle={names.length ? FILTER_FILL : undefined}
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
