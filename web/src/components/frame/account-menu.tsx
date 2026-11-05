"use client";
// The signed-in account button (the display name's initial) and its menu, loaded only once an account is found
// (account.tsx), so a signed-out page does not ship Avatar.

import { Avatar } from "@astryxdesign/core/Avatar";
import {
  DropdownMenu,
  DropdownMenuGroup,
  DropdownMenuItem,
} from "@astryxdesign/core/DropdownMenu";
import { IconButton } from "@astryxdesign/core/IconButton";
import { useUi } from "@/components/providers";
import { api } from "@/lib/api";

export default function AccountMenu({
  name,
  onSignedOut,
}: {
  name: string;
  onSignedOut: () => void;
}) {
  const { t } = useUi();
  return (
    <DropdownMenu
      placement="below"
      alignment="end"
      renderTrigger={(props) => (
        <IconButton
          {...props}
          label={t("nav.account")}
          tooltip={name}
          variant="ghost"
          icon={<Avatar name={name} size="sm" tooltip={false} />}
        />
      )}
    >
      <DropdownMenuGroup title={name}>
        <DropdownMenuItem
          label={t("account.signOut")}
          onClick={async () => {
            await api("/api/account/logout", { method: "POST" }).catch(
              () => {},
            );
            onSignedOut();
          }}
        />
      </DropdownMenuGroup>
    </DropdownMenu>
  );
}
