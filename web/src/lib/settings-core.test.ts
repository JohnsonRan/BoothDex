import { expect, test } from "bun:test";
import { translator } from "./dictionaries";
import {
  contentLang,
  DEFAULT_SETTINGS,
  formatPrice,
  itemText,
  parseSettings,
  shownMode,
  uiFromAcceptLanguage,
} from "./settings-core";

test("stored settings win, invalid fields fall back", () => {
  expect(
    parseSettings(
      '{"ui":"ko","currency":"USD","theme":"light","content":"en"}',
    ),
  ).toEqual({
    ui: "ko",
    currency: "USD",
    theme: "light",
    content: "en",
    style: "neutral",
  });
  expect(
    parseSettings('{"ui":"fr","currency":"XYZ","theme":"neon"}', "ja-JP"),
  ).toEqual({ ...DEFAULT_SETTINGS, ui: "ja" });
  expect(parseSettings("not json")).toEqual(DEFAULT_SETTINGS);
  expect(parseSettings(undefined)).toEqual(DEFAULT_SETTINGS);
});

test("item text follows the interface language unless a language is chosen", () => {
  expect(DEFAULT_SETTINGS.content).toBe("ui");
  expect(contentLang({ ui: "ja", content: "ui" })).toBe("ja");
  expect(contentLang({ ui: "ja", content: "original" })).toBe("original");
  expect(parseSettings('{"content":"ui"}').content).toBe("ui");
});

test("browser language picks the first supported one", () => {
  expect(uiFromAcceptLanguage("fr-FR,ko;q=0.9,en;q=0.8")).toBe("ko");
  expect(uiFromAcceptLanguage("zh-TW,zh;q=0.9")).toBe("zh");
  expect(uiFromAcceptLanguage("de")).toBeUndefined();
  expect(parseSettings(null, "en-US").ui).toBe("en");
});

test("browser language honours q-weights and ignores q=0", () => {
  expect(uiFromAcceptLanguage("ja;q=0,en;q=1")).toBe("en");
  expect(uiFromAcceptLanguage("ja;q=0.1,en;q=1")).toBe("en");
  expect(uiFromAcceptLanguage("en;q=0.8,ja;q=0.9")).toBe("ja");
  expect(uiFromAcceptLanguage("en,ja")).toBe("en");
});

test("the theme defaults to following the system", () => {
  expect(DEFAULT_SETTINGS.theme).toBe("system");
  expect(parseSettings('{"theme":"dark"}').theme).toBe("dark");
  expect(parseSettings('{"theme":"system"}').theme).toBe("system");
});

test("the Astryx theme defaults to neutral; an unknown one falls back to it", () => {
  expect(DEFAULT_SETTINGS.style).toBe("neutral");
  expect(parseSettings('{"style":"matcha"}').style).toBe("matcha");
  expect(parseSettings('{"style":"charcoal"}').style).toBe("neutral");
  expect(parseSettings('{"style":7}').style).toBe("neutral");
});

test("a dark-only theme is drawn dark whatever the mode setting", () => {
  expect(shownMode("light", "gothic")).toBe("dark");
  expect(shownMode("system", "gothic")).toBe("dark");
  expect(shownMode("light", "matcha")).toBe("light");
  expect(shownMode("system", "neutral")).toBe("system");
});

test("translations fill variables", () => {
  expect(translator("en")("title.count", { n: 3 })).toBe("3 items");
  expect(translator("zh")("title.count", { n: 3 })).toBe("3 件");
});

test("prices: yen as is, free by name, other currencies once a rate is known", () => {
  const s = { ui: "en", currency: "USD" } as const;
  const t = translator("en");
  expect(formatPrice(null, s, {}, t)).toBe("—");
  expect(formatPrice(0, s, {}, t)).toBe(t("price.free"));
  expect(formatPrice(1500, s, {}, t)).toBe("¥1,500");
  expect(formatPrice(1500, s, { USD: 0.0066 }, t)).toBe("≈$9.90");
});

test("item text uses the chosen translation, else the original", () => {
  const it = {
    title: "原題",
    bio: "",
    translations: { en: { title: "Title" } },
  };
  expect(itemText(it, "title", "en")).toBe("Title");
  expect(itemText(it, "title", "ko")).toBe("原題");
  expect(itemText(it, "title", "original")).toBe("原題");
});
