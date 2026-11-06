import type { Metadata } from "next";
import { headers } from "next/headers";
import { Suspense } from "react";
import { AdminConsole } from "@/components/admin/admin-console";
import { Providers } from "@/components/providers";
import { MESSAGES } from "@/lib/dictionaries";
import { componentCatalogue, readSettings } from "@/lib/server-data";

export const metadata: Metadata = {
  title: "Admin · BoothDex",
  robots: { index: false, follow: false },
};

/** A phone by its client hint (Chromium) or its user agent (Safari sends no hints). */
async function isPhone() {
  const h = await headers();
  return (
    h.get("sec-ch-ua-mobile") === "?1" ||
    /\b(iPhone|Mobile)\b/.test(h.get("user-agent") ?? "")
  );
}

async function AdminRoot({ children }: { children: React.ReactNode }) {
  const settings = await readSettings();
  return (
    <Providers
      settings={settings}
      messages={MESSAGES[settings.ui]}
      catalogue={await componentCatalogue(settings.ui)}
    >
      <AdminConsole mobile={await isPhone()}>{children}</AdminConsole>
    </Providers>
  );
}

// Settings (cookie) are per request, so the console streams in behind one boundary; its data loads in the browser.
export default function AdminLayout({ children }: LayoutProps<"/admin">) {
  return (
    <Suspense>
      <AdminRoot>{children}</AdminRoot>
    </Suspense>
  );
}
