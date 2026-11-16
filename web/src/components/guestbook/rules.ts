// The server's limits on a message (crates/api/src/guestbook.rs `validate`): trimmed, counted in Unicode scalar
// values, and free of control characters but a message's line breaks and tabs. A card number is left to the server.

export const NICKNAME_MAX = 24;
export const BODY_MAX = 500;

const size = (s: string) => [...s].length;
// biome-ignore lint/suspicious/noControlCharactersInRegex: the control characters are what this finds
const CONTROL = /[\u0000-\u001f\u007f-\u009f]/;
// biome-ignore lint/suspicious/noControlCharactersInRegex: the control characters are what this finds
const BODY_CONTROL = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f]/;

/** Whether the nickname is one the server takes. */
export const nicknameOk = (v: string) => {
  const s = v.trim();
  return size(s) >= 1 && size(s) <= NICKNAME_MAX && !CONTROL.test(s);
};

/** Whether the message is one the server takes. */
export const bodyOk = (v: string) => {
  const s = v.replaceAll("\r\n", "\n").trim();
  return size(s) >= 1 && size(s) <= BODY_MAX && !BODY_CONTROL.test(s);
};
