// Settings parsing and the formatters built from them, shared by the server (cookie -> first render) and the
// client. No React here, so server components can import it.
import type { ItemResponse } from "./api-types";
import {
  type Dict,
  format,
  type MessageKey,
  UI_LANGS,
  type UiLang,
} from "./messages";

const CONTENT_LANGS = ["original", "zh", "en", "ja", "ko"] as const;
export type ContentLang = (typeof CONTENT_LANGS)[number];
/** The item-text setting: a language, or "ui" to follow the interface language. */
export const ITEM_TEXT = ["ui", ...CONTENT_LANGS] as const;
export type ItemTextSetting = (typeof ITEM_TEXT)[number];
/** "system" follows the operating system; the other two pin a mode. */
const THEMES = ["system", "light", "dark"] as const;
export type Theme = (typeof THEMES)[number];
/** Pure black: in dark mode the page's backgrounds turn black (globals.css). */
const BLACK = ["off", "on"] as const;
type Black = (typeof BLACK)[number];
/** Where the filters sit on a wide screen: a rail left of the results (catalog/filter-rail.tsx, the default), or
 *  the toolbar's row. A compact surface always has the row. The row was "top" while it was the default, so a
 *  stored "top" was mostly never chosen: it reads as the rail now (owner). */
const FILTER_PLACES = ["side", "row"] as const;
type FilterPlace = (typeof FILTER_PLACES)[number];
/** The Astryx themes, neutral first (the default, built into every page); the others load when chosen
 *  (themes/index.tsx). */
export const STYLES = [
  "neutral",
  "stone",
  "matcha",
  "y2k",
  "gothic",
  "chocolate",
  "butter",
] as const;
export type Style = (typeof STYLES)[number];
/** Themes drawn in one mode whatever the setting: Gothic is dark-only (@astryxdesign/theme-gothic README.md:3-5). */
export const SINGLE_MODE: Partial<Record<Style, "light" | "dark">> = {
  gothic: "dark",
};
/** The mode a page is drawn in: a single-mode theme's own, otherwise the visitor's setting. */
export const shownMode = (theme: Theme, style: Style): Theme =>
  SINGLE_MODE[style] ?? theme;
/** Switches of style, each shown once `prepare` has loaded it. The last one asked for wins, also one of the style
 *  already shown (it cancels a switch still loading), and one asked for before a change of account
 *  (`session`, lib/preferences.ts) is dropped. Settles as "failed" when the last one cannot load. */
export function styleSwitch(
  prepare: (style: Style) => Promise<boolean>,
  session: () => number,
) {
  let last = 0;
  return async (
    style: Style,
    show: () => void,
  ): Promise<"shown" | "failed" | "replaced"> => {
    const ask = ++last;
    const at = session();
    const ok = await prepare(style);
    if (ask !== last || session() !== at) return "replaced";
    if (!ok) return "failed";
    show();
    return "shown";
  };
}
export const CURRENCIES = [
  "JPY",
  "USD",
  "EUR",
  "GBP",
  "CNY",
  "TWD",
  "HKD",
  "KRW",
  "AUD",
  "CAD",
  "SGD",
  "THB",
] as const;
export type Currency = (typeof CURRENCIES)[number];
export type Settings = {
  ui: UiLang;
  content: ItemTextSetting;
  currency: Currency;
  theme: Theme;
  style: Style;
  black: Black;
  filters: FilterPlace;
};

export const SETTINGS_COOKIE = "settings";
export const DEFAULT_SETTINGS: Settings = {
  ui: "zh",
  content: "ui",
  currency: "JPY",
  theme: "system",
  style: "neutral",
  black: "off",
  filters: "side",
};

const pick = <T extends string>(
  v: unknown,
  allowed: readonly T[],
): T | undefined =>
  typeof v === "string" && (allowed as readonly string[]).includes(v)
    ? (v as T)
    : undefined;

