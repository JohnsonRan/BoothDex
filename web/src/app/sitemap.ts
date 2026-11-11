import type { MetadataRoute } from "next";
import { connection } from "next/server";
import { getSiteUrl } from "@/lib/seo";

export default async function sitemap(): Promise<MetadataRoute.Sitemap> {
  await connection();
  const url = getSiteUrl();
  return url
    ? ["/", "/privacy", "/terms"].map((path) => ({
        url: new URL(path, url).href,
      }))
    : [];
}
