// A signed-in visitor's settings also live in the account (/api/account/preferences), as on master
// (settings.tsx): the account's values replace this browser's when the session is found, and every later
// change is sent back. Signed out nothing is sent.

import { api } from "./api";
import { parseSettings, type Settings } from "./settings-core";

/** The account's preference names for the settings fields (the API calls item text "language"). */
const NAMES = {
  ui: "ui",
  content: "language",
  currency: "currency",
  theme: "theme",
} as const satisfies Record<keyof Settings, string>;

let signedIn = false;

/** The session (components/account/session.tsx) turns sending on once it has applied the account's values. */
export function setPreferenceSync(on: boolean) {
  signedIn = on;
}

/** Sends changed settings to the account; a failure keeps them in this browser only, as on master. */
export function savePreferences(patch: Partial<Settings>) {
  if (!signedIn) return;
  const json = Object.fromEntries(
    Object.entries(patch).map(([k, v]) => [NAMES[k as keyof Settings], v]),
  );
  api("/api/account/preferences", { method: "PUT", json }).catch(() => {});
}

/** The settings the account sets differently from `current` (invalid or missing values are ignored). */
export function accountPatch(
  preferences: Record<string, string> | undefined,
  current: Settings,
): Partial<Settings> {
  const p = preferences ?? {};
  const given = Object.fromEntries(
    Object.entries(NAMES).map(([field, name]) => [field, p[name]]),
  );
  // a value parseSettings keeps as given is valid; one it replaces by a default is not
  const valid = parseSettings(JSON.stringify(given));
  return Object.fromEntries(
    Object.entries(valid).filter(
      ([k, v]) => given[k] === v && current[k as keyof Settings] !== v,
    ),
  ) as Partial<Settings>;
}
