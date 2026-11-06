"use client";
// The site search once its code has loaded (search-box.tsx shows a plain field until then): an Astryx Tokenizer
// with block TypeaheadSearchField's search icon and suggestions on focus (TypeaheadSearchField.tsx:33-41) at the
// filter row's `sm` size. An empty field offers the recent searches; typing offers /api/palette's groups, each row a
// TypeaheadItem with a leading picture and a muted second line (the component's own example, TypeaheadItem.tsx
// :112-122). Each word of the applied query is a token, as on Mercari: its × (or Backspace) searches without it.
// Enter with no row highlighted adds the typed words and searches; the API matches every word either way. Once the
// search answers, the words it read as a model or a smart category (its `recognized`) are one token, led by the
// model's picture or a category glyph and described as what they were read as.

import { Avatar } from "@astryxdesign/core/Avatar";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { VStack } from "@astryxdesign/core/Stack";
import { Token } from "@astryxdesign/core/Token";
import { Tokenizer } from "@astryxdesign/core/Tokenizer";
import { Tooltip } from "@astryxdesign/core/Tooltip";
import { TypeaheadItem } from "@astryxdesign/core/Typeahead";
import { type RefObject, useEffect, useMemo, useRef } from "react";
import { useItemOpener, useRates } from "@/components/item/item-host";
import { useUi } from "@/components/providers";
import { markTokens, queryWords } from "@/lib/catalog";
import { loadHistory, saveHistory } from "@/lib/history";
import { CONTROL_SIZE, type SearchMark } from "./filter-style";
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
  category: "site:category",
} as const;

/** Avatar's box in an `sm` token, the size of the token's own glyphs */
const AVATAR_TOKEN = 16;

/** 1 search row + /api/palette's 5 avatars, 8 items, 5 shops and 5 tags (crates/api/src/main.rs `palette`) */
const MAX_ROWS = 24;
/** the quick search's debounce, which /api/palette's rate limit is set for (crates/api/src/limit.rs) */
const DEBOUNCE_MS = 120;
/** The selectors' height in every state. Typeahead's sm is a minimum height around its padded input, and a touch
 *  screen's 16 px input outgrows it (32 px against 28 px, 40 against 35 at a larger font); TextInput and Selector
 *  pin sm as a height. controlStyle styles the control box (Field/fieldParts.d.ts:34-46). */
const FIELD_HEIGHT = { height: "var(--size-element-sm)" } as const;

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
  marks,
}: {
  inputRef: RefObject<HTMLInputElement | null>;
  /** the plain field's state when this one replaced it: focused, and the text typed into it */
  handover: { isFocused: boolean; text: string };
  /** what the search read the query's words as, once it answers */
  marks?: SearchMark[];
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

  // the position keeps a repeated word its own token, and the ids clear of the rows' (search-source.ts)
  const [tokens, markOf] = useMemo(() => {
    const words = markTokens(queryWords(filters.q), marks ?? []);
    const ids = words.map((w, i) => `w${i}:${w.label}`);
    return [
      words.map((w, i): Row => ({ id: ids[i], label: w.label })),
      new Map(words.flatMap((w, i) => (w.mark ? [[ids[i], w.mark]] : []))),
    ] as const;
  }, [filters.q, marks]);
  /** the query's words with `more` after them */
  const plus = (more: string) =>
    [...tokens.map((w) => w.label), ...queryWords(more)].join(" ");

  // takes over from the plain field once, where the person left it: focused, with the words typed after the
  // query's in the input (text edited inside the query is typed in whole, after its tokens)
  const taken = useRef(false);
  useEffect(() => {
    const input = inputRef.current;
    if (taken.current || !input) return;
    taken.current = true;
    if (!handover.isFocused) return;
    input.click();
    const typed = handover.text.trim();
    if (typed !== filters.q)
      type(
        input,
        typed.startsWith(filters.q) ? typed.slice(filters.q.length) : typed,
      );
  }, [handover, inputRef, filters.q]);

  // a removed word searches the rest in a new field (its `key`): the focus stays, so Backspace goes on removing
  const refocus = useRef(false);
  // biome-ignore lint/correctness/useExhaustiveDependencies: runs once the new query's field is in
  useEffect(() => {
    if (!refocus.current) return;
    refocus.current = false;
    inputRef.current?.focus();
  }, [filters.q]);

  const pick = (row: Row) => {
    const p = row.auxiliaryData?.pick;
    if (!p) return;
    if (p.kind === "search")
      // the typed text: a fast Enter may land on the row of the text before the last keystrokes
      return search(plus(inputRef.current?.value || p.value));
    if (p.kind === "word") return search(plus(p.value));
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
    <Tokenizer<Row>
      // a new query starts a new field, so no text typed for the previous one is left behind its tokens
      key={filters.q}
      ref={inputRef}
      label={t("nav.search")}
      isLabelHidden
      placeholder={t("filter.search")}
      size={CONTROL_SIZE}
      controlStyle={FIELD_HEIGHT}
      width="100%"
      startIcon="search"
      searchSource={source}
      value={tokens}
      // a picked row acts and is never kept as a token; a removed token (or Clear all) searches the rest
      onChange={(words, change) => {
        if (change.type === "add") return pick(change.item);
        refocus.current = true;
        apply({ q: words.map((w) => w.label).join(" ") });
      }}
      hasClear
      // the unfocused field is the selectors' width: its tokens stay on one line there
      tokenOverflowBehavior="unfocusedInline"
      // the inline list on phones too, as the Typeahead had: its rows act at once, where the sheet saves a draft
      presentation="popover"
      hasEntriesOnFocus
      maxMenuItems={MAX_ROWS}
      debounceMs={DEBOUNCE_MS}
      emptySearchText={t("suggest.empty")}
      renderToken={(word, remove) => {
        const mark = markOf.get(word.id);
        const token = (
          <Token label={word.label} size={CONTROL_SIZE} onRemove={remove} />
        );
        if (!mark) return token;
        // what the search read the words as: "Avatar: Mao", "Categories: Hair · Short"
        const read = t("filter.clause", {
          field: t(
            mark.kind === "model" ? "filter.avatar" : "filter.categories",
          ),
          value: mark.name,
        });
        return (
          <Tooltip content={read}>
            <Token
              label={word.label}
              size={CONTROL_SIZE}
              onRemove={remove}
              description={read}
              icon={
                mark.kind === "model" ? (
                  <Avatar
                    src={mark.image}
                    name={mark.name}
                    size={AVATAR_TOKEN}
                    tooltip={false}
                  />
                ) : (
                  <Icon icon={ICONS.category} size="sm" color="secondary" />
                )
              }
            />
          </Tooltip>
        );
      }}
      onKeyDown={(e) => {
        // Enter on a highlighted row picks it (the Tokenizer's own handling); keys on the tokens are their own
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
        search(plus(input.value));
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
