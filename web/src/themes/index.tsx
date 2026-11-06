"use client";
// The Astryx themes besides neutral (lib/settings-core.ts STYLES), each loaded on demand. Astryx's pattern (the
// generated astryx-themes.ts, `astryx theme add`) imports every theme into every page; here a neutral visitor
// loads none of them, nor this module (components/providers.tsx imports it on demand). A theme's module (themes/<slug>.ts: its built object, theme.css and fonts) loads when it is
// chosen or when Settings previews it.
// - A visitor who chose one gets it from the server render: the root renders ThemeSheet, a next/dynamic component,
//   so the page carries the theme's stylesheet and React reveals the page only once it is in (stylesheet
//   precedence); the root waits for the theme object before it renders (styleTheme).
// - Fonts are self-hosted from Fontsource in place of the theme's Google Fonts @import, Latin only (CJK keeps the
//   system fonts later in each stack), and `font-display: optional`: a face not in by the first paint is skipped
//   rather than swapped in, so nothing shifts. A switch loads the faces first (prepareStyle); a returning visitor
//   has them cached. A preview draws no text, so it loads no font.

import type { DefinedTheme } from "@astryxdesign/core/theme";
import dynamic from "next/dynamic";
import { use } from "react";
import type { Style } from "@/lib/settings-core";

type ThemeModule = { theme: DefinedTheme; fonts: string[] };
type OnDemand = Exclude<Style, "neutral">;

/** One load per theme; a failed one settles as null, which shows neutral, and the next load tries again (a choice
 *  in Settings: the page shown keeps the result it rendered with). */
export function themeLoader(
  modules: Record<string, () => Promise<ThemeModule>>,
) {
  const loads = new Map<string, Promise<ThemeModule | null>>();
  const settled = new Map<string, ThemeModule | null>();
  return {
    load(style: string): Promise<ThemeModule | null> {
      let load = loads.get(style);
      if (!load) {
        const start = modules[style];
        load = (start ? start() : Promise.reject(new Error(style))).then(
          (m) => m,
          () => null,
        );
        load.then((m) => {
          settled.set(style, m);
          if (!m) loads.delete(style);
        });
        loads.set(style, load);
      }
      return load;
    },
    /** the module once its load has settled (null: failed), undefined before */
    settled: (style: string) => settled.get(style),
  };
}

const loader = themeLoader({
  stone: () => import("./stone"),
  matcha: () => import("./matcha"),
  y2k: () => import("./y2k"),
  gothic: () => import("./gothic"),
  chocolate: () => import("./chocolate"),
  butter: () => import("./butter"),
} satisfies Record<OnDemand, () => Promise<ThemeModule>>);

/** A theme's object, called while rendering: suspends while it loads (on the server too); undefined for neutral
 *  (astryx-themes.ts has it) or a theme that cannot load, which then shows neutral. */
export function styleTheme(style: Style): DefinedTheme | undefined {
  if (style === "neutral") return undefined;
  const m = loader.settled(style);
  return (m === undefined ? use(loader.load(style)) : m)?.theme;
}

/** A theme's object once loaded (Settings' previews); undefined as for styleTheme. */
export function loadStyleTheme(
  style: Style,
): Promise<DefinedTheme | undefined> {
  return style === "neutral"
    ? Promise.resolve(undefined)
    : loader.load(style).then((m) => m?.theme);
}

/** Loads a theme and its faces before a switch shows it; false if it cannot load.
 *  @public used through providers.tsx's dynamic import, which knip does not follow */
export async function prepareStyle(style: Style): Promise<boolean> {
  if (style === "neutral") return true;
  const m = await loader.load(style);
  if (!m) return false;
  await Promise.all(m.fonts.map((f) => document.fonts.load(f))).catch(() => {});
  return true;
}

// literal imports: next/dynamic records each module so the server can send its stylesheet
const SHEETS: Record<OnDemand, React.ComponentType> = {
  stone: dynamic(() => import("./stone")),
  matcha: dynamic(() => import("./matcha")),
  y2k: dynamic(() => import("./y2k")),
  gothic: dynamic(() => import("./gothic")),
  chocolate: dynamic(() => import("./chocolate")),
  butter: dynamic(() => import("./butter")),
};

/** The stylesheet of the theme shown (its `name`), in the server render too; neutral's is in globals.css.
 *  @public as prepareStyle */
export function ThemeSheet({ name }: { name: string }) {
  const Sheet = SHEETS[name as OnDemand];
  return Sheet ? <Sheet /> : null;
}
