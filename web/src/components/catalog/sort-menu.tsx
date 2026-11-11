"use client";
// The toolbar's sort (toolbar.tsx), a radio group as the top bar's language and currency menus
// (frame/header-menus.tsx), apart so its DropdownMenu code loads after the page.

import {
  DropdownMenu,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
} from "@astryxdesign/core/DropdownMenu";
import { useUi } from "@/components/providers";
import type { Filters } from "@/lib/catalog";
import { useLateMenu } from "@/lib/late-menu";
import { MENU_MAX_HEIGHT } from "@/lib/menu-height";
import { SORTS, sortKey, sortLabel } from "./clause-text";
import { sortButton } from "./toolbar";

export function SortMenu({
  filters,
  apply,
}: {
  filters: Filters;
  apply: (patch: Partial<Filters>) => void;
}) {
  const { t } = useUi();
  const late = useLateMenu("sort");
  return (
    <DropdownMenu
      button={{ ...sortButton(filters, t), ...late }}
      alignment="end"
      menuMaxHeight={MENU_MAX_HEIGHT}
    >
      <DropdownMenuRadioGroup
        label={t("sort.label")}
        value={sortKey(filters)}
        onChange={(v) => {
          const picked = SORTS.find(([, , key]) => key === v);
          if (picked) apply({ sort: picked[0], dir: picked[1] });
        }}
      >
        {SORTS.map(([, , key]) => (
          <DropdownMenuRadioItem
            key={key}
            value={key}
            label={sortLabel(filters, t, key)}
          />
        ))}
      </DropdownMenuRadioGroup>
    </DropdownMenu>
  );
}