/** Supported language with highest q-weight in an Accept-Language header; ignores q=0, ties keep header order. */
export function uiFromAcceptLanguage(
  header: string | null | undefined,
): UiLang | undefined {
  if (!header) return undefined;
  const candidates: { lang: UiLang; q: number; order: number }[] = [];
  const parts = header.split(",");
  for (let i = 0; i < parts.length; i++) {
    const part = parts[i].trim();
    if (!part) continue;
    const [tag, ...params] = part.split(";");
    const l = pick(tag.trim().slice(0, 2).toLowerCase(), UI_LANGS);
    if (!l) continue;
    let q = 1.0;
    for (const param of params) {
      const [k, v] = param.trim().split("=");
      if (k === "q" && v) {
        const parsed = Number.parseFloat(v);
        if (!Number.isNaN(parsed)) q = parsed;
      }
    }
    if (q > 0) {
      candidates.push({ lang: l, q, order: i });
    }
  }
  candidates.sort((a, b) => b.q - a.q || a.order - b.order);
  return candidates[0]?.lang;
}

/** Validated settings from a stored JSON string; unknown or invalid fields fall back to defaults. */
export function parseSettings(
  raw: string | null | undefined,
  acceptLanguage?: string | null,
): Settings {
  let v: Record<string, unknown> = {};
  try {
    const parsed = raw ? JSON.parse(raw) : null;
    if (parsed && typeof parsed === "object") v = parsed;
  } catch {}
  return {
    ui:
      pick(v.ui, UI_LANGS) ??
      uiFromAcceptLanguage(acceptLanguage) ??
      DEFAULT_SETTINGS.ui,
    content: pick(v.content, ITEM_TEXT) ?? DEFAULT_SETTINGS.content,
    currency: pick(v.currency, CURRENCIES) ?? DEFAULT_SETTINGS.currency,
    theme: pick(v.theme, THEMES) ?? DEFAULT_SETTINGS.theme,
    style: pick(v.style, STYLES) ?? DEFAULT_SETTINGS.style,
    black: pick(v.black, BLACK) ?? DEFAULT_SETTINGS.black,
    filters: pick(v.filters, FILTER_PLACES) ?? DEFAULT_SETTINGS.filters,
  };
}

/** BCP 47 tag of an interface language, for `<html lang>`, Intl and the component library's catalogue. */
export const LOCALES = {
  zh: "zh-CN",
  en: "en-US",
  ja: "ja-JP",
  ko: "ko-KR",
} as const satisfies Record<UiLang, string>;

export type Translate = (
  key: MessageKey,
  vars?: Record<string, string | number>,
) => string;

/** Message lookup in one language's dictionary (every dictionary has every key). */
export const translate =
  (dict: Dict): Translate =>
  (key, vars) =>
    format(dict[key], vars);

/** Yen per unit of each other currency (frankfurter.dev, base JPY). */
export type Rates = Partial<Record<Currency, number>>;

/** Number formats by locale and currency ("" for plain numbers): building one costs as much as formatting
 *  forty prices with it, and a page formats a hundred. */
const numberFormats = new Map<string, Intl.NumberFormat>();

function numberFormat(
  locale: string,
  currency: Currency | "",
): Intl.NumberFormat {
  const key = `${locale} ${currency}`;
  let format = numberFormats.get(key);
  if (!format) {
    format = new Intl.NumberFormat(
      locale,
      currency
        ? {
            style: "currency",
            currency,
            maximumFractionDigits:
              currency === "KRW" || currency === "TWD" ? 0 : 2,
          }
        : undefined,
    );
    numberFormats.set(key, format);
  }
  return format;
}

/** Price in the chosen currency; converted amounts are prefixed with ≈, plain ¥ until a rate is known. */
export function formatPrice(
  jpy: number | null | undefined,
  s: Pick<Settings, "ui" | "currency">,
  rates: Rates,
  t: Translate,
): string {
  if (jpy == null) return "—";
  if (jpy === 0) return t("price.free");
  const locale = LOCALES[s.ui];
  const rate = s.currency === "JPY" ? 1 : rates[s.currency];
  // plain ¥ for yen (Intl writes "JP¥" in zh-CN)
  if (s.currency === "JPY" || !rate)
    return `¥${numberFormat(locale, "").format(jpy)}`;
  return `≈${numberFormat(locale, s.currency).format(jpy * rate)}`;
}

/** The item-text language the settings ask for. */
export const contentLang = (
  s: Pick<Settings, "ui" | "content">,
): ContentLang => (s.content === "ui" ? s.ui : s.content);

/** Title or bio in the chosen item-text language, falling back to the original. */
export function itemText(
  it: Pick<ItemResponse, "title" | "bio" | "translations">,
  field: "title" | "bio",
  content: ContentLang,
): string {
  return (
    (content === "original"
      ? it[field]
      : it.translations?.[content]?.[field]) || it[field]
  );
}
