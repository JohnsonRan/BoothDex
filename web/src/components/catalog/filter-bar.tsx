"use client";
// The Filterable Table template's filter row (page table-filter `filterBar`, block ToolbarTableFilter): the
// clauses on one line, folding from the end into a "+N" control as the row narrows, then the readout (Clear
// all). The fold opens a Popover with a pointer and a BottomSheet on a narrow or touch screen, as the
// template's FilterOverflow; on phones the sheet ends with Clear and "Show {n} items".

import { Button } from "@astryxdesign/core/Button";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { Link } from "@astryxdesign/core/Link";
import {
  type OverflowItem,
  OverflowList,
} from "@astryxdesign/core/OverflowList";
import { Popover } from "@astryxdesign/core/Popover";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import dynamic from "next/dynamic";
import { useEffect, useState } from "react";
import { useUi } from "@/components/providers";
import { activeFilterCount, FILTER_RESET } from "@/lib/catalog";
import { preloadWhenIdle } from "@/lib/preload";
import { LOCALES } from "@/lib/settings-core";
import { useClauses } from "./filter-clauses";
import {
  CONTROL_SIZE,
  type Facets,
  FILTER_CHROME,
  FILTER_SURFACE,
  useFacets,
} from "./filter-style";
import { useFilterUrl } from "./filter-url";

// The sheet's code loads after the page on phones (lib/preload), not with it.
const loadSheet = () => import("@astryxdesign/core/BottomSheet");
const BottomSheet = dynamic(() => loadSheet().then((m) => m.BottomSheet), {
  ssr: false,
});

// Below `md`, or a touch tablet, the fold opens a sheet (the template's COMPACT_SURFACE_QUERY).
const COMPACT_SURFACE_QUERY =
  "(max-width: 768px), (max-width: 1024px) and (pointer: coarse) and (hover: none)";
// The template's SHEET_TOP_INSET: sheet content starts below the grab pill's band.
const SHEET_TOP_INSET = 6;
// Width the slot holds back for the readout after the clauses (see the template's READOUT_RESERVE): paid as
// padding, which the overflow measurement subtracts, and taken back by the readout's negative margin.
const READOUT_RESERVE = 88;

export function FilterBar({ facets }: { facets: Promise<Facets> }) {
  const { t } = useUi();
  const { filters, apply } = useFilterUrl();
  const isCompactSurface = useMediaQuery(COMPACT_SURFACE_QUERY);
  const settled = useFacets(facets);
  const clauses = useClauses(settled);
  const hasFilters = activeFilterCount(filters) > 0 || !!filters.author;

  return (
    <HStack gap={2} vAlign="center" wrap="nowrap">
      {/* The slot, not the list, carries the sizing (template: filterOverflowSlot): it takes the row's width,
          and the list, observing it, folds what does not fit. */}
      <StackItem
        size="fill"
        style={{
          display: "flex",
          alignItems: "center",
          minWidth: 0,
          overflow: "hidden",
          containerType: "inline-size",
          paddingInlineEnd: READOUT_RESERVE,
        }}
      >
        <OverflowList
          behavior="observeParent"
          gap={2}
          // one clause stays where there is width for it; a phone folds the whole set behind the count
          minVisibleItems={isCompactSurface ? 0 : 1}
          style={{
            flexGrow: 0,
            flexShrink: 0,
            width: "max-content",
            maxWidth: "100%",
          }}
          overflowRenderer={(items) => (
            <FilterOverflow
              items={items}
              isCompactSurface={isCompactSurface}
              total={settled?.count}
            />
          )}
        >
          {clauses}
        </OverflowList>
        <HStack
          gap={2}
          vAlign="center"
          wrap="nowrap"
          style={{
            flexShrink: 0,
            whiteSpace: "nowrap",
            marginInlineStart: 8,
            marginInlineEnd: -READOUT_RESERVE,
          }}
        >
          {hasFilters && (
            // href-less, so Link renders a button: this resets the filters, it does not navigate
            <Link
              type="supporting"
              onClick={() => apply({ ...FILTER_RESET, author: "" })}
            >
              {t("filter.clear")}
            </Link>
          )}
        </HStack>
      </StackItem>
    </HStack>
  );
}

function FilterOverflow({
  items,
  isCompactSurface,
  total,
}: {
  items: OverflowItem[];
  isCompactSurface: boolean;
  /** the result count, once the search answers */
  total?: number;
}) {
  const { t, ui } = useUi();
  const { apply } = useFilterUrl();
  const [isOpen, setIsOpen] = useState(false);
  // mounted from the first open on, so later closes still animate
  const [wasOpened, setWasOpened] = useState(false);
  // focus enters the panel only when it was opened from the keyboard: a click leaves it on the trigger, so
  // the first clause does not light up as focused
  const [byKeyboard, setByKeyboard] = useState(false);
  const count = items.length;
  const label = t("filter.more", { n: count });
  useEffect(() => {
    if (isCompactSurface) preloadWhenIdle(loadSheet);
  }, [isCompactSurface]);

  // each control fills the panel: a column of content-width triggers reads as a ragged edge
  const body = (
    <VStack
      gap={2}
      padding={isCompactSurface ? 4 : 0}
      paddingBlockStart={isCompactSurface ? SHEET_TOP_INSET : 0}
    >
      {items.map(({ child, index }) => (
        <StackItem
          key={index}
          style={{ display: "flex", flexDirection: "column" }}
        >
          {child}
        </StackItem>
      ))}
      {isCompactSurface && (
        <HStack gap={2} hAlign="between" vAlign="center" paddingBlockStart={2}>
          <Button
            label={t("filter.clear")}
            variant="ghost"
            onClick={() => apply({ ...FILTER_RESET, author: "" })}
          />
          <Button
            label={t("filter.show", {
              n: (total ?? 0).toLocaleString(LOCALES[ui]),
            })}
            variant="primary"
            onClick={() => setIsOpen(false)}
          />
        </HStack>
      )}
    </VStack>
  );

  const trigger = (
    <Button
      label={`+${count}`}
      tooltip={label}
      aria-label={label}
      variant="ghost"
      size={CONTROL_SIZE}
      style={{ ...FILTER_CHROME, ...FILTER_SURFACE }}
      onPointerDown={() => setByKeyboard(false)}
      onKeyDown={() => setByKeyboard(true)}
      onClick={
        isCompactSurface
          ? () => {
              setWasOpened(true);
              setIsOpen(true);
            }
          : undefined
      }
    />
  );

  if (isCompactSurface) {
    return (
      <>
        {trigger}
        {wasOpened && (
          <BottomSheet
            isOpen={isOpen}
            onOpenChange={setIsOpen}
            label={label}
            height="hug"
            padding={0}
          >
            {body}
          </BottomSheet>
        )}
      </>
    );
  }
  return (
    <Popover
      placement="below"
      alignment="end"
      width={280}
      label={label}
      hasAutoFocus={byKeyboard}
      content={body}
    >
      {trigger}
    </Popover>
  );
}
