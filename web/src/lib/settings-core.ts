// Settings parsing shared by the server (cookie -> first render) and the client. No React here, so server
// components can import it.
import { UI_LANGS, type UiLang } from "./messages";

export type ContentLang = "original" | "en" | "ja" | "ko";
export type Theme = "dark" | "light";
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
  theme: "dark",
};

const pick = <T extends string>(
  v: unknown,
  allowed: readonly T[],
): T | undefined =>
  typeof v === "string" && (allowed as readonly string[]).includes(v)
    ? (v as T)
    : undefined;

/** First supported language in an Accept-Language header (e.g. "ja-JP,ja;q=0.9,en;q=0.8"). */
export function uiFromAcceptLanguage(
  header: string | null | undefined,
): UiLang | undefined {
  for (const part of (header ?? "").split(",")) {
    const l = pick(part.trim().slice(0, 2).toLowerCase(), UI_LANGS);
    if (l) return l;
  }
  return undefined;
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
    content:
      pick(v.content, ["original", "en", "ja", "ko"] as const) ??
      DEFAULT_SETTINGS.content,
    currency: pick(v.currency, CURRENCIES) ?? DEFAULT_SETTINGS.currency,
    theme: pick(v.theme, ["dark", "light"] as const) ?? DEFAULT_SETTINGS.theme,
  };
}
