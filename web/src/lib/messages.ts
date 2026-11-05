// UI strings, checked against the Microsoft localization style guides (aka.ms/<language>-styleguide):
// zh-CN software text: 你 not 您; half-width ( ) ? : ; spaces around Latin words and numbers.
// ja-JP: desu-masu for messages, "...してください。" for requests; half-width ( ) for supplements; half-width space
//        between full- and half-width characters; “ ” quotes.
// ko-KR: conversational Microsoft voice; half-width ( ) with no space before.
// Placeholders: {name}. One file per language (messages-<lang>.ts); the Chinese keys define MessageKey.

import { en } from "./messages-en";
import { ja } from "./messages-ja";
import { ko } from "./messages-ko";
import { zh } from "./messages-zh";

export const UI_LANGS = ["zh", "en", "ja", "ko"] as const;
export type UiLang = (typeof UI_LANGS)[number];

export type MessageKey = keyof typeof zh;
export type Dict = Record<MessageKey, string>;

export const MESSAGES: Record<UiLang, Dict> = { zh, en, ja, ko };

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
