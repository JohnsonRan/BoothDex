"use client";
// The settings dialog's panels, filled into template settings-dialog's: Appearance as its AppearancePanel's
// card (page.tsx:1269-1306: the colour theme cards, then a second row, here the Astryx theme's cards), Language & region as its LanguagePanel
// (page.tsx:1797-1885, a Selector per row; more than three choices each), and About with master's credits
// (settings-menu.tsx:128-157) as rows whose control opens the link (a Button: an action, not a value). The
// values are the same settings the top-bar menus change (Providers' `update`), so both always agree.

import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { Selector } from "@astryxdesign/core/Selector";
import { CURRENCY_SYMBOLS } from "@/components/frame/header-controls";
import { useUi } from "@/components/providers";
import {
  LANG_NAMES,
  type MessageKey,
  UI_LANGS,
  type UiLang,
} from "@/lib/messages";
import {
  CURRENCIES,
  type Currency,
  ITEM_TEXT,
  type ItemTextSetting,
  LOCALES,
} from "@/lib/settings-core";
import { CONTROL_WIDTH, SettingsCard, SettingsRow } from "./rows";
import { StyleChoiceCards, ThemeChoiceCards } from "./theme-choice";

export type PanelId = "appearance" | "language" | "about";

/** The one list both shells read (page.tsx:372-450): the side navigation's sections and the tab strip. */
export const PANEL_GROUPS: {
  label: MessageKey;
  panels: {
    id: PanelId;
    label: MessageKey;
    description: MessageKey;
    icon: `site:${string}` | "info";
  }[];
}[] = [
  {
    label: "settings.preferences",
    panels: [
      {
        id: "appearance",
        label: "settings.appearance",
        description: "settings.appearanceDesc",
        icon: "site:appearance",
      },
      {
        id: "language",
        label: "settings.language",
        description: "settings.languageDesc",
        icon: "site:language",
      },
    ],
  },
  {
    label: "settings.site",
    panels: [
      {
        id: "about",
        label: "settings.about",
        description: "settings.aboutDesc",
        icon: "info",
      },
    ],
  },
];

function AppearancePanel() {
  const { t, settings, update } = useUi();
  return (
    <SettingsCard>
      <SettingsRow
        title={t("settings.theme")}
        icon="site:appearance"
        description={t("settings.syncNote")}
        detail={
          <ThemeChoiceCards
            value={settings.theme}
            onChange={(theme) => update({ theme })}
          />
        }
      />
      <SettingsRow
        title={t("settings.style")}
        icon="site:style"
        description={t("settings.styleDesc")}
        detail={
          <StyleChoiceCards
            value={settings.style}
            onChange={(style) => update({ style })}
          />
        }
      />
    </SettingsCard>
  );
}

function LanguagePanel() {
  const { t, ui, settings, update } = useUi();
  const names = new Intl.DisplayNames(LOCALES[ui], { type: "currency" });
  const selector = <T extends string>(
    label: MessageKey,
    value: T,
    options: { value: T; label: string }[],
    onChange: (v: T) => void,
  ) => (
    <Selector
      label={t(label)}
      isLabelHidden
      size="sm"
      width={CONTROL_WIDTH}
      value={value}
      onChange={(v) => onChange(v as T)}
      options={options}
    />
  );
  return (
    <SettingsCard>
      <SettingsRow
        title={t("settings.ui")}
        icon="site:language"
        description={t("settings.uiDesc")}
        control={selector<UiLang>(
          "settings.ui",
          settings.ui,
          UI_LANGS.map((l) => ({ value: l, label: LANG_NAMES[l] })),
          (v) => update({ ui: v }),
        )}
      />
      <SettingsRow
        title={t("settings.content")}
        icon="site:itemText"
        description={t("settings.contentDesc")}
        control={selector<ItemTextSetting>(
          "settings.content",
          settings.content,
          ITEM_TEXT.map((l) => ({
            value: l,
            label:
              l === "ui"
                ? t("settings.contentUi")
                : l === "original"
                  ? t("settings.contentOriginal")
                  : LANG_NAMES[l],
          })),
          (v) => update({ content: v }),
        )}
      />
      <SettingsRow
        title={t("settings.currency")}
        icon="site:currency"
        description={t("settings.currencyDesc")}
        control={selector<Currency>(
          "settings.currency",
          settings.currency,
          CURRENCIES.map((c) => ({
            value: c,
            label: `${CURRENCY_SYMBOLS[c]} ${c} · ${names.of(c)}`,
          })),
          (v) => update({ currency: v }),
        )}
      />
    </SettingsCard>
  );
}

const LINKS = [
  {
    title: "settings.source",
    icon: "site:code",
    href: "https://github.com/JohnsonRan/BoothDex",
  },
] as const;

function AboutPanel() {
  const { t } = useUi();
  return (
    <SettingsCard>
      <SettingsRow
        title="BoothDex"
        icon="info"
        description={t("settings.copyright")}
      />
      {LINKS.map((link) => (
        <SettingsRow
          key={link.href}
          title={t(link.title)}
          icon={link.icon}
          description={link.href.replace(/^https:\/\/|\/$/g, "")}
          control={
            <Button
              label={t("settings.open")}
              variant="secondary"
              size="sm"
              icon={<Icon icon="externalLink" size="sm" />}
              href={link.href}
              target="_blank"
              rel="noopener"
            />
          }
        />
      ))}
    </SettingsCard>
  );
}

export function SettingsPanelBody({ panel }: { panel: PanelId }) {
  switch (panel) {
    case "appearance":
      return <AppearancePanel />;
    case "language":
      return <LanguagePanel />;
    case "about":
      return <AboutPanel />;
  }
}
