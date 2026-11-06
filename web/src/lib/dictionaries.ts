// Every interface language's messages, for the server: pages render with them and give the browser the
// visitor's own dictionary (Providers `messages`), so the client bundle carries none of them.

import type { Dict, UiLang } from "./messages";
import { en } from "./messages-en";
import { ja } from "./messages-ja";
import { ko } from "./messages-ko";
import { zh } from "./messages-zh";
import { type Translate, translate } from "./settings-core";

export const MESSAGES: Record<UiLang, Dict> = { zh, en, ja, ko };

/** Message lookup for one interface language. */
export const translator = (ui: UiLang): Translate => translate(MESSAGES[ui]);
