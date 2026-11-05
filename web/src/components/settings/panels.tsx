"use client";
// The settings dialog's panels, filled into template settings-dialog's: Appearance as its AppearancePanel's
// first row (page.tsx:1269-1289, the colour theme cards), Language & region as its LanguagePanel
// (page.tsx:1797-1885, a Selector per row; more than three choices each), and About with master's credits
// (settings-menu.tsx:128-157) as rows whose control opens the link (a Button: an action, not a value). The
// values are the same settings the top-bar menus change (Providers' `update`), so both always agree.

import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { Selector } from "@astryxdesign/core/Selector";
import { CURRENCY_SYMBOLS } from "@/components/frame/header-controls";
import { useUi } from "@/components/providers";
import {
  MESSAGES,
  type MessageKey,
  UI_LANGS,
  type UiLang,
} from "@/lib/messages";
import {
  CONTENT_LANGS,
  type ContentLang,
  CURRENCIES,
  type Currency,
  LOCALES,
} from "@/lib/settings-core";
import { CONTROL_WIDTH, SettingsCard, SettingsRow } from "./rows";
import { ThemeChoiceCards } from "./theme-choice";

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
          UI_LANGS.map((l) => ({ value: l, label: MESSAGES[l]["lang.name"] })),
          (v) => update({ ui: v }),
        )}
      />
      <SettingsRow
        title={t("settings.content")}
        icon="site:itemText"
        description={t("settings.contentDesc")}
        control={selector<ContentLang>(
          "settings.content",
          settings.content,
          CONTENT_LANGS.map((l) => ({
            value: l,
            label:
              l === "original"
                ? t("settings.contentOriginal")
                : MESSAGES[l]["lang.name"],
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
  {
    title: "settings.fontCredit",
    icon: "site:itemText",
    href: "https://hyperos.mi.com/font/",
  },
] as const;

function AboutPanel() {
  const { t } = useUi();
  return (
    <SettingsCard>
      <SettingsRow
        title="BoothDex"
        icon="info"
        description="© 2026 BoothDex contributors · AGPL-3.0-or-later"
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
