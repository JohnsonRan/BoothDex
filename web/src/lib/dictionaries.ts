// Every interface language's messages, for the server: pages render with them and give the browser the
// visitor's own dictionary (Providers `messages`), so the client bundle carries none of them.

import type { Dict, MessageKey, UiLang } from "./messages";
import { en } from "./messages-en";
import { ja } from "./messages-ja";
import { ko } from "./messages-ko";
import { zh } from "./messages-zh";
import { type Translate, translate } from "./settings-core";

export const MESSAGES: Record<UiLang, Dict> = { zh, en, ja, ko };

/** Message lookup for one interface language. */
export const translator = (ui: UiLang): Translate => translate(MESSAGES[ui]);

/** Keys that only results.tsx looks up, on the server: the notes on how a search was answered and the plain page
 *  links. The dictionary the browser gets leaves them out; dictionaries.test.ts keeps every other file off them. */
export const SERVER_ONLY: readonly MessageKey[] = [
  "list.typoTolerant",
  "list.searchInstead",
  "list.expandedWith",
  "list.withoutExpanded",
  "list.related",
  "list.semanticUnavailable",
  "list.semanticPending",
  "page.nav",
  "page.prev",
  "page.next",
];

/** The visitor's dictionary for the browser (Providers `messages`). Typed as the whole dictionary: no client
 *  component looks up a key it leaves out. */
export const browserMessages = (ui: UiLang): Dict =>
  Object.fromEntries(
    Object.entries(MESSAGES[ui]).filter(
      ([key]) => !SERVER_ONLY.includes(key as MessageKey),
    ),
  ) as Dict;
