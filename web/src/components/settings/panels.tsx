"use client";
// The settings dialog's panels, filled into template settings-dialog's: Appearance as its AppearancePanel's
// card (page.tsx:1269-1306: the colour theme cards, then a second row, here the Astryx theme's cards), Language & region as its LanguagePanel
// (page.tsx:1797-1885, a Selector per row; more than three choices each), and About with master's credits
// (settings-menu.tsx:128-157) as rows whose control opens the link (a Button: an action, not a value). The
// values are the same settings the top-bar menus change (Providers' `update`), so both always agree. Signed in,
// Devices lists the account's sessions as rows of the same card, each with a Sign out Button but this one,
// which carries a Token instead.

import { Banner } from "@astryxdesign/core/Banner";
import { Button } from "@astryxdesign/core/Button";
import { Icon } from "@astryxdesign/core/Icon";
import { Selector } from "@astryxdesign/core/Selector";
import { Spinner } from "@astryxdesign/core/Spinner";
import { Switch } from "@astryxdesign/core/Switch";
import { Timestamp } from "@astryxdesign/core/Timestamp";
import { Token } from "@astryxdesign/core/Token";
import { useCallback, useEffect, useState } from "react";
import { useAccount } from "@/components/account/session";
import { CURRENCY_SYMBOLS } from "@/components/frame/header-controls";
import { useUi } from "@/components/providers";
import { useToast } from "@/components/toasts";
import { api, errorKey } from "@/lib/api";
import { type AccountSession, deviceName } from "@/lib/device";
import {
  API_ERRORS,
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

export type PanelId = "appearance" | "language" | "devices" | "about";

type PanelGroup = {
  label: MessageKey;
  panels: {
    id: PanelId;
    label: MessageKey;
    description: MessageKey;
    icon: `site:${string}` | "info";
  }[];
  /** shown only to a signed-in visitor */
  signedIn?: true;
};

/** The one list both shells read (page.tsx:372-450): the side navigation's sections and the tab strip. */
export const PANEL_GROUPS: PanelGroup[] = [
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
    label: "settings.account",
    signedIn: true,
    panels: [
      {
        id: "devices",
        label: "settings.devices",
        description: "settings.devicesDesc",
        icon: "site:devices",
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
  const toast = useToast();
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
            onChange={async (style) => {
              if (!(await update({ style })))
                toast({
                  body: t("settings.styleFailed"),
                  type: "error",
                  uniqueID: "settings.styleFailed",
                });
            }}
          />
        }
      />
      <SettingsRow
        title={t("settings.black")}
        icon="site:dark"
        description={t("settings.blackDesc")}
        control={
          <Switch
            label={t("settings.black")}
            isLabelHidden
            value={settings.black === "on"}
            onChange={(on) => update({ black: on ? "on" : "off" })}
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

/** this site's own pages, opened in place (app/privacy, app/terms) */
const LEGAL = [
  {
    title: "legal.privacy",
    description: "legal.privacyDesc",
    icon: "site:privacy",
    href: "/privacy",
  },
  {
    title: "legal.terms",
    description: "legal.termsDesc",
    icon: "site:terms",
    href: "/terms",
  },
] as const;

const LINKS = [
  {
    title: "settings.feedback",
    icon: "site:feedback",
    href: "https://github.com/JohnsonRan/BoothDex/issues",
  },
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
      {LEGAL.map((link) => (
        <SettingsRow
          key={link.href}
          title={t(link.title)}
          icon={link.icon}
          description={t(link.description)}
          control={
            <Button
              label={t("settings.open")}
              variant="secondary"
              size="sm"
              href={link.href}
            />
          }
        />
      ))}
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

/** The groups this visitor sees. */
export function usePanelGroups(): PanelGroup[] {
  const signedIn = !!useAccount().me;
  return PANEL_GROUPS.filter((g) => signedIn || !g.signedIn);
}

function DevicesPanel() {
  const { t } = useUi();
  const toast = useToast();
  const [sessions, setSessions] = useState<AccountSession[]>();
  const [failed, setFailed] = useState(false);
  const [busy, setBusy] = useState<number>();

  const load = useCallback(
    () =>
      api<{ sessions: AccountSession[] }>("/api/account/sessions").then(
        (r) => {
          setSessions(r.sessions);
          setFailed(false);
        },
        () => setFailed(true),
      ),
    [],
  );
  useEffect(() => {
    load();
  }, [load]);

  const signOut = async (id: number) => {
    setBusy(id);
    try {
      await api(`/api/account/sessions/${id}`, { method: "DELETE" });
      toast({ body: t("settings.signedOutDevice"), uniqueID: "devices" });
      await load();
    } catch (e) {
      toast({
        type: "error",
        body: t(errorKey(e, API_ERRORS, "error.failed")),
        uniqueID: "devices",
      });
    } finally {
      setBusy(undefined);
    }
  };

  if (failed)
    return <Banner status="error" title={t("settings.devicesFailed")} />;
  if (!sessions) return <Spinner label={t("settings.devices")} />;
  return (
    <SettingsCard>
      {sessions.map((s) => (
        <SettingsRow
          key={s.id}
          title={deviceName(s.user_agent) ?? t("settings.unknownDevice")}
          icon="site:devices"
          description={
            <>
              {s.ip && `${s.ip} · `}
              <Timestamp
                value={s.created_at}
                format="relative"
                type="supporting"
                color="secondary"
              />
            </>
          }
          control={
            s.current ? (
              <Token size="sm" label={t("settings.thisDevice")} />
            ) : (
              <Button
                label={t("settings.signOutDevice")}
                variant="secondary"
                size="sm"
                isLoading={busy === s.id}
                isDisabled={busy !== undefined}
                onClick={() => signOut(s.id)}
              />
            )
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
    case "devices":
      return <DevicesPanel />;
    case "about":
      return <AboutPanel />;
  }
}
