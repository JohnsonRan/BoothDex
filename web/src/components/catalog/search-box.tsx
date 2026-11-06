"use client";
// The site search, leading the filter row as the table-filter template's search (page.tsx:3419-3430): an `sm`
// TextInput in its own slot, which sits at the selectors' width and widens on focus (`searchSlot`, :1500-1521;
// app/filter-row.css). Typing applies the query after a pause, Enter at once; Ctrl/Cmd+K focuses the field.
// While an input method is composing, nothing is applied until the text is committed.

import { StackItem } from "@astryxdesign/core/Stack";
import { TextInput } from "@astryxdesign/core/TextInput";
import { useEffect, useRef, useState } from "react";
import { useUi } from "@/components/providers";
import { useFilterUrl } from "./filter-url";

const SEARCH_DELAY_MS = 400;

export function SearchBox() {
  const { t } = useUi();
  const { filters, apply } = useFilterUrl();
  const input = useRef<HTMLInputElement>(null);
  const [composing, setComposing] = useState(false);
  const [value, setValue] = useState(filters.q);
  // the URL's query changed by something else (Clear all, Back, a saved search): the field shows it; the field's
  // own changes come back equal to what it holds, so typing is never overwritten
  const [shown, setShown] = useState(filters.q);
  if (filters.q !== shown) {
    setShown(filters.q);
    if (filters.q !== value.trim()) setValue(filters.q);
  }
  const query = value.trim();

  useEffect(() => {
    if (composing || query === filters.q) return;
    const timer = setTimeout(() => apply({ q: query }), SEARCH_DELAY_MS);
    return () => clearTimeout(timer);
  }, [composing, query, filters.q, apply]);

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        input.current?.focus();
        input.current?.select();
      }
    };
    addEventListener("keydown", key);
    return () => removeEventListener("keydown", key);
  }, []);

  return (
    <StackItem
      className="filter-search"
      onCompositionStart={() => setComposing(true)}
      onCompositionEnd={() => setComposing(false)}
    >
      <TextInput
        ref={input}
        type="search"
        label={t("nav.search")}
        isLabelHidden
        placeholder={t("filter.search")}
        size="sm"
        width="100%"
        value={value}
        onChange={setValue}
        onEnter={() => query !== filters.q && apply({ q: query })}
        startIcon="search"
        enterKeyHint="search"
      />
    </StackItem>
  );
}
