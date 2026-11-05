"use client";
// The Card Grid template's control row: filter tabs that narrow the grid in place (categories, or gender in
// the avatar view), then the filter panel and the sort menu.

import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { DropdownMenu } from "@astryxdesign/core/DropdownMenu";
import { Icon } from "@astryxdesign/core/Icon";
import { OverflowList } from "@astryxdesign/core/OverflowList";
import { Popover } from "@astryxdesign/core/Popover";
import { HStack, StackItem, VStack } from "@astryxdesign/core/Stack";
import {
  ToggleButton,
  ToggleButtonGroup,
} from "@astryxdesign/core/ToggleButton";
import { useState } from "react";
import { useUi } from "@/components/providers";
import {
  activeFilterCount,
  type Category,
  type Filters,
  localName,
} from "@/lib/catalog";
import type { MessageKey } from "@/lib/messages";
import { FilterPanel } from "./filter-panel";
import { useFilterUrl } from "./filter-url";

const SORTS = [
  ["trending", "desc", "sort.trending"],
  ["newest", "desc", "sort.newest"],
  ["likes", "desc", "sort.likes"],
  ["price", "asc", "sort.priceAsc"],
  ["price", "desc", "sort.priceDesc"],
] as const satisfies readonly [Filters["sort"], Filters["dir"], MessageKey][];

export function Toolbar({
  categories,
  featureCounts,
}: {
  categories: Category[];
  featureCounts: Record<string, number>;
}) {
  const { t, ui } = useUi();
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
    : [
        ["", t("filter.all")],
        ...categories
          .filter((c) => c.mode === "items")
          .map((c): [string, string] => [c.slug, localName(c, ui)]),
      ];
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
      <StackItem size="fill">
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
            nActive ? t("nav.filtersActive", { n: nActive }) : t("nav.filters")
          }
          size="lg"
          icon={<Icon icon="funnel" />}
          endContent={nActive ? <Badge label={nActive} /> : undefined}
        >
          {t("nav.filters")}
        </Button>
      </Popover>
      <DropdownMenu
        button={{ label: sortLabel(current[2]), size: "lg" }}
        alignment="end"
        items={SORTS.map(([sort, dir, key]) => ({
          label: sortLabel(key),
          onClick: () => apply({ sort, dir }),
        }))}
      />
    </HStack>
  );
}
