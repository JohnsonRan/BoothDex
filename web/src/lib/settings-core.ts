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
    content:
      pick(v.content, ["original", "en", "ja", "ko"] as const) ??
      DEFAULT_SETTINGS.content,
    currency: pick(v.currency, CURRENCIES) ?? DEFAULT_SETTINGS.currency,
    theme: pick(v.theme, ["dark", "light"] as const) ?? DEFAULT_SETTINGS.theme,
  };
}
