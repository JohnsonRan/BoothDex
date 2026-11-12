"use client";
// The avatar strip's toggle, last in the filter toolbar's end cluster (toolbar.tsx), so it stays in one place as
// the strip shrinks under the toolbar or opens over the results. An icon button as its neighbours, the table-filter
// template's toolbar actions (page.tsx:4439-4446 save, :4450-4468 saved views: ghost `sm`, the name as label and
// tooltip), with the chevron of Collapsible's trigger anatomy, turned up to fold. The band (band.tsx) owns the
// strip's open state and hands it here (block `CollapsibleHookUsage` :14-42: aria-expanded and aria-controls on the
// trigger); without a strip there is no toggle.

import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { createContext, useContext } from "react";
import { CONTROL_SIZE } from "@/components/catalog/filter/filter-style";
import { useUi } from "@/components/providers";

export type StripToggleState = {
  /** shown at full size: the toggle folds it */
  expanded: boolean;
  /** the cards' id */
  controls: string;
  onToggle: () => void;
};

export const StripToggleContext = createContext<StripToggleState | null>(null);

const TURNED = { transform: "rotate(180deg)" };

export function StripToggle() {
  const { t } = useUi();
  const strip = useContext(StripToggleContext);
  if (!strip) return null;
  const label = t(strip.expanded ? "strip.collapse" : "strip.expand");
  return (
    <IconButton
      label={label}
      tooltip={label}
      variant="ghost"
      size={CONTROL_SIZE}
      icon={
        <Icon
          icon="chevronDown"
          size="sm"
          style={strip.expanded ? TURNED : undefined}
        />
      }
      aria-expanded={strip.expanded}
      aria-controls={strip.controls}
      onClick={strip.onToggle}
    />
  );
}
