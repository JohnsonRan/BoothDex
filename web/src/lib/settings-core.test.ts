import { expect, test } from "bun:test";
import {
  DEFAULT_SETTINGS,
  parseSettings,
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
