// The server's limits on a message (crates/api/src/guestbook.rs `validate`): trimmed of Unicode white space, counted
// in Unicode scalar values, free of control characters but a message's line breaks and tabs, and of invisible
// characters. `INVISIBLE` and `BLANK` copy the server's `INVISIBLE` and `SHAPING` lists; rules.test.ts reads the
// server's file and fails when a copy falls behind.

export const NICKNAME_MAX = 24;
export const BODY_MAX = 500;

const size = (s: string) => [...s].length;
// a nickname's: the line and paragraph separators break a line as a line feed does
// biome-ignore lint/suspicious/noControlCharactersInRegex: the control characters are what this finds
const CONTROL = /[\u0000-\u001f\u007f-\u009f\u2028\u2029]/;
// biome-ignore lint/suspicious/noControlCharactersInRegex: the control characters are what this finds
const BODY_CONTROL = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f]/;
const INVISIBLE =
  /[\u00ad\u061c\u115f\u1160\u180e\u200b\u200c\u200e\u200f\u202a-\u202e\u2060-\u206f\u2800\u3164\ufeff\uffa0\ufff0-\ufffc\u{1bca0}-\u{1bca3}\u{1d173}-\u{1d17a}\u{e0080}-\u{e00ff}\u{e01f0}-\u{e0fff}]/u;
// what shapes the text beside it (an emoji's joiner, variation selectors, a flag's tags) shows nothing on its own
const BLANK =
  // biome-ignore lint/suspicious/noMisleadingCharacterClass: the joiners and selectors stand alone here, not inside an emoji
  /^[\p{White_Space}\u034f\u17b4\u17b5\u180b-\u180d\u180f\u200d\ufe00-\ufe0f\u{e0000}-\u{e007f}\u{e0100}-\u{e01ef}]*$/u;
// Rust's `str::trim`. JavaScript's own trim also takes the BOM, which the server refuses, and leaves U+0085.
const trim = (s: string) =>
  s.replace(/^\p{White_Space}+|\p{White_Space}+$/gu, "");
const shown = (s: string, max: number) =>
  size(s) >= 1 && size(s) <= max && !BLANK.test(s) && !INVISIBLE.test(s);

/** Whether the nickname is one the server takes. */
export const nicknameOk = (v: string) => {
  const s = trim(v);
  return shown(s, NICKNAME_MAX) && !CONTROL.test(s);
};

/** The message as the server keeps it: line breaks as line feeds, trimmed. */
export const normalBody = (v: string) => trim(v.replaceAll("\r\n", "\n"));

/** The message's length as the server and the database count it, in Unicode scalar values: the form's counter. */
export const bodyLength = (v: string) => size(normalBody(v));

/** Whether the message is one the server takes. */
export const bodyOk = (v: string) => {
  const s = normalBody(v);
  return shown(s, BODY_MAX) && !BODY_CONTROL.test(s);
};
