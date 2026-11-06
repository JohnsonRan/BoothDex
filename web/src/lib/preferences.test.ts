import { expect, test } from "bun:test";
import {
  accountPatch,
  preferenceSession,
  saveAvatar,
  savePreferences,
  setPreferenceAccount,
  setPreferenceSync,
} from "./preferences";
import { DEFAULT_SETTINGS } from "./settings-core";

test("the account's valid preferences replace the browser's settings that differ", () => {
  const current = { ...DEFAULT_SETTINGS, ui: "en" as const };
  expect(
    accountPatch(
      {
        ui: "ja",
        language: "en",
        currency: "JPY",
        theme: "dark",
        style: "gothic",
      },
      current,
    ),
  ).toEqual({ ui: "ja", content: "en", theme: "dark", style: "gothic" });
});

test("missing or invalid preferences leave the browser's settings alone", () => {
  expect(accountPatch(undefined, DEFAULT_SETTINGS)).toEqual({});
  expect(
    accountPatch(
      { ui: "fr", currency: "XYZ", theme: "sepia", style: "charcoal" },
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

test("item text that follows the interface language is sent to the account as such", () => {
  expect(
    sentWhileSignedIn(() => savePreferences({ content: "ui", ui: "ja" })),
  ).toEqual([
    ["/api/account/preferences", "PUT", '{"language":"ui","ui":"ja"}'],
  ]);
});

test("the Astryx theme is sent to the account under its own name", () => {
  expect(sentWhileSignedIn(() => savePreferences({ style: "matcha" }))).toEqual(
    [["/api/account/preferences", "PUT", '{"style":"matcha"}']],
  );
});

test("an account's item text that follows the interface language reaches another browser", () => {
  const current = { ...DEFAULT_SETTINGS, content: "en" as const };
  expect(accountPatch({ language: "ui" }, current)).toEqual({ content: "ui" });
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

test("the preference session changes on a sign-in or sign-out, not when the session is first read or read again", () => {
  const start = preferenceSession();
  setPreferenceAccount(7);
  setPreferenceAccount(7);
  expect(preferenceSession()).toBe(start);
  setPreferenceAccount(null);
  expect(preferenceSession()).toBe(start + 1);
  setPreferenceAccount(8);
  expect(preferenceSession()).toBe(start + 2);
});
