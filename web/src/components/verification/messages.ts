// The Turnstile strings of the forms that show the widget (the guestbook, sign-in and registration), loaded with
// those forms rather than with the page: the site's own catalogue (lib/messages-*.ts) travels in every page's HTML.
// The Chinese keys define VerificationKey.

import { useMemo } from "react";
import { useUi } from "@/components/providers";
import { ApiError } from "@/lib/api";
import type { UiLang } from "@/lib/messages";
import { verificationEn } from "./messages-en";
import { verificationJa } from "./messages-ja";
import { verificationKo } from "./messages-ko";
import { verificationZh } from "./messages-zh";

export type VerificationKey = keyof typeof verificationZh;
export type VerificationDict = Record<VerificationKey, string>;

const VERIFICATION: Record<UiLang, VerificationDict> = {
  zh: verificationZh,
  en: verificationEn,
  ja: verificationJa,
  ko: verificationKo,
};

export function useVerificationT(): (key: VerificationKey) => string {
  const { ui } = useUi();
  return useMemo(() => {
    const dict = VERIFICATION[ui];
    return (key) => dict[key];
  }, [ui]);
}

/** True when the API refused the widget's token (403 `verification_failed`); the form says so in these words. */
export const verificationRefused = (e: unknown) =>
  e instanceof ApiError && e.code === "verification_failed";
