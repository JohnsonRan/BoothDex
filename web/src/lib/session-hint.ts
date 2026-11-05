// The last session's answer, kept in this browser for the next page load: only that the visitor was signed in
// (no account data). The head script (app/layout.tsx) sets `<html data-account>` from it before the first paint,
// so globals.css shows the account control instead of Sign in; session.tsx keeps both up to date.

export const SIGNED_IN = "in";
const KEY = "account";

/** The head script's part: marks the page signed in before the first paint. */
export const SESSION_HINT_SCRIPT = `try{if(localStorage.getItem("${KEY}")==="${SIGNED_IN}")document.documentElement.dataset.account="${SIGNED_IN}"}catch(e){}`;

/** Records the session's answer for the next page load and shows the matching control. */
export function keepSessionHint(signedIn: boolean) {
  const root = document.documentElement;
  if (signedIn) root.dataset.account = SIGNED_IN;
  else delete root.dataset.account;
  try {
    if (signedIn) localStorage.setItem(KEY, SIGNED_IN);
    else localStorage.removeItem(KEY);
  } catch {}
}
