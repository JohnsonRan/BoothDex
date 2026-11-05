import type { Metadata } from "next";
import { Suspense } from "react";
import { HomeScreen } from "@/components/catalog/home-screen";
import { homeMetadata } from "@/lib/seo";

export async function generateMetadata({
  searchParams,
}: PageProps<"/">): Promise<Metadata> {
  // Await request data before reading SITE_URL: one prebuilt image serves any domain.
  return homeMetadata(Object.keys(await searchParams).length > 0);
}

// Settings (cookie) and the search (URL) are per request, so the screen streams in behind one boundary.
export default function Home({ searchParams }: PageProps<"/">) {
  return (
    <Suspense>
      <HomeScreen searchParams={searchParams} />
    </Suspense>
  );
}
