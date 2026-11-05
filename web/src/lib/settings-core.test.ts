import { expect, test } from "bun:test";
import {
  DEFAULT_SETTINGS,
  formatPrice,
  itemText,
  parseSettings,
  translator,
  uiFromAcceptLanguage,
} from "./settings-core";

test("stored settings win, invalid fields fall back", () => {
  expect(
    parseSettings(
      '{"ui":"ko","currency":"USD","theme":"light","content":"en"}',
    ),
  ).toEqual({ ui: "ko", currency: "USD", theme: "light", content: "en" });
  expect(
    parseSettings('{"ui":"fr","currency":"XYZ","theme":"neon"}', "ja-JP"),
  ).toEqual({ ...DEFAULT_SETTINGS, ui: "ja" });
  expect(parseSettings("not json")).toEqual(DEFAULT_SETTINGS);
  expect(parseSettings(undefined)).toEqual(DEFAULT_SETTINGS);
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

test("translations fall back to Chinese and fill variables", () => {
  expect(translator("en")("title.count", { n: 3 })).toBe("3 items");
  expect(translator("zh")("title.count", { n: 3 })).toBe("3 件");
});

test("prices: yen as is, free by name, other currencies once a rate is known", () => {
  const s = { ui: "en", currency: "USD" } as const;
  expect(formatPrice(null, s, {})).toBe("—");
  expect(formatPrice(0, s, {})).toBe(translator("en")("price.free"));
  expect(formatPrice(1500, s, {})).toBe("¥1,500");
  expect(formatPrice(1500, s, { USD: 0.0066 })).toBe("≈$9.90");
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
