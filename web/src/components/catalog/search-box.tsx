"use client";

import { TextInput } from "@astryxdesign/core/TextInput";
import { useState } from "react";
import { useUi } from "@/components/providers";
import { useFilterUrl } from "./filter-url";

/** Text search; Enter applies it (IME-safe), clearing the field drops it. */
export function SearchBox() {
  const { filters } = useFilterUrl();
  // remounted when the URL's query changes (Back, a cleared token), so the field shows it
  return <Field key={filters.q} />;
}

function Field() {
  const { t } = useUi();
  const { filters, apply } = useFilterUrl();
  const [value, setValue] = useState(filters.q);
  return (
    <TextInput
      type="search"
      label={t("nav.search")}
      isLabelHidden
      placeholder={t("nav.search")}
      value={value}
      onChange={(v) => {
        setValue(v);
        if (!v && filters.q) apply({ q: "" });
      }}
      onEnter={() => apply({ q: value.trim() })}
      startIcon="search"
      enterKeyHint="search"
      hasClear
      size="lg"
    />
  );
}
