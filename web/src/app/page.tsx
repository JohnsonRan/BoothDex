import { cookies, headers } from "next/headers";
import { Suspense } from "react";
import { Browse } from "@/components/catalog/browse";
import { SettingsProvider } from "@/lib/settings";
import { parseSettings, SETTINGS_COOKIE } from "@/lib/settings-core";

export default function Home() {
  return (
    <Suspense>
      <WithSettings />
    </Suspense>
  );
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
