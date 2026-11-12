// UI strings, checked against the Microsoft localization style guides (aka.ms/<language>-styleguide):
// zh-CN software text: 你 not 您; half-width ( ) ? : ; spaces around Latin words and numbers.
// ja-JP: desu-masu for messages, "...してください。" for requests; half-width ( ) for supplements; half-width space
//        between full- and half-width characters; “ ” quotes.
// ko-KR: conversational Microsoft voice; half-width ( ) with no space before.
// Placeholders: {name}. One file per language (messages-<lang>.ts); the Chinese keys define MessageKey. The
// dictionaries are read on the server (dictionaries.ts), which hands the browser the visitor's language only.

import type { ApiErrors } from "./api";
import type { zh } from "./messages-zh";

export const UI_LANGS = ["zh", "en", "ja", "ko"] as const;
export type UiLang = (typeof UI_LANGS)[number];

export type MessageKey = keyof typeof zh;
export type Dict = Record<MessageKey, string>;

/** The API errors the site explains (lib/api.ts errorKey); any other failure shows the action's own message. */
export const API_ERRORS: ApiErrors<MessageKey> = {
  codes: {
    bad_credentials: "error.credentials",
    name_taken: "error.taken",
    account_disabled: "error.disabled",
    registration_closed: "error.registrationClosed",
    saved_search_limit: "saved.limit",
    wrong_password: "error.password",
    wrong_current_password: "error.password",
    password_unchanged: "profile.same",
    admin_cannot_delete: "delete.admin",
  },
  statuses: {
    401: "error.signIn",
    429: "error.tooMany",
    503: "list.startingTitle",
  },
};

/** Each language's name in itself, for the language menus. */
export const LANG_NAMES: Record<UiLang, string> = {
  zh: "中文",
  en: "English",
  ja: "日本語",
  ko: "한국어",
};

export function format(
  template: string,
  vars?: Record<string, string | number>,
) {
  return vars
    ? template.replace(/\{(\w+)\}/g, (m, k) =>
        k in vars ? String(vars[k]) : m,
      )
    : template;
}
