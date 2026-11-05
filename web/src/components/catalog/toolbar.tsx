"use client";
// The Card Grid template's control row: filter tabs that narrow the grid in place (categories, or gender in
// the avatar view), then the sort menu. The other filters are the clause row under it (filter-bar.tsx).

import { DropdownMenu } from "@astryxdesign/core/DropdownMenu";
import { OverflowList } from "@astryxdesign/core/OverflowList";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import {
  ToggleButton,
  ToggleButtonGroup,
} from "@astryxdesign/core/ToggleButton";
import { useUi } from "@/components/providers";
import type { Filters } from "@/lib/catalog";
import type { MessageKey } from "@/lib/messages";
import { LOCALES } from "@/lib/settings-core";
import {
  CONTROL_SIZE,
  type Facets,
  FILTER_FILL,
  useFacets,
} from "./filter-style";
import { useFilterUrl } from "./filter-url";

const SORTS = [
  ["trending", "desc", "sort.trending"],
  ["newest", "desc", "sort.newest"],
  ["likes", "desc", "sort.likes"],
  ["price", "asc", "sort.priceAsc"],
  ["price", "desc", "sort.priceDesc"],
] as const satisfies readonly [Filters["sort"], Filters["dir"], MessageKey][];

export function Toolbar({
  categoryTabs,
  facets,
}: {
  /** the items view's category tabs: slug and name in the interface language */
  categoryTabs: [string, string][];
  /** the category counts arrive with the search; the tabs never wait for them */
  facets: Promise<Facets>;
}) {
  const { t, ui } = useUi();
  const { filters, apply } = useFilterUrl();
  const smart = useFacets(facets)?.smart;
  const avatars = filters.view === "avatars";
  // the tabs' values; "" is the first tab, meaning no restriction
  const tabs: [string, string][] = avatars
    ? [
        ["", t("filter.genderAny")],
        ["female", t("filter.female")],
        ["male", t("filter.male")],
      ]
    : [["", t("filter.all")], ...categoryTabs];
  const tab = avatars ? filters.gender : filters.sc;
  const pickTab = (v: string) =>
    apply(avatars ? { gender: v as Filters["gender"] } : { sc: v });
  // a category's item count for the current search, shown where it costs no layout: tooltip and menu
  const count = (v: string) =>
    !avatars && v && smart ? smart[v]?.toLocaleString(LOCALES[ui]) : undefined;
  // a text search's default order is relevance (see searchQuery)
  const sortLabel = (key: MessageKey) =>
    t(key === "sort.trending" && filters.q ? "sort.relevant" : key);
  const current =
    SORTS.find(
      ([s, d]) => s === filters.sort && (s !== "price" || d === filters.dir),
    ) ?? SORTS[0];

  return (
    <HStack vAlign="center" gap={4}>
      {/* the tabs take what the sort menu leaves: their overflow is measured in the browser, and the menu
          must not move when it is (no layout shift on hydration) */}
      <StackItem size="fill" style={{ minWidth: 0 }}>
        <VStack>
          <ToggleButtonGroup
            label={avatars ? t("filter.gender") : t("filter.categories")}
            value={tab}
            onChange={(v) => pickTab(typeof v === "string" ? v : "")}
          >
            <OverflowList
              gap={1}
              behavior="observeParent"
              overflowRenderer={(overflowItems) => {
                // a folded selected tab names the overflow control, so the choice stays visible on a phone
                const folded = overflowItems.find(
                  ({ index }) => tab && tabs[index][0] === tab,
                );
                return (
                  <DropdownMenu
                    button={{
                      label: folded
                        ? tabs[folded.index][1]
                        : `+${overflowItems.length}`,
                      variant: "ghost",
                      size: CONTROL_SIZE,
                      style: folded ? FILTER_FILL : undefined,
                    }}
                    items={overflowItems.map(({ index }) => ({
                      label: tabs[index][1],
                      endContent: count(tabs[index][0]),
                      onClick: () =>
                        pickTab(tabs[index][0] === tab ? "" : tabs[index][0]),
                    }))}
                  />
                );
              }}
            >
              {tabs.map(([value, label]) => (
                <ToggleButton
                  key={value || "all"}
                  label={label}
                  value={value}
                  size={CONTROL_SIZE}
                  tooltip={count(value)}
                />
              ))}
            </OverflowList>
          </ToggleButtonGroup>
        </VStack>
      </StackItem>
      <StackItem size="static">
        <DropdownMenu
          button={{ label: sortLabel(current[2]), size: CONTROL_SIZE }}
          alignment="end"
          items={SORTS.map(([sort, dir, key]) => ({
            label: sortLabel(key),
            onClick: () => apply({ sort, dir }),
          }))}
        />
      </StackItem>
    </HStack>
  );
}
