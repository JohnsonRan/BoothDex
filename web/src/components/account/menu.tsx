"use client";
// The signed-in account menu, master's (account-menu.tsx:95-137) in Astryx: the trigger is the bar's account
// IconButton (trigger.tsx; DropdownMenu `renderTrigger`, as the bar's other menus); the menu's header names the
// account (display name and email), and its data-driven rows form two groups: the account's lists, then
// settings, the console (admins only) and Sign out. Saved searches live in the filter row
// (catalog/toolbar.tsx), as the table-filter template's saved views.

import {
  DropdownMenu,
  type DropdownMenuOption,
} from "@astryxdesign/core/DropdownMenu";
import { Icon } from "@astryxdesign/core/Icon";
import { VStack } from "@astryxdesign/core/Stack";
import { Text } from "@astryxdesign/core/Text";
import { useFilterUrl } from "@/components/catalog/filter-url";
import { useUi } from "@/components/providers";
import { type Me, useAccount } from "./session";
import { AccountTrigger } from "./trigger";
import "@/lib/icons-on-demand";

const icon = (name: `site:${string}`) => <Icon icon={name} size="sm" />;

export default function AccountMenu({
  me,
  onSettings,
}: {
  me: Me;
  onSettings: () => void;
}) {
  const { t } = useUi();
  const { signOut } = useAccount();
  const { apply } = useFilterUrl();
  const name = me.display_name || me.username;
  const items: DropdownMenuOption[] = [
    {
      label: t("account.myFavourites"),
      icon: icon("site:favourite"),
      // the account's whole list: the search, shop, avatar and categories are cleared, as on master
      onClick: () =>
        apply({
          view: "favourites",
          avatar: "",
          q: "",
          author: "",
          sc: "",
          features: [],
        }),
    },
    {
      label: t("account.recent"),
      icon: icon("site:recent"),
      // keeps the avatar, so going back to all shows it again
      onClick: () =>
        apply({ view: "recent", q: "", author: "", sc: "", features: [] }),
    },
    { type: "divider" },
    {
      label: t("settings.title"),
      icon: icon("site:settings"),
      onClick: onSettings,
    },
    ...(me.is_admin
      ? [
          {
            label: t("account.admin"),
            icon: icon("site:admin"),
            href: "/admin",
          },
        ]
      : []),
    {
      label: t("account.signOut"),
      icon: icon("site:signOut"),
      onClick: signOut,
    },
  ];
  return (
    <DropdownMenu
      placement="below"
      alignment="end"
      items={items}
      header={
        <VStack gap={0.5}>
          <Text weight="semibold" maxLines={1}>
            {name}
          </Text>
          <Text type="supporting" color="secondary" maxLines={1}>
            {me.email}
          </Text>
        </VStack>
      }
      renderTrigger={(props) => <AccountTrigger {...props} name={name} />}
    />
  );
}
