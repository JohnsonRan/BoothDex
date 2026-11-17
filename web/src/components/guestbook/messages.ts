// The guestbook drawer's strings, loaded with the drawer rather than with the page: the site's own catalogue
// (lib/messages-*.ts) travels in every page's HTML, and only the header button's name is needed there. The
// Chinese keys define GuestbookKey; the terms match the privacy policy (lib/legal-*.ts).

import { useMemo } from "react";
import { useUi } from "@/components/providers";
import { type ApiErrors, errorKey } from "@/lib/api";
import { format, type UiLang } from "@/lib/messages";
import { guestbookEn } from "./messages-en";
import { guestbookJa } from "./messages-ja";
import { guestbookKo } from "./messages-ko";
import { guestbookZh } from "./messages-zh";

export type GuestbookKey = keyof typeof guestbookZh;
export type GuestbookDict = Record<GuestbookKey, string>;
export type GuestbookT = (
  key: GuestbookKey,
  vars?: Record<string, string | number>,
) => string;

const GUESTBOOK: Record<UiLang, GuestbookDict> = {
  zh: guestbookZh,
  en: guestbookEn,
  ja: guestbookJa,
  ko: guestbookKo,
};

export function useGuestbookT(): GuestbookT {
  const { ui } = useUi();
  return useMemo(() => {
    const dict = GUESTBOOK[ui];
    return (key, vars) => format(dict[key], vars);
  }, [ui]);
}

/** The API errors the guestbook explains. A closed guestbook answers 503 by code, so the code wins over the site's
 *  "service is starting" reading of a bare 503. */
export const GUESTBOOK_ERRORS: ApiErrors<GuestbookKey> = {
  codes: {
    bad_guestbook_entry: "error.entry",
    verification_unavailable: "closed",
  },
  statuses: { 429: "error.tooMany" },
};

export const guestbookError = (e: unknown) =>
  errorKey(e, GUESTBOOK_ERRORS, "error.failed");
