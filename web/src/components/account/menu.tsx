"use client";
// The signed-in account menu, master's (account-menu.tsx:95-137) in Astryx: the trigger is the bar's account
// IconButton (trigger.tsx; DropdownMenu `renderTrigger`, as the bar's other menus); its data-driven rows form
// two groups: the account's lists, titled with the display name, then settings, the console (admins only) and
// Sign out. Saved searches live in the filter row
// (catalog/toolbar.tsx), as the table-filter template's saved views.

import {
  DropdownMenu,
  type DropdownMenuItemData,
  type DropdownMenuOption,
} from "@astryxdesign/core/DropdownMenu";
import { Icon } from "@astryxdesign/core/Icon";
import { useFilterUrl } from "@/components/catalog/filter/filter-url";
import { useUi } from "@/components/providers";
import { type Me, useAccount } from "./session";
import { AccountTrigger } from "./trigger";
import "@/lib/icons-on-demand";
import { useLateMenu } from "@/lib/late-menu";
import { MENU_MAX_HEIGHT } from "@/lib/menu-height";

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
  // a section title sits on the rows' inset; the menu's `header` is for controls and has none
  const lists: DropdownMenuItemData[] = [
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
  ];
  const items: DropdownMenuOption[] = [
    {
      type: "section",
      title: name,
      items: lists,
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
  const late = useLateMenu("account");
  return (
    <DropdownMenu
      placement="below"
      alignment="end"
      // the default 300 px cap scrolled the admin's six rows at the page's 115 % scale; a short menu should show
      // whole, and the viewport still bounds it (lib/menu-height)
      menuMaxHeight={MENU_MAX_HEIGHT}
      items={items}
      renderTrigger={(props) => (
        <AccountTrigger {...props} {...late} name={name} />
      )}
    />
  );
}
