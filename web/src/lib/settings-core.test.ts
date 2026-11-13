import { expect, test } from "bun:test";
import { translator } from "./dictionaries";
import {
  contentLang,
  DEFAULT_SETTINGS,
  formatPrice,
  itemText,
  parseSettings,
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
    black: "off",
    filters: "side",
  });
  expect(
    parseSettings('{"ui":"fr","currency":"XYZ","theme":"neon"}', "ja-JP"),
  ).toEqual({ ...DEFAULT_SETTINGS, ui: "ja" });
  expect(parseSettings("not json")).toEqual(DEFAULT_SETTINGS);
  expect(parseSettings(undefined)).toEqual(DEFAULT_SETTINGS);
});

test("pure black is off unless turned on", () => {
  expect(DEFAULT_SETTINGS.black).toBe("off");
  expect(parseSettings('{"black":"on"}').black).toBe("on");
  expect(parseSettings('{"black":true}').black).toBe("off");
});

test("filters sit at the side unless put in the row", () => {
  expect(DEFAULT_SETTINGS.filters).toBe("side");
  expect(parseSettings('{"filters":"row"}').filters).toBe("row");
  // the row's old value, stored while it was the default
  expect(parseSettings('{"filters":"top"}').filters).toBe("side");
  expect(parseSettings('{"filters":"left"}').filters).toBe("side");
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

test("a stored field the site no longer has is dropped", () => {
  // the Astryx theme choice, removed on 2026-11-13; old cookies and accounts still hold it
  expect(parseSettings('{"style":"y2k","theme":"dark"}')).toEqual({
    ...DEFAULT_SETTINGS,
    theme: "dark",
  });
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

test("prices keep each language's and currency's format after another was used", () => {
  const t = translator("en");
  const rates = { USD: 0.0066, KRW: 9.1 };
  expect(formatPrice(1500, { ui: "en", currency: "USD" }, rates, t)).toBe(
    "≈$9.90",
  );
  expect(formatPrice(1500, { ui: "en", currency: "KRW" }, rates, t)).toBe(
    "≈₩13,650",
  );
  expect(formatPrice(1234567, { ui: "ja", currency: "JPY" }, rates, t)).toBe(
    "¥1,234,567",
  );
  expect(formatPrice(1500, { ui: "en", currency: "USD" }, rates, t)).toBe(
    "≈$9.90",
  );
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
