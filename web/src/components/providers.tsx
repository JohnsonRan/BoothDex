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
  use,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  useTransition,
} from "react";
import { defaultThemeSlug, themes } from "@/astryx-themes";
import "@/lib/icons";
import type { Dict, UiLang } from "@/lib/messages";
import { savePreferences } from "@/lib/preferences";
import {
  LOCALES,
  parseSettings,
  SETTINGS_COOKIE,
  type Settings,
  type Style,
  shownMode,
  type Translate,
  translate,
} from "@/lib/settings-core";
import { ToastHost } from "./toasts";

type Ui = {
  ui: UiLang;
  t: Translate;
  settings: Settings;
  /** saves a change in the cookie for a year and applies it without a reload */
  update: (patch: Partial<Settings>) => void;
};
const UiContext = createContext<Ui | null>(null);

/** master's localStorage key for the settings, before they moved to the cookie */
const LEGACY_SETTINGS = "settings";

type Look = Pick<Settings, "theme" | "style">;

let onDemand: Promise<typeof import("@/themes") | null> | undefined;
/** The other themes' code (themes/index.tsx), loaded once a page shows or chooses one; null if it cannot load. */
const loadThemes = () => (onDemand ??= import("@/themes").catch(() => null));

/** Puts a colour mode and theme on <html> and the root Theme's element at once, ahead of the render that follows
 *  (globals.css lets these attributes alone set the colour scheme). */
function showLook({ theme, style }: Look) {
  const html = document.documentElement;
  for (const el of [html, html.querySelector("body [data-astryx-theme]")]) {
    if (!el) continue;
    const mode = shownMode(theme, style);
    if (mode === "system") el.removeAttribute("data-theme");
    else el.setAttribute("data-theme", mode);
    el.setAttribute("data-astryx-theme", style);
  }
}

export function Providers({
  settings: fromServer,
  messages,
  catalogue,
  children,
}: {
  settings: Settings;
  /** the site's strings in the visitor's language (lib/dictionaries.ts) */
  messages: Dict;
  /** the component library's strings for this language (English is built in) */
  catalogue?: Record<string, string>;
  children: React.ReactNode;
}) {
  const router = useRouter();
  const [, startTransition] = useTransition();
  // the mode and theme apply at once on the client; the rest is rendered on the server
  const [look, setLook] = useState<Look>({
    theme: fromServer.theme,
    style: fromServer.style,
  });
  const lazy = look.style === "neutral" ? null : use(loadThemes());
  const theme = lazy?.styleTheme(look.style) ?? themes[defaultThemeSlug];
  const locale = LOCALES[fromServer.ui];
  // what the cookie holds: a second change made before the first one's render arrives builds on it
  const written = useRef(fromServer);
  // the theme last asked for: an earlier one that finishes loading later is not shown
  const asked = useRef<Style>(fromServer.style);
  const value = useMemo<Ui>(() => {
    const settings = { ...fromServer, ...look };
    const save = (patch: Partial<Settings>) => {
      const next = { ...written.current, ...patch };
      written.current = next;
      // biome-ignore lint/suspicious/noDocumentCookie: the server reads this cookie; Cookie Store is not in every browser
      document.cookie = `${SETTINGS_COOKIE}=${encodeURIComponent(JSON.stringify(next))}; path=/; max-age=31536000; samesite=lax`;
      // signed in, the account keeps them too (lib/preferences.ts)
      savePreferences(patch);
      if (patch.theme || patch.style) {
        const shown = { theme: next.theme, style: next.style };
        showLook(shown);
        // the new look then reaches every Icon and setting in a transition: an urgent update hydrated the page's
        // pending content (site-frame's Activity) in one long task, up to 1 s at 4x CPU
        startTransition(() => setLook(shown));
      }
      if (patch.ui || patch.content || patch.currency)
        startTransition(() => router.refresh());
    };
    return {
      ui: settings.ui,
      t: translate(messages),
      settings,
      update: ({ style, ...patch }) => {
        if (Object.keys(patch).length) save(patch);
        if (!style) return;
        asked.current = style;
        // shown once its stylesheet, object and fonts are in; one that cannot load leaves the page as it is
        (style === "neutral"
          ? Promise.resolve(true)
          : loadThemes().then((m) => m?.prepareStyle(style) ?? false)
        ).then((ok) => {
          if (ok && asked.current === style) save({ style });
        });
      },
    };
  }, [fromServer, messages, look, router]);

  // the language the page is rendered in, also when it came from Accept-Language rather than the cookie
  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

  // visitors from before the cookie existed: their settings move over from localStorage once, as on master
  const { update } = value;
  useEffect(() => {
    try {
      const old = localStorage.getItem(LEGACY_SETTINGS);
      if (old === null) return;
      localStorage.removeItem(LEGACY_SETTINGS);
      if (
        document.cookie
          .split("; ")
          .some((c) => c.startsWith(`${SETTINGS_COOKIE}=`))
      )
        return;
      const saved = parseSettings(old, navigator.language);
      const patch = Object.fromEntries(
        Object.entries(saved).filter(
          ([k, v]) => written.current[k as keyof Settings] !== v,
        ),
      );
      if (Object.keys(patch).length) update(patch);
    } catch {}
  }, [update]);

  return (
    <Theme theme={theme} mode={shownMode(look.theme, look.style)}>
      {/* the theme shown: one that failed to load shows neutral, which needs none */}
      {lazy && <lazy.ThemeSheet name={theme.name} />}
      <InternationalizationProvider
        locale={locale}
        messages={catalogue ? { [locale]: catalogue } : undefined}
      >
        <LinkProvider component={Link}>
          {/* the static layout cannot know a language read from Accept-Language: set while the page is parsed,
              not by the effect after hydration, whose change restyles the whole document */}
          <script
            // biome-ignore lint/security/noDangerouslySetInnerHtml: static inline script, no user input
            dangerouslySetInnerHTML={{
              __html: `document.documentElement.lang=${JSON.stringify(locale)}`,
            }}
          />
          <UiContext value={value}>
            <ToastHost>{children}</ToastHost>
          </UiContext>
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
