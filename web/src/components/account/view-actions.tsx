"use client";
// The favourites and recent views' own actions at the end of the title row, as master's (browse.tsx:546-569):
// Clear history (recent view, while it has items) and Back to all, ghost buttons with an icon.

import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { HStack, StackItem } from "@astryxdesign/core/Stack";
import { useFilterUrl } from "@/components/catalog/filter-url";
import { useUi } from "@/components/providers";
import { useAccount } from "./session";

export function ViewActions() {
  const { t } = useUi();
  const { filters, apply } = useFilterUrl();
  const { viewed, clearViewed } = useAccount();
  return (
    <StackItem size="fill">
      <HStack gap={1} hAlign="end">
        {filters.view === "recent" && !!viewed?.length && (
          <Button
            label={t("recent.clear")}
            variant="ghost"
            icon={<Icon icon="site:clear" size="sm" />}
            onClick={clearViewed}
          />
        )}
        <Button
          label={t("title.backToAll")}
          variant="ghost"
          icon={<Icon icon="close" size="sm" />}
          onClick={() => apply({ view: "items" })}
        />
      </HStack>
    </StackItem>
  );
}
