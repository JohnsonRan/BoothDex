import type { Metadata } from "next";

/** The link preview picture, drawn by app/og.png/route.tsx. */
export const SHARE_IMAGE = {
  url: "/og.png",
  width: 1200,
  height: 630,
  type: "image/png",
  alt: "BoothDex — VRChat assets, avatars and clothing on BOOTH",
};

let warnedMissingSiteUrl = false;
let warnedInvalidSiteUrl = false;

export function getSiteUrl(value = process.env.SITE_URL): URL | undefined {
  if (!value) {
    if (process.env.NODE_ENV === "production") {
      if (!warnedMissingSiteUrl) {
        warnedMissingSiteUrl = true;
        console.warn(
          "SITE_URL is not set; canonical URLs, metadataBase, and sitemap are disabled",
        );
      }
      return undefined;
    }
    return new URL("http://localhost:3100");
  }
  try {
    const url = new URL(value);
    if (
      !["http:", "https:"].includes(url.protocol) ||
      url.username ||
      url.password ||
      url.pathname !== "/" ||
      url.search ||
      url.hash
    ) {
      throw new Error();
    }
    return url;
  } catch {
    if (!warnedInvalidSiteUrl) {
      warnedInvalidSiteUrl = true;
      console.warn(
        "SITE_URL must be an HTTP(S) origin without credentials, path, query or fragment; canonical URLs, metadataBase, and sitemap are disabled",
      );
    }
    return undefined;
  }
}

export function homeMetadata(hasQuery: boolean): Metadata {
  const url = getSiteUrl();
  const title = "BoothDex — VRChat Assets, Avatars & Clothing on BOOTH";
  const description =
    "Discover VRChat avatars, clothing, accessories and tools on BOOTH. Search by avatar compatibility, category, price and features across multiple languages.";
  return {
    title,
    description,
    applicationName: "BoothDex",
    ...(url && {
      metadataBase: url,
      alternates: { canonical: url.href },
    }),
    // Search, filter and personal views share one app, not separate indexable pages.
    robots: { index: !hasQuery, follow: true },
    openGraph: {
      type: "website",
      siteName: "BoothDex",
      title,
      description,
      ...(url && { url: url.href, images: [SHARE_IMAGE] }),
    },
    // Next gives twitter:image the openGraph picture
    twitter: { card: "summary_large_image", title, description },
  };
}
