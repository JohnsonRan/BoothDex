"use client";
// App root for every route: the Astryx theme and component catalogue, Next.js links, and the site's own
// message lookup and preferences. Settings come from the cookie, read on the server; changing one writes the
// cookie, and the server renders the page again in the new language or currency.

import { InternationalizationProvider } from "@astryxdesign/core/i18n";
import { LinkProvider } from "@astryxdesign/core/Link";
import { Theme } from "@astryxdesign/core/theme";
import Link from "next/link";
import { useRouter } from "next/navigation";
import {
  createContext,
  useContext,
  useMemo,
  useState,
  useTransition,
} from "react";
import { defaultThemeSlug, themes } from "@/astryx-themes";
import type { UiLang } from "@/lib/messages";
import {
  LOCALES,
  SETTINGS_COOKIE,
  type Settings,
  type Translate,
  translator,
} from "@/lib/settings-core";

type Ui = {
  ui: UiLang;
  t: Translate;
  settings: Settings;
  /** saves a change in the cookie for a year and applies it without a reload */
  update: (patch: Partial<Settings>) => void;
};
const UiContext = createContext<Ui | null>(null);

export function Providers({
  settings: fromServer,
  catalogue,
  children,
}: {
  settings: Settings;
  /** the component library's strings for this language (English is built in) */
  catalogue?: Record<string, string>;
  children: React.ReactNode;
}) {
  const router = useRouter();
  const [, startTransition] = useTransition();
  // the mode applies at once on the client; the rest is rendered on the server
  const [theme, setTheme] = useState(fromServer.theme);
  const locale = LOCALES[fromServer.ui];
  const value = useMemo<Ui>(() => {
    const settings = { ...fromServer, theme };
    return {
      ui: settings.ui,
      t: translator(settings.ui),
      settings,
      update: (patch) => {
        const next = { ...settings, ...patch };
        // biome-ignore lint/suspicious/noDocumentCookie: the server reads this cookie; Cookie Store is not in every browser
        document.cookie = `${SETTINGS_COOKIE}=${encodeURIComponent(JSON.stringify(next))}; path=/; max-age=31536000; samesite=lax`;
        if (patch.theme) setTheme(patch.theme);
        if (patch.ui) document.documentElement.lang = LOCALES[patch.ui];
        if (patch.ui || patch.content || patch.currency)
          startTransition(() => router.refresh());
      },
    };
  }, [fromServer, theme, router]);
  return (
    <Theme theme={themes[defaultThemeSlug]} mode={theme}>
      <InternationalizationProvider
        locale={locale}
        messages={catalogue ? { [locale]: catalogue } : undefined}
      >
        <LinkProvider component={Link}>
          <UiContext value={value}>{children}</UiContext>
        </LinkProvider>
      </InternationalizationProvider>
    </Theme>
  );
}

/** The interface language, its message lookup and the visitor's settings. */
export function useUi(): Ui {
  const ui = useContext(UiContext);
  if (!ui) throw new Error("useUi outside Providers");
  return ui;
}
