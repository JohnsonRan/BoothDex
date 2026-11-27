import type { Metadata } from "next";

/** The link preview picture, drawn by app/og.png/route.tsx; next.config.ts sets the hash. */
export const SHARE_IMAGE = {
  url: `/og.png?${process.env.SHARE_IMAGE_HASH}`,
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

export type HomeParams = Record<string, string | string[] | undefined>;

const DEFAULT_TITLE = "BoothDex — VRChat Assets, Avatars & Clothing on BOOTH";

const paramText = (v: string | string[] | undefined) => {
  const raw = Array.isArray(v) ? v[0] : v;
  const trimmed = raw?.trim();
  return trimmed ? trimmed.slice(0, 80) : undefined;
};

function homeTitle(params: HomeParams | undefined): string {
  if (!params) return DEFAULT_TITLE;
  const q = paramText(params.q);
  if (q) return `Search: ${q} — BoothDex`;
  const v = paramText(params.v);
  if (v === "favourites") return "My favourites — BoothDex";
  if (v === "recent") return "Recently viewed — BoothDex";
  if (v === "avatars") return "Avatar bodies — BoothDex";
  const author = paramText(params.author);
  if (author) return `${author} — BoothDex`;
  return DEFAULT_TITLE;
}

export function homeMetadata(queryOrParams: boolean | HomeParams): Metadata {
  const url = getSiteUrl();
  const hasQuery =
    typeof queryOrParams === "boolean"
      ? queryOrParams
      : Object.keys(queryOrParams).length > 0;
  const title = homeTitle(
    typeof queryOrParams === "boolean" ? undefined : queryOrParams,
  );
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
