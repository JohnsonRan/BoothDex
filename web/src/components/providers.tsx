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

type Look = Pick<Settings, "theme" | "black">;

/** Puts a colour mode on <html> and the root Theme's element at once, ahead of the render that follows
 *  (globals.css lets these attributes alone set the colour scheme). */
function showLook({ theme, black }: Look) {
  const html = document.documentElement;
  html.toggleAttribute("data-black", black === "on");
  for (const el of [html, html.querySelector("body [data-astryx-theme]")]) {
    if (!el) continue;
    if (theme === "system") el.removeAttribute("data-theme");
    else el.setAttribute("data-theme", theme);
  }
  showChrome();
}

/** Gives the browser's bar (theme-color, layout.tsx viewport) the background of the page's AppShell as now shown. */
function showChrome() {
  const shell = document.querySelector(".astryx-app-shell");
  if (!shell) return;
  const color = getComputedStyle(shell).backgroundColor;
  for (const meta of document.querySelectorAll('meta[name="theme-color"]'))
    meta.setAttribute("content", color);
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
  // the mode applies at once on the client; the rest is rendered on the server
  const [look, setLook] = useState<Look>({
    theme: fromServer.theme,
    black: fromServer.black,
  });
  const locale = LOCALES[fromServer.ui];
  // what the cookie holds: a second change made before the first one's render arrives builds on it
  const written = useRef(fromServer);
  const value = useMemo<Ui>(() => {
    const settings = { ...fromServer, ...look };
    const save = (patch: Partial<Settings>) => {
      const next = { ...written.current, ...patch };
      written.current = next;
      // biome-ignore lint/suspicious/noDocumentCookie: the server reads this cookie; Cookie Store is not in every browser
      document.cookie = `${SETTINGS_COOKIE}=${encodeURIComponent(JSON.stringify(next))}; path=/; max-age=31536000; samesite=lax`;
      // signed in, the account keeps them too (lib/preferences.ts)
      savePreferences(patch);
      if (patch.theme || patch.black) {
        const shown = { theme: next.theme, black: next.black };
        showLook(shown);
        // the new look then reaches every Icon and setting in a transition: an urgent update hydrated the page's
        // pending content (site-frame's Activity) in one long task, up to 1 s at 4x CPU
        startTransition(() => setLook(shown));
      }
      // the filters' place is the server's layout (catalog/home-screen.tsx)
      if (patch.ui || patch.content || patch.currency || patch.filters)
        startTransition(() => router.refresh());
    };
    return {
      ui: settings.ui,
      t: translate(messages),
      settings,
      update: save,
    };
  }, [fromServer, messages, look, router]);

  // the bar follows the mode the page opened in (a pinned one, from the head script or the server) and then the
  // system's mode while that is followed
  const mode = look.theme;
  useEffect(() => {
    showChrome();
    if (mode !== "system") return;
    const dark = matchMedia("(prefers-color-scheme: dark)");
    dark.addEventListener("change", showChrome);
    return () => dark.removeEventListener("change", showChrome);
  }, [mode]);

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
    <Theme theme={themes[defaultThemeSlug]} mode={mode}>
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
