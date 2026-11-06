// A signed-in visitor's settings and "my avatar" also live in the account (/api/account/preferences), as on
// master (settings.tsx, browse.tsx): the account's values replace this browser's when the session is found, and
// every later change is sent back. Signed out nothing is sent.

import { api } from "./api";
import { parseSettings, type Settings } from "./settings-core";

/** The account's preference names for the settings fields (the API calls item text "language"). */
const NAMES = {
  ui: "ui",
  content: "language",
  currency: "currency",
  theme: "theme",
  style: "style",
} as const satisfies Record<keyof Settings, string>;

let signedIn = false;

/** The session (components/account/session.tsx) turns sending on once it has applied the account's values. */
export function setPreferenceSync(on: boolean) {
  signedIn = on;
}

// who is signed in (null: nobody; undefined until the session has read it), and how often that has changed
let account: number | null | undefined;
let accountChanges = 0;

/** The session sets who is signed in once it has read it: a theme asked for under another account is not saved
 *  (components/providers.tsx). */
export function setPreferenceAccount(id: number | null) {
  if (account !== undefined && id !== account) accountChanges++;
  account = id;
}

/** Counts the sign-ins and sign-outs since the page opened. */
export const preferenceSession = () => accountChanges;

/** Sends preferences to the account; a failure keeps them in this browser only, as on master. */
function send(json: Record<string, string>) {
  if (!signedIn) return;
  api("/api/account/preferences", { method: "PUT", json }).catch(() => {});
}

/** Sends changed settings to the account; item text that follows the interface language is sent as "ui". */
export function savePreferences(patch: Partial<Settings>) {
  send(
    Object.fromEntries(
      Object.entries(patch).map(([k, v]) => [NAMES[k as keyof Settings], v]),
    ),
  );
}

/** Sends "my avatar" (the avatar-fit view's pick) to the account. */
export function saveAvatar(avatar: string) {
  send({ avatar });
}

/** The settings the account sets differently from `current` (invalid or missing values are ignored: an account
 *  saved before item text could be stored as "ui" has no "language" when it follows the interface). */
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
