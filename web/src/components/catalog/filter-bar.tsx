"use client";
// The Filterable Table template's filter row (page table-filter `filterBar`, page.tsx:3408-3493): the search
// leading the row, then the clauses on one line, folding from the end into a "+N" control as the row narrows
// (`FilterOverflow`, :2034-2105: a Popover with a pointer, a BottomSheet on a compact surface), then the readout
// "{n} 件 • Clear all" right after the last clause.

import { Button } from "@astryxdesign/core/Button";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { Link } from "@astryxdesign/core/Link";
import {
  type OverflowItem,
  OverflowList,
} from "@astryxdesign/core/OverflowList";
import { Popover } from "@astryxdesign/core/Popover";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import dynamic from "next/dynamic";
import { type ReactNode, useEffect, useLayoutEffect, useState } from "react";
import { useUi } from "@/components/providers";
import { activeFilterCount, FILTER_RESET } from "@/lib/catalog";
import { preloadWhenIdle } from "@/lib/preload";
import type { ClauseNames } from "./clause-text";
import { useClauses } from "./filter-clauses";
import {
  COMPACT_SURFACE_QUERY,
  CONTROL_SIZE,
  type Facets,
  FILTER_CHROME,
  FILTER_SURFACE,
  READOUT_RESERVE,
  SHEET_TOP_INSET,
  useFacets,
} from "./filter-style";
import { useFilterUrl } from "./filter-url";
import { SearchBox } from "./search-box";

// The sheet's code loads after the page on compact surfaces (lib/preload), not with it.
const loadSheet = () => import("@astryxdesign/core/BottomSheet");
const BottomSheet = dynamic(() => loadSheet().then((m) => m.BottomSheet), {
  ssr: false,
});

export function FilterBar({
  facets,
  names,
  count,
}: {
  facets: Promise<Facets>;
  names: ClauseNames;
  /** the result count, rendered on the server when the search answers */
  count: ReactNode;
}) {
  const { t } = useUi();
  const { filters: f, apply } = useFilterUrl();
  const isCompactSurface = useMediaQuery(COMPACT_SURFACE_QUERY);
  const clauses = useClauses(useFacets(facets), names);
  // The server's row holds every clause, so the readout first lands past them; the list folds them while
  // hydrating, before that frame is painted. Until then the readout is laid out but not shown: appearing in place
  // is not a layout shift, moving would be.
  const [isFolded, setIsFolded] = useState(false);
  useLayoutEffect(() => setIsFolded(true), []);
  // the template's hasFilters (:2958): any clause or a search
  const hasFilters =
    activeFilterCount(f) > 0 || !!f.q || !!f.author || !!f.avatar;

  return (
    // filterRow (:1329): one line at every width
    <HStack
      gap={2}
      vAlign="center"
      wrap="nowrap"
      minHeight={32}
      style={{ rowGap: 6 }}
    >
      <SearchBox />
      {/* filterOverflowSlot (:1350-1372): the slot, not the list, carries the sizing — the row's only grower, the
          first to give (shrink 100 against the search's 1), floored at the "+N" trigger plus the reserve */}
      <StackItem
        style={{
          display: "flex",
          alignItems: "center",
          flexGrow: 1,
          flexShrink: 100,
          paddingInlineEnd: READOUT_RESERVE,
          minWidth: `calc(44px + ${READOUT_RESERVE})`,
        }}
      >
        <OverflowList
          behavior="observeParent"
          gap={2}
          // one clause stays where there is width for it; a phone folds the whole set behind the count (:3447-3452)
          minVisibleItems={isCompactSurface ? 0 : 1}
          overflowRenderer={(items) => (
            <FilterOverflow items={items} isCompactSurface={isCompactSurface} />
          )}
        >
          {clauses}
        </OverflowList>
        {/* filterMeta (:1406-1410): draws over the reserve, against the last clause */}
        <HStack
          gap={2}
          vAlign="center"
          // the count's place, read by tools/lab.cjs
          data-readout
          style={{
            flexShrink: 0,
            marginInlineStart: 8,
            marginInlineEnd: `calc(-1 * ${READOUT_RESERVE})`,
            visibility: isFolded ? undefined : "hidden",
          }}
        >
          {count}
          {hasFilters && (
            <>
              <Text type="supporting" color="secondary">
                •
              </Text>
              {/* href-less, so Link renders a button: this resets the filters, it does not navigate */}
              <Link
                type="supporting"
                onClick={() =>
                  apply({ ...FILTER_RESET, q: "", author: "", avatar: "" })
                }
              >
                {t("filter.clearAll")}
              </Link>
            </>
          )}
        </HStack>
      </StackItem>
    </HStack>
  );
}

function FilterOverflow({
  items,
  isCompactSurface,
}: {
  items: OverflowItem[];
  isCompactSurface: boolean;
}) {
  const { t } = useUi();
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

  // each control fills the panel (overflowItem, :1375): a column of content-width triggers reads as a ragged edge
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
