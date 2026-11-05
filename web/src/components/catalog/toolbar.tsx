"use client";
// The Card Grid template's control row: filter tabs that narrow the grid in place (categories, or gender in
// the avatar view), then the filter panel and the sort menu.

import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { Center } from "@astryxdesign/core/Center";
import { DropdownMenu } from "@astryxdesign/core/DropdownMenu";
import { Icon } from "@astryxdesign/core/Icon";
import { OverflowList } from "@astryxdesign/core/OverflowList";
import { Popover } from "@astryxdesign/core/Popover";
import { Spinner } from "@astryxdesign/core/Spinner";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import {
  ToggleButton,
  ToggleButtonGroup,
} from "@astryxdesign/core/ToggleButton";
import dynamic from "next/dynamic";
import { useState } from "react";
import { useUi } from "@/components/providers";
import { activeFilterCount, type Filters } from "@/lib/catalog";
import type { MessageKey } from "@/lib/messages";
import { useFilterUrl } from "./filter-url";

// The panel's inputs (segmented control, checkboxes, tag search) load on first use, not with the page.
const FilterPanel = dynamic(
  () => import("./filter-panel").then((m) => m.FilterPanel),
  {
    ssr: false,
    loading: () => (
      <Center>
        <Spinner />
      </Center>
    ),
  },
);

const SORTS = [
  ["trending", "desc", "sort.trending"],
  ["newest", "desc", "sort.newest"],
  ["likes", "desc", "sort.likes"],
  ["price", "asc", "sort.priceAsc"],
  ["price", "desc", "sort.priceDesc"],
] as const satisfies readonly [Filters["sort"], Filters["dir"], MessageKey][];

export function Toolbar({
  categoryTabs,
  featureCounts,
}: {
  /** the items view's category tabs: slug and name in the interface language */
  categoryTabs: [string, string][];
  /** per feature tag, for the filter panel; resolves with the search */
  featureCounts: Promise<Record<string, number>>;
}) {
  const { t } = useUi();
  const { filters, apply } = useFilterUrl();
  const [panelOpen, setPanelOpen] = useState(false);
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
  // a text search's default order is relevance (see searchQuery)
  const sortLabel = (key: MessageKey) =>
    t(key === "sort.trending" && filters.q ? "sort.relevant" : key);
  const current =
    SORTS.find(
      ([s, d]) => s === filters.sort && (s !== "price" || d === filters.dir),
    ) ?? SORTS[0];
  const nActive = activeFilterCount(filters);

  return (
    <HStack vAlign="center" gap={4}>
      {/* the tabs take what the buttons leave: their overflow is measured in the browser, and the buttons
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
              overflowRenderer={(overflowItems) => (
                <DropdownMenu
                  button={{
                    label: `+${overflowItems.length}`,
                    variant: "ghost",
                    size: "lg",
                  }}
                  items={overflowItems.map(({ index }) => ({
                    label: tabs[index][1],
                    onClick: () => pickTab(tabs[index][0]),
                  }))}
                />
              )}
            >
              {tabs.map(([value, label]) => (
                <ToggleButton
                  key={value || "all"}
                  label={label}
                  value={value}
                  size="lg"
                />
              ))}
            </OverflowList>
          </ToggleButtonGroup>
        </VStack>
      </StackItem>
      <StackItem size="static">
        <Popover
          presentation="adaptive"
          placement="below"
          alignment="end"
          label={t("nav.filters")}
          width={360}
          isOpen={panelOpen}
          onOpenChange={setPanelOpen}
          content={
            <FilterPanel
              featureCounts={featureCounts}
              onDone={() => setPanelOpen(false)}
            />
          }
        >
          <Button
            label={
              nActive
                ? t("nav.filtersActive", { n: nActive })
                : t("nav.filters")
            }
            size="lg"
            icon={<Icon icon="funnel" />}
            endContent={nActive ? <Badge label={nActive} /> : undefined}
          >
            {t("nav.filters")}
          </Button>
        </Popover>
      </StackItem>
      <StackItem size="static">
        <DropdownMenu
          button={{ label: sortLabel(current[2]), size: "lg" }}
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
