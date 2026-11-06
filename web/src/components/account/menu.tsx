"use client";
// The signed-in account menu, master's (account-menu.tsx:95-137) in Astryx: the trigger is a ghost IconButton
// holding an Avatar with the display name's initial (DropdownMenu `renderTrigger`, as the bar's other menus);
// the rows are data-driven DropdownMenu items in a section titled with the display name (block
// DropdownMenuWithSections), dividers between the account's lists, the console and settings, and Sign out. Saved
// searches live in the filter row (catalog/toolbar.tsx), as the table-filter template's saved views.

import { Avatar } from "@astryxdesign/core/Avatar";
import {
  DropdownMenu,
  type DropdownMenuOption,
} from "@astryxdesign/core/DropdownMenu";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { useFilterUrl } from "@/components/catalog/filter-url";
import { useUi } from "@/components/providers";
import { type Me, useAccount } from "./session";
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
      type: "section",
      title: name,
      items: [
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
      ],
    },
    { type: "divider" },
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
      label: t("settings.title"),
      icon: icon("site:settings"),
      onClick: onSettings,
    },
    { type: "divider" },
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
      renderTrigger={(props) => (
        <IconButton
          {...props}
          label={t("nav.account")}
          tooltip={name}
          variant="ghost"
          icon={<Avatar name={name} size="sm" tooltip={false} />}
        />
      )}
    />
  );
}
