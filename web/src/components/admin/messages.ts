// The admin console's strings, kept apart from the public site's catalogue. Admin languages are Chinese,
// Japanese and English; the Chinese keys define AdminKey. Korean, which the admin does not offer,
// reads English.

import { useMemo } from "react";
import { useUi } from "@/components/providers";
import { type ApiErrors, errorKey } from "@/lib/api";
import { format, type UiLang } from "@/lib/messages";
import { adminEn } from "./messages-en";
import { adminJa } from "./messages-ja";
import { adminZh } from "./messages-zh";

export type AdminKey = keyof typeof adminZh;
export type AdminDict = Record<AdminKey, string>;
export type AdminT = (
  key: AdminKey,
  vars?: Record<string, string | number>,
) => string;

const ADMIN: Record<UiLang, AdminDict> = {
  zh: adminZh,
  en: adminEn,
  ja: adminJa,
  ko: adminEn,
};

export function useAdminT(): AdminT {
  const { ui } = useUi();
  return useMemo(() => {
    const dict = ADMIN[ui];
    return (key, vars) => format(dict[key], vars);
  }, [ui]);
}

/** The API errors the console explains (lib/api.ts errorKey); any other failure says only that it failed. */
const ADMIN_ERRORS: ApiErrors<AdminKey> = {
  messages: {
    "A job of this kind is already queued or running": "error.jobBusy",
    "Job is not queued or running": "error.jobIdle",
    "Set the base URL and API key first": "error.aiUnset",
    "No such user": "error.noUser",
    "Admins can't be disabled.": "error.adminDisable",
  },
  statuses: {
    401: "gate.signInTitle",
    403: "gate.forbiddenTitle",
    502: "error.provider",
  },
};

/** A failed call's message in the console's language. */
export const errorText = (t: AdminT, e: unknown) =>
  t(errorKey(e, ADMIN_ERRORS, "error.failed"));
