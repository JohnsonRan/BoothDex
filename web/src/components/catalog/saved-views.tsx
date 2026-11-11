"use client";
// The saved-views bar of the Filterable Table template (page table-filter `savedViewsBar`, page.tsx:3495-3581): it
// takes the filter row's place while the saved-views toggle is on. Loaded on first use (lib/lazy), not with the
// page.

import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { HStack, StackItem } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { ToggleButton } from "@astryxdesign/core/ToggleButton";
import { useUi } from "@/components/providers";
import "@/lib/icons-on-demand";
import { CONTROL_SIZE, toggleStyle } from "./filter-style";
import type { Saved } from "./saved-searches";

// savedViewChip (:1438): in a scrolling nowrap row a chip keeps its width instead of squeezing
const CHIP = { flexShrink: 0 } as const;

export function SavedViewsBar({
  items,
  active,
  onApply,
  onEdit,
}: {
  items: Saved[] | null;
  /** the saved search the page shows now, if any */
  active: Saved | undefined;
  /** null: the All chip, no saved search */
  onApply: (saved: Saved | null) => void;
  onEdit: (saved: Saved) => void;
}) {
  const { t } = useUi();
  return (
    <HStack gap={2} vAlign="center" wrap="nowrap" minHeight={32}>
      {/* outside the scroller, so the row keeps saying what it is (savedViewsLabel, :1411) */}
      <Text type="label" style={{ flexShrink: 0 }}>
        {t("saved.barLabel")}
      </Text>
      {/* savedViewsSlot + savedViewsScroller (:1417-1434): scrolled, not folded, so every name stays browsable */}
      <StackItem style={{ flexShrink: 1 }}>
        <HStack
          gap={2}
          vAlign="center"
          wrap="nowrap"
          style={{
            overflowX: "auto",
            overflowY: "hidden",
            overscrollBehaviorInline: "contain",
            scrollbarWidth: "none",
          }}
        >
          <ToggleButton
            label={t("saved.all")}
            size={CONTROL_SIZE}
            isPressed={!active}
            style={{ ...CHIP, ...toggleStyle(!active) }}
            onPressedChange={() => onApply(null)}
          />
          {(items ?? []).map((saved) => {
            const isPressed = active?.id === saved.id;
            return (
              <ToggleButton
                key={saved.id}
                label={saved.name}
                size={CONTROL_SIZE}
                isPressed={isPressed}
                icon={<Icon icon="site:saved" size="sm" />}
                style={{ ...CHIP, ...toggleStyle(isPressed) }}
                onPressedChange={(next) => onApply(next ? saved : null)}
              />
            );
          })}
          {active && (
            <IconButton
              label={t("saved.edit")}
              tooltip={t("saved.edit")}
              variant="ghost"
              size={CONTROL_SIZE}
              icon={<Icon icon="site:edit" size="sm" />}
              style={CHIP}
              onClick={() => onEdit(active)}
            />
          )}
        </HStack>
      </StackItem>
    </HStack>
  );
}
