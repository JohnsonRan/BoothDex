"use client";
// The top bar's preference controls, as on the Astryx site and doona lite (ColorModeToggle, LanguageMenu): ghost
// IconButtons with a tooltip; the menus are DropdownMenu radio groups. Every change is saved in the settings
// cookie and applies without a reload (Providers' `update`).

import { Button } from "@astryxdesign/core/Button";
import type { DropdownMenuTriggerProps } from "@astryxdesign/core/DropdownMenu";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import dynamic from "next/dynamic";
import { useUi } from "@/components/providers";
import { lateMenuButton } from "@/lib/late-menu";
import {
  type Currency,
  LOCALES,
  SINGLE_MODE,
  shownMode,
} from "@/lib/settings-core";

// the sign each currency is known by locally (Intl's narrow symbols repeat "$" and "¥" across currencies)
export const CURRENCY_SYMBOLS: Record<Currency, string> = {
  JPY: "¥",
  USD: "$",
  EUR: "€",
  GBP: "£",
  CNY: "¥",
  TWD: "NT$",
  HKD: "HK$",
  KRW: "₩",
  AUD: "A$",
  CAD: "C$",
  SGD: "S$",
  THB: "฿",
};

const loadMenus = () => import("./header-menus");

/** The language menu's button: a ghost IconButton like its neighbours. */
export function LanguageButton(props: Partial<DropdownMenuTriggerProps>) {
  const { t } = useUi();
  return (
    <IconButton
      {...props}
      label={t("nav.language")}
      tooltip={t("nav.language")}
      variant="ghost"
      icon={<Icon icon="site:language" size="sm" />}
    />
  );
}

/** The currency menu's button: the current code alone, a ghost button of the icon buttons' size beside them
 *  (template shell-top-nav, end actions); its name and the menu rows give symbol and full name. Prices other
 *  than yen are converted (≈) on the server. */
export function useCurrencyButton() {
  const { t, ui, settings } = useUi();
  const names = new Intl.DisplayNames(LOCALES[ui], { type: "currency" });
  const code = settings.currency;
  const label = t("nav.currency", { name: names.of(code) ?? code, c: code });
  return {
    names,
    button: {
      label,
      tooltip: label,
      variant: "ghost",
      children: code,
    } as const,
  };
}

function CurrencyButton() {
  return (
    <Button {...useCurrencyButton().button} {...lateMenuButton("currency")} />
  );
}

// The menus' code loads after the page, as the account menu's (account-button.tsx); until then each shows the
// button it opens from, which keeps a press for the menu (lib/late-menu.ts).

/** Interface language, a radio group (the item-text language is in Settings only). */
export const LanguageMenu = dynamic(
  () => loadMenus().then((m) => m.LanguageMenu),
  {
    ssr: false,
    loading: () => <LanguageButton {...lateMenuButton("language")} />,
  },
);

/** Display currency, a radio group. */
export const CurrencyMenu = dynamic(
  () => loadMenus().then((m) => m.CurrencyMenu),
  { ssr: false, loading: () => <CurrencyButton /> },
);

/** Flips light and dark relative to what is shown; without a saved choice the page follows the system. */
export function ColorModeToggle() {
  const { t, settings, update } = useUi();
  const prefersDark = useMediaQuery("(prefers-color-scheme: dark)");
  const mode = shownMode(settings.theme, settings.style);
  const isDark = mode === "dark" || (mode === "system" && prefersDark);
  const label = isDark ? t("theme.toLight") : t("theme.toDark");
  // a theme drawn in one mode: disabled, its tooltip saying why (IconButton.doc.mjs:55-58, aria-disabled so it
  // stays focusable while it has a tooltip)
  const single = !!SINGLE_MODE[settings.style];
  return (
    <IconButton
      label={label}
      tooltip={single ? t("settings.darkOnly") : label}
      isDisabled={single}
      variant="ghost"
      icon={<Icon icon={isDark ? "site:light" : "site:dark"} size="sm" />}
      onClick={() => update({ theme: isDark ? "light" : "dark" })}
    />
  );
}
