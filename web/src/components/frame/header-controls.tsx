"use client";
// The top bar's preference controls, as on the Astryx site and doona lite (ColorModeToggle, LanguageMenu): ghost
// IconButtons with a tooltip; the menus are DropdownMenu radio groups. Every change is saved in the settings
// cookie and applies without a reload (Providers' `update`).

import {
  DropdownMenu,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
} from "@astryxdesign/core/DropdownMenu";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { useUi } from "@/components/providers";
import { MENU_MAX_HEIGHT } from "@/lib/menu-height";
import { LANG_NAMES, UI_LANGS, type UiLang } from "@/lib/messages";
import {
  CURRENCIES,
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

/** Interface language, a radio group (the item-text language is in Settings only). */
export function LanguageMenu() {
  const { t, settings, update } = useUi();
  return (
    <DropdownMenu
      placement="below"
      alignment="end"
      menuMaxHeight={MENU_MAX_HEIGHT}
      renderTrigger={(props) => (
        <IconButton
          {...props}
          label={t("nav.language")}
          tooltip={t("nav.language")}
          variant="ghost"
          icon={<Icon icon="site:language" size="sm" />}
        />
      )}
    >
      <DropdownMenuRadioGroup
        label={t("settings.ui")}
        value={settings.ui}
        onChange={(v) => update({ ui: v as UiLang })}
      >
        {UI_LANGS.map((l) => (
          <DropdownMenuRadioItem key={l} value={l} label={LANG_NAMES[l]} />
        ))}
      </DropdownMenuRadioGroup>
    </DropdownMenu>
  );
}

/** Display currency: the trigger is the current code alone, a ghost button of the icon buttons' size beside
 *  them (template shell-top-nav, end actions); its name and the menu rows give symbol and full name. Prices other
 *  than yen are converted (≈) on the server. */
export function CurrencyMenu() {
  const { t, ui, settings, update } = useUi();
  const names = new Intl.DisplayNames(LOCALES[ui], { type: "currency" });
  const code = settings.currency;
  const label = t("nav.currency", { name: names.of(code) ?? code, c: code });
  return (
    <DropdownMenu
      placement="below"
      alignment="end"
      hasChevron={false}
      button={{
        label,
        tooltip: label,
        variant: "ghost",
        children: code,
      }}
    >
      <DropdownMenuRadioGroup
        label={t("settings.currency")}
        value={code}
        onChange={(v) => update({ currency: v as Currency })}
      >
        {CURRENCIES.map((c) => (
          <DropdownMenuRadioItem
            key={c}
            value={c}
            label={`${CURRENCY_SYMBOLS[c]} ${c} · ${names.of(c)}`}
          />
        ))}
      </DropdownMenuRadioGroup>
    </DropdownMenu>
  );
}

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
