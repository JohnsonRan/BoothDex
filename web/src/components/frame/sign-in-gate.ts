// What the sign-in dialog (sign-in.tsx) waits for before it sends a form: the answer of `/api/account/registration`,
// which names the Turnstile site key while sign-in and registration are checked. A form sent before that answer, or
// after its request failed, carried no token, so the API refused it and the address spent one of its attempts.

/** `/api/account/registration` */
export type Registration = { open: boolean; site_key: string | null };

/** The widget as the dialog reads it when it sends. */
export type Widget = {
  siteKey: string;
  token: string;
  failed: boolean;
} | null;

/** How long one ask for the answer may take before the dialog goes on without it. */
export const ANSWER_WAIT_MS = 5000;

/** The answer of `fetchAnswer`, or null when it failed or took longer than `wait` (a late answer is dropped). */
export function askRegistration(
  fetchAnswer: () => Promise<Registration>,
  wait = ANSWER_WAIT_MS,
): Promise<Registration | null> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  return Promise.race([
    fetchAnswer().catch(() => null),
    new Promise<null>((resolve) => {
      timer = setTimeout(() => resolve(null), wait);
    }),
  ]).finally(() => clearTimeout(timer));
}

/**
 * The answer a submit goes by: this open's (`pending`), once it arrives; when that is null, one more `ask` (which puts
 * its answer in the dialog, as the first one was, whatever becomes of this submit). "cancelled" when the form the
 * submit came from is no longer shown after a wait (`still`): the dialog closed, opened again, or changed between
 * sign-in and registration, even if it changed back.
 */
export async function waitForAnswer({
  pending,
  ask,
  still,
}: {
  pending: Promise<Registration | null>;
  ask: () => Promise<Registration | null>;
  still: () => boolean;
}): Promise<{ answer: Registration | null } | "cancelled"> {
  const first = await pending;
  if (!still()) return "cancelled";
  if (first) return { answer: first };
  const again = await ask();
  if (!still()) return "cancelled";
  return { answer: again };
}

/**
 * The token a submit sends, or "required" while the answer names a site key and the widget has given none: it may
 * not even be drawn yet when the answer has only just arrived. A widget whose script did not load, an answer naming no
 * site key, and no answer at all send none, and the API decides, as it lets a request through when Cloudflare is out
 * of reach.
 */
export function tokenFor(
  answer: Registration | null,
  widget: Widget,
): string | "required" {
  if (!answer?.site_key) return "";
  if (!widget || widget.siteKey !== answer.site_key) return "required";
  if (widget.failed) return "";
  return widget.token || "required";
}

/**
 * Single-flight controller across the sign-in dialog's two submit phases:
 * - waiting on `/api/account/registration`: closing, reopening or switching tabs cancels the wait so the form is never stuck;
 * - posting to `/api/account/login` or `/api/account/register`: stays single-flight until the request settles so reopening cannot overlap two account POSTs.
 */
export function createSubmitFlight() {
  let form = 0;
  let seq = 0;
  let active = 0;
  let posting = false;
  return {
    /** Whether a submit is currently waiting on registration settings or posting to the account API. */
    busy: () => posting || active !== 0,
    /** Called whenever the dialog opens, closes, or switches mode; returns whether a submit is still busy. */
    onFormChange(): boolean {
      form++;
      if (!posting) {
        active = 0;
        return false;
      }
      return true;
    },
    /** Starts a submit attempt, or returns null while another submit is waiting or posting. */
    begin() {
      if (posting || active !== 0) return null;
      const run = ++seq;
      const from = form;
      active = run;
      return {
        still: () => form === from && active === run,
        enterPost() {
          if (form !== from || active !== run) return false;
          posting = true;
          return true;
        },
        finish(): boolean {
          if (active !== run) return false;
          posting = false;
          active = 0;
          return true;
        },
      };
    },
  };
}
