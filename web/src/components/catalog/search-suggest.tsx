"use client";
// The site search once its code has loaded (search-box.tsx shows a plain field until then): an Astryx Typeahead as
// block TypeaheadSearchField (search icon, suggestions on focus: TypeaheadSearchField.tsx:33-41) at the filter
// row's `sm` size. An empty field offers the recent searches; typing offers /api/palette's groups, each row a
// TypeaheadItem with a leading picture and a muted second line (the component's own example, TypeaheadItem.tsx
// :112-122). The applied query is the field's value, so it shows as the Typeahead's token: a click edits it, its
// clear button drops it. Enter with no row highlighted searches the typed text.

import { Avatar } from "@astryxdesign/core/Avatar";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { VStack } from "@astryxdesign/core/Stack";
import { Typeahead, TypeaheadItem } from "@astryxdesign/core/Typeahead";
import { type RefObject, useEffect, useMemo, useRef } from "react";
import { useItemOpener, useRates } from "@/components/item/item-host";
import { useUi } from "@/components/providers";
import { loadHistory, saveHistory } from "@/lib/history";
import { CONTROL_SIZE } from "./filter-style";
import { useFilterUrl } from "./filter-url";
import { useSearch } from "./search-box";
import { type Row, searchSource } from "./search-source";
import "@/lib/icons-on-demand";

// Avatar's sm box (Avatar.d.ts:80): a row with a glyph holds it in the box a row with a picture has, so every
// row's text starts on one edge, as the component's example leads every row with an Avatar
// (TypeaheadItemShowcase.tsx:43)
const AVATAR_SM = 24;

// the theme's semantic icons where it has one (TEMPLATE-MAP rule 6), the site's own otherwise (lib/icons)
const ICONS = {
  history: "clock",
  search: "search",
  tag: "site:tag",
  clear: "site:clear",
} as const;

/** 1 search row + /api/palette's 5 avatars, 8 items, 5 shops and 5 tags (crates/api/src/main.rs `palette`) */
const MAX_ROWS = 24;
/** the quick search's debounce, which /api/palette's rate limit is set for (crates/api/src/limit.rs) */
const DEBOUNCE_MS = 120;

/**
 * Types `text` into the Typeahead's input as a person would: the component keeps the typed text to itself, so a
 * native value set plus an input event is the way in (the Typeahead's own edit mode does the same).
 */
function type(input: HTMLInputElement, text: string) {
  Object.getOwnPropertyDescriptor(
    HTMLInputElement.prototype,
    "value",
  )?.set?.call(input, text);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

export function SearchSuggest({
  inputRef,
  handover,
}: {
  inputRef: RefObject<HTMLInputElement | null>;
  /** the plain field's state when this one replaced it: focused, and the text typed into it */
  handover: { isFocused: boolean; text: string };
}) {
  const { t, settings } = useUi();
  const { filters, apply } = useFilterUrl();
  const { open: openItem } = useItemOpener();
  const rates = useRates();
  const search = useSearch();
  const source = useMemo(
    () => searchSource({ t, settings, rates, adult: filters.adult }),
    [t, settings, rates, filters.adult],
  );

  const value = useMemo(
    () => (filters.q ? { id: "q", label: filters.q } : null),
    [filters.q],
  );

  // takes over from the plain field once, where the person left it: a click on the field opens the query for
  // editing (or focuses the empty input), then any text typed meanwhile replaces it
  const taken = useRef(false);
  useEffect(() => {
    const input = inputRef.current;
    if (taken.current || !input) return;
    taken.current = true;
    if (!handover.isFocused) return;
    input.click();
    if (handover.text.trim() !== filters.q) type(input, handover.text);
  }, [handover, inputRef, filters.q]);

  const pick = (row: Row | null) => {
    // the token's clear button
    if (!row) return apply({ q: "" });
    const p = row.auxiliaryData?.pick;
    if (!p) return;
    if (p.kind === "search")
      // the typed text: a fast Enter may land on the row of the text before the last keystrokes
      return search(inputRef.current?.value || p.value);
    if (p.kind === "query") return search(p.value);
    if (p.kind === "avatar") return apply({ avatar: p.value });
    if (p.kind === "shop") return apply({ author: p.value, q: "", avatar: "" });
    if (p.kind === "item") return openItem(p.value);
    if (p.kind === "clear") saveHistory([]);
  };

  const forget = (h: string) => {
    saveHistory(loadHistory().filter((x) => x !== h));
    // the open list reloads only for new text: step through a blank, back to the empty field
    const input = inputRef.current;
    if (input) {
      type(input, " ");
      type(input, "");
    }
  };

  return (
    <Typeahead<Row>
      // a new query starts a new field, so no text typed for the previous one is left behind its token
      key={filters.q}
      ref={inputRef}
      label={t("nav.search")}
      isLabelHidden
      placeholder={t("filter.search")}
      size={CONTROL_SIZE}
      width="100%"
      startIcon="search"
      searchSource={source}
      value={value}
      onChange={pick}
      hasEntriesOnFocus
      maxMenuItems={MAX_ROWS}
      debounceMs={DEBOUNCE_MS}
      emptySearchText={t("suggest.empty")}
      onKeyDown={(e) => {
        // Enter on a highlighted row picks it (the Typeahead's own handling); keys on the token are its own
        const input = e.currentTarget;
        if (
          e.key !== "Enter" ||
          // an input method's Enter commits its text (the check of Astryx's utils/ime.ts)
          e.nativeEvent.isComposing ||
          e.keyCode === 229 ||
          e.target !== input ||
          !(input instanceof HTMLInputElement) ||
          input.getAttribute("aria-activedescendant") ||
          (!input.value.trim() && !filters.q)
        )
          return;
        // the list closes with the focus; a new query starts a new field anyway
        input.blur();
        search(input.value);
      }}
      renderItem={(row) => {
        const d = row.auxiliaryData;
        return (
          <>
            <TypeaheadItem
              item={row}
              // the hook for the list's width (app/filter-row.css)
              className="search-suggest"
              style={{ flex: 1, minWidth: 0 }}
              icon={
                d?.image !== undefined ? (
                  <Avatar
                    src={d.image || undefined}
                    name={row.label}
                    size="sm"
                    shape={d.square ? "rounded" : "circle"}
                    tooltip={false}
                  />
                ) : (
                  d?.icon && (
                    <VStack
                      width={AVATAR_SM}
                      height={AVATAR_SM}
                      hAlign="center"
                      vAlign="center"
                    >
                      <Icon icon={ICONS[d.icon]} size="sm" color="secondary" />
                    </VStack>
                  )
                )
              }
              description={d?.note}
            />
            {d?.icon === "history" && (
              <IconButton
                label={t("history.remove", { q: row.label })}
                variant="ghost"
                size="sm"
                icon={<Icon icon="close" size="sm" />}
                // not a pick: the press keeps the input's focus, the click stays out of the row's
                onMouseDown={(e) => e.preventDefault()}
                onClick={(e) => {
                  e.stopPropagation();
                  forget(row.label);
                }}
              />
            )}
          </>
        );
      }}
    />
  );
}
