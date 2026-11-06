import { expect, test } from "bun:test";
import {
  accountPatch,
  saveAvatar,
  savePreferences,
  setPreferenceSync,
} from "./preferences";
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

/** The account requests `run` makes while signed in, as [path, method, body]. */
function sentWhileSignedIn(run: () => void) {
  const sent: unknown[] = [];
  const realFetch = globalThis.fetch;
  globalThis.fetch = (async (path: string, init: RequestInit) => {
    sent.push([path, init.method, init.body]);
    return Response.json({});
  }) as typeof fetch;
  setPreferenceSync(true);
  try {
    run();
  } finally {
    setPreferenceSync(false);
    globalThis.fetch = realFetch;
  }
  return sent;
}

test("item text that follows the interface language clears the account's item-text language", () => {
  expect(
    sentWhileSignedIn(() => savePreferences({ content: "ui", ui: "ja" })),
  ).toEqual([["/api/account/preferences", "PUT", '{"language":"","ui":"ja"}']]);
});

test("my avatar goes to the account only while signed in", () => {
  const sent: unknown[] = [];
  const realFetch = globalThis.fetch;
  globalThis.fetch = (async (path: string, init: RequestInit) => {
    sent.push([path, init.method, init.body]);
    return Response.json({});
  }) as typeof fetch;
  try {
    saveAvatar("manuka");
    setPreferenceSync(true);
    saveAvatar("manuka");
  } finally {
    setPreferenceSync(false);
    globalThis.fetch = realFetch;
  }
  expect(sent).toEqual([
    ["/api/account/preferences", "PUT", '{"avatar":"manuka"}'],
  ]);
});
