"use client";

import type { DropdownMenuTriggerProps } from "@astryxdesign/core/DropdownMenu";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { useUi } from "@/components/providers";

/** The signed-in control: a ghost IconButton like its neighbours, named by the display name once known. */
export function AccountTrigger({
  name,
  ...props
}: Partial<DropdownMenuTriggerProps> & { name?: string }) {
  const { t } = useUi();
  return (
    <IconButton
      {...props}
      label={t("nav.account")}
      tooltip={name ?? t("nav.account")}
      variant="ghost"
      icon={<Icon icon="site:account" size="sm" />}
    />
  );
}
