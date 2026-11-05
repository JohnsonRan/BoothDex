import type { Metadata } from "next";
import { cookies, headers } from "next/headers";
import { connection } from "next/server";
import { Suspense } from "react";
import { Browse } from "@/components/catalog/browse";
import { homeMetadata } from "@/lib/seo";
import { SettingsProvider } from "@/lib/settings";
import { parseSettings, SETTINGS_COOKIE } from "@/lib/settings-core";

export async function generateMetadata({
  searchParams,
}: PageProps<"/">): Promise<Metadata> {
  // Await request data before reading SITE_URL: one prebuilt image serves any domain.
  return homeMetadata(Object.keys(await searchParams).length > 0);
}

export default function Home() {
  return (
    <>
      <Suspense>
        <WithSettings />
      </Suspense>
      {/* metadata depends on the query (noindex for searches), so mark the page as per-request on purpose */}
      <Suspense>
        <DynamicMarker />
      </Suspense>
    </>
  );
}

async function DynamicMarker() {
  await connection();
  return null;
}

/** Reads the visitor's settings (cookie, else Accept-Language) so the very first HTML is already in their language. */
async function WithSettings() {
  const [jar, hdrs] = await Promise.all([cookies(), headers()]);
  const raw = jar.get(SETTINGS_COOKIE)?.value;
  let decoded = raw;
  try {
    decoded = raw && decodeURIComponent(raw);
  } catch {}
  return (
    <SettingsProvider
      initial={parseSettings(decoded, hdrs.get("accept-language"))}
    >
      <Browse />
    </SettingsProvider>
  );
}
