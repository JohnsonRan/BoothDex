"use client";
// The top bar's preference controls, as on the Astryx site and doona lite (ColorModeToggle, LanguageMenu): ghost
// IconButtons with a tooltip; the menus are DropdownMenu radio groups. Every change is saved in the settings
// cookie and applies without a reload (Providers' `update`).

import {
  DropdownMenu,
  DropdownMenuDivider,
  DropdownMenuGroup,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
} from "@astryxdesign/core/DropdownMenu";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { Icon } from "@astryxdesign/core/Icon";
import { IconButton } from "@astryxdesign/core/IconButton";
import { Coins, Languages, Moon, Sun } from "lucide-react";
import { useUi } from "@/components/providers";
import { MESSAGES, UI_LANGS, type UiLang } from "@/lib/messages";
import {
  CONTENT_LANGS,
  type ContentLang,
  CURRENCIES,
  type Currency,
} from "@/lib/settings-core";

/** Interface language and item-text language, two radio groups in one menu. */
export function LanguageMenu() {
  const { t, settings, update } = useUi();
  return (
    <DropdownMenu
      placement="below"
      alignment="end"
      renderTrigger={(props) => (
        <IconButton
          {...props}
          label={t("nav.language")}
          tooltip={t("nav.language")}
          variant="ghost"
          icon={<Icon icon={Languages} size="sm" />}
        />
      )}
    >
      <DropdownMenuGroup title={t("settings.ui")}>
        <DropdownMenuRadioGroup
          label={t("settings.ui")}
          value={settings.ui}
          onChange={(v) => update({ ui: v as UiLang })}
        >
          {UI_LANGS.map((l) => (
            <DropdownMenuRadioItem
              key={l}
              value={l}
              label={MESSAGES[l]["lang.name"]}
            />
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuGroup>
      <DropdownMenuDivider />
      <DropdownMenuGroup title={t("settings.content")}>
        <DropdownMenuRadioGroup
          label={t("settings.content")}
          value={settings.content}
          onChange={(v) => update({ content: v as ContentLang })}
        >
          {CONTENT_LANGS.map((l) => (
            <DropdownMenuRadioItem
              key={l}
              value={l}
              label={
                l === "original"
                  ? t("settings.contentOriginal")
                  : MESSAGES[l]["lang.name"]
              }
            />
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuGroup>
    </DropdownMenu>
  );
}

/** Display currency; prices other than yen are converted (≈) on the server. */
export function CurrencyMenu() {
  const { t, settings, update } = useUi();
  const label = t("nav.currency", { c: settings.currency });
  return (
    <DropdownMenu
      placement="below"
      alignment="end"
      renderTrigger={(props) => (
        <IconButton
          {...props}
          label={label}
          tooltip={label}
          variant="ghost"
          icon={<Icon icon={Coins} size="sm" />}
        />
      )}
    >
      <DropdownMenuRadioGroup
        label={t("settings.currency")}
        value={settings.currency}
        onChange={(v) => update({ currency: v as Currency })}
      >
        {CURRENCIES.map((c) => (
          <DropdownMenuRadioItem key={c} value={c} label={c} />
        ))}
      </DropdownMenuRadioGroup>
    </DropdownMenu>
  );
}

/** Flips light and dark relative to what is shown; without a saved choice the page follows the system. */
export function ColorModeToggle() {
  const { t, settings, update } = useUi();
  const prefersDark = useMediaQuery("(prefers-color-scheme: dark)");
  const isDark =
    settings.theme === "dark" || (settings.theme === "system" && prefersDark);
  const label = isDark ? t("theme.toLight") : t("theme.toDark");
  return (
    <IconButton
      label={label}
      tooltip={label}
      variant="ghost"
      icon={<Icon icon={isDark ? Sun : Moon} size="sm" />}
      onClick={() => update({ theme: isDark ? "light" : "dark" })}
    />
  );
}
