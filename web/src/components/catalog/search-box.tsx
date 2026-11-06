"use client";
// The site search, leading the filter row as the table-filter template's search (page.tsx:3419-3430): an `sm`
// field in its own slot, which sits at the selectors' width and widens on focus (`searchSlot`, :1500-1521;
// app/filter-row.css). Its suggestions are a Typeahead (search-suggest.tsx) whose code loads when the page is idle
// or on the first focus; until then the field is a plain TextInput, where Enter applies the query. Ctrl/Cmd+K
// focuses the field; in the recent view, which has no filter row, it goes to the items view first.

import { StackItem } from "@astryxdesign/core/Stack";
import { TextInput } from "@astryxdesign/core/TextInput";
import { useEffect, useRef, useState } from "react";
import { useUi } from "@/components/providers";
import { recordQuery } from "@/lib/history";
import { preloadWhenIdle } from "@/lib/lazy";
import { CONTROL_SIZE } from "./filter-style";
import { useFilterUrl } from "./filter-url";

const loadSuggest = () => import("./search-suggest");
type Suggest = Awaited<ReturnType<typeof loadSuggest>>["SearchSuggest"];

/** Set by the shortcut where there is no field: the field takes focus when it next mounts. */
let focusOnMount = false;

/** Runs `run` on Ctrl/Cmd+K. */
function useShortcut(run: () => void) {
  const latest = useRef(run);
  latest.current = run;
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        latest.current();
      }
    };
    addEventListener("keydown", key);
    return () => removeEventListener("keydown", key);
  }, []);
}

/** Applies a search and records it in the recent searches. */
export function useSearch() {
  const { filters, apply } = useFilterUrl();
  return (text: string) => {
    const q = text.trim();
    if (q) recordQuery(q);
    if (q !== filters.q) apply({ q });
  };
}

/** Ctrl/Cmd+K in a view without the filter row: the items view, with the search focused. */
export function SearchShortcut() {
  const { apply } = useFilterUrl();
  useShortcut(() => {
    focusOnMount = true;
    apply({ view: "items" });
  });
  return null;
}

export function SearchBox() {
  const { t } = useUi();
  const { filters } = useFilterUrl();
  const search = useSearch();
  const input = useRef<HTMLInputElement>(null);
  const [suggest, setSuggest] = useState<{
    Field: Suggest;
    handover: { isFocused: boolean; text: string };
  }>();
  const [text, setText] = useState(filters.q);
  // the URL's query changed by something else (Clear all, Back, a saved search): the plain field shows it
  const [shown, setShown] = useState(filters.q);
  if (filters.q !== shown) {
    setShown(filters.q);
    setText(filters.q);
  }

  useEffect(() => preloadWhenIdle(loadSuggest), []);
  useEffect(() => {
    if (!focusOnMount) return;
    focusOnMount = false;
    input.current?.focus();
  }, []);
  // a click is how the Typeahead is entered: it opens an applied query for editing
  useShortcut(() =>
    suggest ? input.current?.click() : input.current?.focus(),
  );

  if (suggest)
    return (
      <StackItem className="filter-search">
        <suggest.Field inputRef={input} handover={suggest.handover} />
      </StackItem>
    );

  const activate = () =>
    loadSuggest().then(({ SearchSuggest }) =>
      setSuggest({
        Field: SearchSuggest,
        // read when the code is here, so focus and text typed while it loaded carry over
        handover: {
          isFocused: document.activeElement === input.current,
          text: input.current?.value ?? "",
        },
      }),
    );

  return (
    <StackItem className="filter-search">
      <TextInput
        ref={input}
        type="search"
        label={t("nav.search")}
        isLabelHidden
        placeholder={t("filter.search")}
        size={CONTROL_SIZE}
        width="100%"
        value={text}
        onChange={setText}
        onFocus={activate}
        onEnter={() => search(text)}
        startIcon="search"
        enterKeyHint="search"
      />
    </StackItem>
  );
}
