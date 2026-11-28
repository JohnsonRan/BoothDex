import type { Metadata } from "next";
import { type AdminKey, adminTranslator } from "@/components/admin/messages";
import { readSettings } from "@/lib/server-data";

export function adminPageMetadata(key: AdminKey): () => Promise<Metadata> {
  return async () => {
    const t = adminTranslator((await readSettings()).ui);
    return { title: `${t(key)} · ${t("title")} · BoothDex` };
  };
}
