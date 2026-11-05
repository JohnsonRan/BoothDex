// The admin console's strings, kept apart from the public site's catalogue. Admin languages are Chinese,
// Japanese and English; the Chinese keys define AdminKey. Korean, which the admin does not offer,
// reads English.

import { useMemo } from "react";
import { useUi } from "@/components/providers";
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
