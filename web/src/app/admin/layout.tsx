import type { Metadata } from "next";
import { Suspense } from "react";
import { AdminConsole } from "@/components/admin/admin-console";
import { Providers } from "@/components/providers";
import { MESSAGES } from "@/lib/dictionaries";
import { componentCatalogue, readSettings } from "@/lib/server-data";

export const metadata: Metadata = {
  title: "Admin · BoothDex",
  robots: { index: false, follow: false },
};

async function AdminRoot({ children }: { children: React.ReactNode }) {
  const settings = await readSettings();
  return (
    <Providers
      settings={settings}
      messages={MESSAGES[settings.ui]}
      catalogue={await componentCatalogue(settings.ui)}
    >
      <AdminConsole>{children}</AdminConsole>
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
