"use client";
// The tag clause: a ComplexSelector whose editor is the tag picker (search, include, exclude). Set, the
// trigger reads as the clause ("Tags: A, +2"); the picker loads the first time the popup opens.

import { ComplexSelector } from "@astryxdesign/core/ComplexSelector";
import dynamic from "next/dynamic";
import { useUi } from "@/components/providers";
import { CONTROL_SIZE, clause, FILTER_FILL } from "./filter-style";
import { useFilterUrl } from "./filter-url";

const TagPicker = dynamic(
  () => import("./tag-picker").then((m) => m.TagPicker),
  { ssr: false },
);

export type TagSet = { tags: string[]; extags: string[] };

export function TagClause() {
  const { t } = useUi();
  const { filters: f, apply } = useFilterUrl();
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
