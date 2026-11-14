import type { Metadata } from "next";
import { HomeScreen } from "@/components/catalog/home-screen";
import { homeMetadata } from "@/lib/seo";

// The metadata reads searchParams, and htmlLimitedBots (next.config.ts) makes it block the <head> for every
// visitor. next dev's validation then finds no static shell and reports an invariant ("did not produce a static
// shell") on every request; instant = false skips that check and leaves the build output as it was.
export const instant = false;

export async function generateMetadata({
  searchParams,
}: PageProps<"/">): Promise<Metadata> {
  // Await request data before reading SITE_URL: one prebuilt image serves any domain.
  return homeMetadata(Object.keys(await searchParams).length > 0);
}

// Settings (cookie) and the search (URL) are per request, and the metadata already holds the <head> for them, so
// the screen renders with the document rather than behind a boundary that React would reveal 300 ms later.
export default function Home({ searchParams }: PageProps<"/">) {
  return <HomeScreen searchParams={searchParams} />;
}
