"use client";

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import { api } from "./api";
import type { Item } from "./catalog";
import {
  format,
  MESSAGES,
  type MessageKey,
  UI_LANGS,
  type UiLang,
} from "./messages";

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

type Settings = {
  ui: UiLang;
  content: ContentLang;
  currency: Currency;
  theme: Theme;
};

type Ctx = Settings & {
  set: (patch: Partial<Settings>) => void;
  t: (key: MessageKey, vars?: Record<string, string | number>) => string;
  /** price in the chosen currency; converted amounts are prefixed with ≈ */
  money: (jpy: number | null | undefined) => string;
  price: (it: Pick<Item, "smart_price" | "price_jpy">) => string;
  /** title/bio in the chosen item-text language, falling back to the original */
  text: (
    it: Pick<Item, "title" | "bio" | "translations">,
    field: "title" | "bio",
  ) => string;
};

const SettingsContext = createContext<Ctx | null>(null);
const STORAGE = "settings";
const RATES = "fx-jpy";

function detectUi(): UiLang {
  const l = navigator.language.slice(0, 2);
  return (UI_LANGS as readonly string[]).includes(l) ? (l as UiLang) : "en";
}

async function loadRates(): Promise<Record<string, number>> {
  const cached = JSON.parse(localStorage.getItem(RATES) ?? "null");
  if (cached && Date.now() - cached.at < 12 * 3600e3) return cached.rates;
  const r = await fetch(
    `https://api.frankfurter.dev/v1/latest?base=JPY&symbols=${CURRENCIES.filter((c) => c !== "JPY").join(",")}`,
  );
  const rates = (await r.json()).rates as Record<string, number>;
  localStorage.setItem(RATES, JSON.stringify({ at: Date.now(), rates }));
  return rates;
}

export function SettingsProvider({ children }: { children: React.ReactNode }) {
  const [s, setS] = useState<Settings>({
    ui: "zh",
    content: "original",
    currency: "JPY",
    theme: "dark",
  });
  const [rates, setRates] = useState<Record<string, number>>({});

  useEffect(() => {
    const stored = JSON.parse(
      localStorage.getItem(STORAGE) ?? "null",
    ) as Partial<Settings> | null;
    setS((p) => ({ ...p, ui: detectUi(), ...stored }));
    // signed-in users get their account preferences
    api<{ preferences: Record<string, string> }>("/api/account")
      .then(({ preferences: p }) =>
        setS((cur) => ({
          ...cur,
          ...(p.ui && { ui: p.ui as UiLang }),
          ...(p.language && { content: p.language as ContentLang }),
          ...(p.currency && { currency: p.currency as Currency }),
          ...(p.theme && { theme: p.theme as Theme }),
        })),
      )
      .catch(() => {});
  }, []);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", s.theme === "dark");
    document.documentElement.lang = {
      zh: "zh-CN",
      en: "en",
      ja: "ja",
      ko: "ko",
    }[s.ui];
  }, [s.theme, s.ui]);

  useEffect(() => {
    if (s.currency !== "JPY" && !rates[s.currency])
      loadRates()
        .then(setRates)
        .catch(() => {});
  }, [s.currency, rates]);

  const set = useCallback((patch: Partial<Settings>) => {
    setS((cur) => {
      const next = { ...cur, ...patch };
      localStorage.setItem(STORAGE, JSON.stringify(next));
      return next;
    });
    const prefs = {
      ...(patch.ui && { ui: patch.ui }),
      ...(patch.content && { language: patch.content }),
      ...(patch.currency && { currency: patch.currency }),
      ...(patch.theme && { theme: patch.theme }),
    };
    api("/api/account/preferences", { method: "PUT", json: prefs }).catch(
      () => {},
    ); // 401 when signed out is fine
  }, []);

  const ctx = useMemo<Ctx>(() => {
    const dict = MESSAGES[s.ui];
    const t: Ctx["t"] = (key, vars) =>
      format(dict[key] ?? MESSAGES.zh[key], vars);
    const locale = { zh: "zh-CN", en: "en-US", ja: "ja-JP", ko: "ko-KR" }[s.ui];
    const money: Ctx["money"] = (jpy) => {
      if (jpy == null) return "—";
      if (jpy === 0) return t("price.free");
      const rate = s.currency === "JPY" ? 1 : rates[s.currency];
      // plain ¥ for yen (Intl writes "JP¥" in zh-CN); also the fallback until rates load
      if (s.currency === "JPY" || !rate)
        return `¥${jpy.toLocaleString(locale)}`;
      const v = new Intl.NumberFormat(locale, {
        style: "currency",
        currency: s.currency,
        maximumFractionDigits:
          s.currency === "KRW" || s.currency === "TWD" ? 0 : 2,
      }).format(jpy * rate);
      return `≈${v}`;
    };
    return {
      ...s,
      set,
      t,
      money,
      price: (it) => money(it.smart_price ?? it.price_jpy),
      text: (it, field) =>
        (s.content === "original"
          ? it[field]
          : it.translations?.[s.content]?.[field]) || it[field],
    };
  }, [s, rates, set]);

  return (
    <SettingsContext.Provider value={ctx}>{children}</SettingsContext.Provider>
  );
}

export function useSettings() {
  const c = useContext(SettingsContext);
  if (!c) throw new Error("useSettings outside SettingsProvider");
  return c;
}
