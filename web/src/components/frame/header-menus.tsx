"use client";
// The top bar's language and currency menus (header-controls.tsx), apart so their DropdownMenu code loads after
// the page rather than with it.

import {
  DropdownMenu,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
} from "@astryxdesign/core/DropdownMenu";
import { useUi } from "@/components/providers";
import { useLateMenu } from "@/lib/late-menu";
import { MENU_MAX_HEIGHT } from "@/lib/menu-height";
import { LANG_NAMES, UI_LANGS, type UiLang } from "@/lib/messages";
import { CURRENCIES, type Currency } from "@/lib/settings-core";
import {
  CURRENCY_SYMBOLS,
  LanguageButton,
  useCurrencyButton,
} from "./header-controls";

export function LanguageMenu() {
  const { t, settings, update } = useUi();
  const late = useLateMenu("language");
  return (
    <DropdownMenu
      placement="below"
      alignment="end"
      menuMaxHeight={MENU_MAX_HEIGHT}
      renderTrigger={(props) => <LanguageButton {...props} {...late} />}
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

export function CurrencyMenu() {
  const { t, update } = useUi();
  const { button, names } = useCurrencyButton();
  const late = useLateMenu("currency");
  return (
    <DropdownMenu
      placement="below"
      alignment="end"
      hasChevron={false}
      button={{ ...button, ...late }}
    >
      <DropdownMenuRadioGroup
        label={t("settings.currency")}
        value={button.children}
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
