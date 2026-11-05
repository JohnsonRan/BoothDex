"use client";
// App root for every route: the Astryx theme and component catalogue, Next.js links, and the site's own
// message lookup. Mode and language come from the settings cookie, read on the server.

import { InternationalizationProvider } from "@astryxdesign/core/i18n";
import { LinkProvider } from "@astryxdesign/core/Link";
import { Theme } from "@astryxdesign/core/theme";
import Link from "next/link";
import { createContext, useContext, useMemo } from "react";
import { defaultThemeSlug, themes } from "@/astryx-themes";
import type { UiLang } from "@/lib/messages";
import {
  LOCALES,
  type Theme as ThemeMode,
  type Translate,
  translator,
} from "@/lib/settings-core";

type Ui = { ui: UiLang; t: Translate };
const UiContext = createContext<Ui | null>(null);

export function Providers({
  ui,
  mode,
  catalogue,
  children,
}: {
  ui: UiLang;
  mode: ThemeMode;
  /** the component library's strings for this language (English is built in) */
  catalogue?: Record<string, string>;
  children: React.ReactNode;
}) {
  const locale = LOCALES[ui];
  const value = useMemo(() => ({ ui, t: translator(ui) }), [ui]);
  return (
    <Theme theme={themes[defaultThemeSlug]} mode={mode}>
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

/** The interface language and its message lookup. */
export function useUi(): Ui {
  const ui = useContext(UiContext);
  if (!ui) throw new Error("useUi outside Providers");
  return ui;
}
