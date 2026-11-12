import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import type { NextConfig } from "next";

// The Rust services sit behind the same origin so session cookies stay first-party (SameSite=Lax) and
// CORS never needs credentials. Next.js forwards /api and /img to them, in development and in the container
// (deploy/web.Dockerfile sets the service addresses; rewrites are baked in at build time).
const API_ORIGIN = process.env.API_ORIGIN ?? "http://127.0.0.1:3000";
const IMG_ORIGIN = process.env.IMG_ORIGIN ?? "http://127.0.0.1:3001";

// /og.png is cached for a year, so SHARE_IMAGE in lib/seo.ts links it with a hash of the files that draw and
// size it; a changed picture gets a new URL.
const SHARE_IMAGE_HASH = createHash("sha256")
  .update(readFileSync("src/app/og.png/route.tsx"))
  .update(readFileSync("src/lib/seo.ts"))
  .digest("hex")
  .slice(0, 16);

const nextConfig: NextConfig = {
  // self-contained server for the container image (deploy/web.Dockerfile)
  output: "standalone",
  env: { SHARE_IMAGE_HASH },
  // Keep default gzip for HTML/JS/CSS; the API's SSE uses Cache-Control: no-transform to bypass it.
  cacheComponents: true,
  partialPrefetching: false,
  // The home page's metadata reads searchParams (searches are noindex), so Next streams it into <body> for any
  // user agent outside its bot list, and Lighthouse and link previews found no description. It resolves at once
  // (no fetch), so every request gets it in <head> at no cost (docs: next-config-js/htmlLimitedBots).
  htmlLimitedBots: /.*/,
  // proxy.ts hands the home page its query string as sent; normalising re-encodes a "%20" as "+"
  skipProxyUrlNormalize: true,
  turbopack: {
    // the theme's font stylesheet is a render-blocking Google Fonts @import; Figtree comes from next/font
    resolveAlias: {
      "@astryxdesign/theme-neutral/fonts.css": "./src/app/theme-fonts.css",
    },
  },
  async rewrites() {
    return [
      { source: "/api/:path*", destination: `${API_ORIGIN}/api/:path*` },
      // imgproxy's image routes only: its /stats is for the admin overview, which the API reads directly
      {
        source: "/img/:kind(b|u|v)/:path*",
        destination: `${IMG_ORIGIN}/:kind/:path*`,
      },
    ];
  },
};

export default nextConfig;
