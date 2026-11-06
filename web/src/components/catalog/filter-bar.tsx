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
import { type ReactNode, useEffect, useLayoutEffect, useState } from "react";
import { useUi } from "@/components/providers";
import { activeFilterCount, FILTER_RESET } from "@/lib/catalog";
import { Lazy, lazySurface, preloadWhenIdle } from "@/lib/lazy";
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

// The sheet's code loads after the page on compact surfaces (lib/lazy), not with it.
const BottomSheet = lazySurface(() =>
  import("@astryxdesign/core/BottomSheet").then((m) => m.BottomSheet),
);

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
  const isCompactSurface = useMediaQuery(COMPACT_SURFACE_QUERY);
  const clauses = useClauses(useFacets(facets), names);
  // The server cannot measure, so its row holds every clause; the list folds them while hydrating. Until then the
  // list spans the slot, as a folded list does (OverflowList's fillParent), keeps back the fold trigger's place
  // and wraps the clauses that do not fit out of sight (app/filter-row.css), so the clauses shown and the readout
  // already sit where they will stay. The readout is laid out but not shown until the fold.
  const [isFolded, setIsFolded] = useState(false);
  useLayoutEffect(() => setIsFolded(true), []);

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
          minWidth: `calc(2.75rem + ${READOUT_RESERVE})`,
        }}
      >
        <OverflowList
          behavior="observeParent"
          gap={2}
          className="filter-clauses"
          data-unfolded={isFolded ? undefined : ""}
          // one clause stays where there is width for it; a phone folds the whole set behind the count (:3447-3452)
          minVisibleItems={isCompactSurface ? 0 : 1}
          overflowRenderer={(items) => (
            <FilterOverflow items={items} isCompactSurface={isCompactSurface} />
          )}
        >
          {clauses}
        </OverflowList>
        {/* filterMeta (:1406-1410): draws over the reserve, against the last clause; the count and "Clear all"
            arrive together (results.tsx), so nothing in it moves when the count replaces its silhouette */}
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
        </HStack>
      </StackItem>
    </HStack>
  );
}

/** The readout's "• Clear all" (:3478-3491), after the count; with no count (a failed search), "Clear all" alone. */
export function ClearAll({ afterCount }: { afterCount: boolean }) {
  const { t } = useUi();
  const { filters: f, apply } = useFilterUrl();
  // the template's hasFilters (:2958): any clause or a search
  if (!(activeFilterCount(f) > 0 || f.q || f.author || f.avatar)) return null;
  return (
    <>
      {afterCount && (
        <Text type="supporting" color="secondary">
          •
        </Text>
      )}
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
    if (isCompactSurface) preloadWhenIdle(BottomSheet.preload);
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
          <Lazy
            surface={BottomSheet}
            isOpen={isOpen}
            onOpenChange={setIsOpen}
            label={label}
            height="hug"
            padding={0}
          >
            {body}
          </Lazy>
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
