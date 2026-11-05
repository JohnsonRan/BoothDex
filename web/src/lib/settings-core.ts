// Settings parsing and the formatters built from them, shared by the server (cookie -> first render) and the
// client. No React here, so server components can import it.
import type { ItemResponse } from "./api-types";
import {
  format,
  MESSAGES,
  type MessageKey,
  UI_LANGS,
  type UiLang,
} from "./messages";

export const CONTENT_LANGS = ["original", "zh", "en", "ja", "ko"] as const;
export type ContentLang = (typeof CONTENT_LANGS)[number];
/** "system" follows the operating system; the other two pin a mode. */
export const THEMES = ["system", "light", "dark"] as const;
export type Theme = (typeof THEMES)[number];
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
  content: ContentLang;
  currency: Currency;
  theme: Theme;
};

export const SETTINGS_COOKIE = "settings";
export const DEFAULT_SETTINGS: Settings = {
  ui: "zh",
  content: "original",
  currency: "JPY",
  theme: "system",
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
    content: pick(v.content, CONTENT_LANGS) ?? DEFAULT_SETTINGS.content,
    currency: pick(v.currency, CURRENCIES) ?? DEFAULT_SETTINGS.currency,
    theme: pick(v.theme, THEMES) ?? DEFAULT_SETTINGS.theme,
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

/** Message lookup for one interface language; a key missing there falls back to Chinese. */
export function translator(ui: UiLang): Translate {
  const dict = MESSAGES[ui];
  return (key, vars) => format(dict[key] ?? MESSAGES.zh[key], vars);
}

/** Yen per unit of each other currency (frankfurter.dev, base JPY). */
export type Rates = Partial<Record<Currency, number>>;

/** Price in the chosen currency; converted amounts are prefixed with ≈, plain ¥ until a rate is known. */
export function formatPrice(
  jpy: number | null | undefined,
  s: Pick<Settings, "ui" | "currency">,
  rates: Rates,
): string {
  if (jpy == null) return "—";
  if (jpy === 0) return translator(s.ui)("price.free");
  const locale = LOCALES[s.ui];
  const rate = s.currency === "JPY" ? 1 : rates[s.currency];
  // plain ¥ for yen (Intl writes "JP¥" in zh-CN)
  if (s.currency === "JPY" || !rate) return `¥${jpy.toLocaleString(locale)}`;
  const v = new Intl.NumberFormat(locale, {
    style: "currency",
    currency: s.currency,
    maximumFractionDigits: s.currency === "KRW" || s.currency === "TWD" ? 0 : 2,
  }).format(jpy * rate);
  return `≈${v}`;
}

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
