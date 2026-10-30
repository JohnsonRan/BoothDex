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
import { format, MESSAGES, type MessageKey } from "./messages";
import {
  type ContentLang,
  CURRENCIES,
  type Currency,
  DEFAULT_SETTINGS,
  parseSettings,
  SETTINGS_COOKIE,
  type Settings,
  type Theme,
} from "./settings-core";

export { CURRENCIES, type ContentLang, type Currency, type Theme };

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

/** Persists settings where the server can read them for the first render (no language flash). */
function writeCookie(s: Settings) {
  // biome-ignore lint/suspicious/noDocumentCookie: the Cookie Store API is missing in older Safari
  document.cookie = `${SETTINGS_COOKIE}=${encodeURIComponent(JSON.stringify(s))}; path=/; max-age=31536000; samesite=lax`;
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

/** `initial` comes from the settings cookie (or Accept-Language) on the server, so the first render already uses it. */
export function SettingsProvider({
  initial = DEFAULT_SETTINGS,
  children,
}: {
  initial?: Settings;
  children: React.ReactNode;
}) {
  const [s, setS] = useState<Settings>(initial);
  const [rates, setRates] = useState<Record<string, number>>({});

  useEffect(() => {
    // visitors from before the cookie existed: move their localStorage settings over once
    if (
      !document.cookie
        .split("; ")
        .some((c) => c.startsWith(`${SETTINGS_COOKIE}=`))
    ) {
      const migrated = parseSettings(
        localStorage.getItem(STORAGE),
        navigator.language,
      );
      setS(migrated);
      writeCookie(migrated);
    }
    // signed-in users get their account preferences
    api<{ preferences: Record<string, string> }>("/api/account")
      .then(({ preferences: p }) =>
        setS((cur) => {
          const next = parseSettings(
            JSON.stringify({
              ...cur,
              ...(p.ui && { ui: p.ui }),
              ...(p.language && { content: p.language }),
              ...(p.currency && { currency: p.currency }),
              ...(p.theme && { theme: p.theme }),
            }),
          );
          writeCookie(next);
          return next;
        }),
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
      writeCookie(next);
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
