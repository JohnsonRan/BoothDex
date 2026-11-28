import type { Metadata } from "next";
import { headers } from "next/headers";
import { HomeScreen } from "@/components/catalog/home-screen";
import { avatarName, parseFilters, RAW_QUERY_HEADER } from "@/lib/catalog";
import { translator } from "@/lib/dictionaries";
import { catalogTitle, homeMetadata } from "@/lib/seo";
import { listAvatars, readSettings } from "@/lib/server-data";

// The metadata reads request headers, and htmlLimitedBots (next.config.ts) makes it block the <head> for every
// visitor. next dev's validation then finds no static shell and reports an invariant ("did not produce a static
// shell") on every request; instant = false skips that check and leaves the build output as it was.
export const instant = false;

export async function generateMetadata({
  searchParams,
}: PageProps<"/">): Promise<Metadata> {
  // Await request data before reading SITE_URL: one prebuilt image serves any domain.
  if (!Object.keys(await searchParams).length) return homeMetadata(false);
  const filters = parseFilters((await headers()).get(RAW_QUERY_HEADER) ?? "");
  const [{ ui }, avatars] = await Promise.all([
    readSettings(),
    filters.avatar ? listAvatars().catch(() => []) : [],
  ]);
  const avatar = avatars.find((a) => a.slug === filters.avatar);
  const title = catalogTitle(
    translator(ui),
    filters,
    avatar && avatarName(avatar, ui),
  );
  return homeMetadata(true, title);
}

// Settings (cookie) and the search (URL) are per request, and the metadata already holds the <head> for them, so
// the screen renders with the document rather than behind a boundary that React would reveal 300 ms later.
export default function Home({ searchParams }: PageProps<"/">) {
  return <HomeScreen searchParams={searchParams} />;
}
