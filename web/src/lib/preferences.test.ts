import { expect, test } from "bun:test";
import { accountPatch } from "./preferences";
import { DEFAULT_SETTINGS } from "./settings-core";

test("the account's valid preferences replace the browser's settings that differ", () => {
  const current = { ...DEFAULT_SETTINGS, ui: "en" as const };
  expect(
    accountPatch(
      { ui: "ja", language: "en", currency: "JPY", theme: "dark" },
      current,
    ),
  ).toEqual({ ui: "ja", content: "en", theme: "dark" });
});

test("missing or invalid preferences leave the browser's settings alone", () => {
  expect(accountPatch(undefined, DEFAULT_SETTINGS)).toEqual({});
  expect(
    accountPatch(
      { ui: "fr", currency: "XYZ", theme: "sepia" },
      DEFAULT_SETTINGS,
    ),
  ).toEqual({});
});
