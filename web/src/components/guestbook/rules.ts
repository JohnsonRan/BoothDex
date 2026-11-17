// The server's limits on a message (crates/api/src/guestbook.rs `validate`): trimmed, counted in Unicode scalar
// values, free of control characters but a message's line breaks and tabs, and of invisible characters (`INVISIBLE`
// there, the same list). A card number is left to the server.

export const NICKNAME_MAX = 24;
export const BODY_MAX = 500;

const size = (s: string) => [...s].length;
// biome-ignore lint/suspicious/noControlCharactersInRegex: the control characters are what this finds
const CONTROL = /[\u0000-\u001f\u007f-\u009f]/;
// biome-ignore lint/suspicious/noControlCharactersInRegex: the control characters are what this finds
const BODY_CONTROL = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f]/;
const INVISIBLE =
  /[\u00ad\u115f\u1160\u180e\u200b\u200c\u200e\u200f\u202a-\u202e\u2060-\u2064\u2066-\u2069\u2800\u3164\ufeff\uffa0]/;
// what an emoji keeps, the zero-width joiner and variation selectors, shows nothing on its own
// biome-ignore lint/suspicious/noMisleadingCharacterClass: the joiner and selectors stand alone here, not inside an emoji
const BLANK = /^[\s\u200d\ufe00-\ufe0f]*$/;
const shown = (s: string, max: number) =>
  size(s) >= 1 && size(s) <= max && !BLANK.test(s) && !INVISIBLE.test(s);

/** Whether the nickname is one the server takes. */
export const nicknameOk = (v: string) => {
  const s = v.trim();
  return shown(s, NICKNAME_MAX) && !CONTROL.test(s);
};

/** Whether the message is one the server takes. */
export const bodyOk = (v: string) => {
  const s = v.replaceAll("\r\n", "\n").trim();
  return shown(s, BODY_MAX) && !BODY_CONTROL.test(s);
};
