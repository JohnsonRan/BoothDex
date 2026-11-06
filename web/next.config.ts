import type { NextConfig } from "next";

// The Rust services sit behind the same origin so session cookies stay first-party (SameSite=Lax) and
// CORS never needs credentials. Next.js forwards /api and /img to them, in development and in the container
// (deploy/web.Dockerfile sets the service addresses; rewrites are baked in at build time).
const API_ORIGIN = process.env.API_ORIGIN ?? "http://127.0.0.1:3000";
const IMG_ORIGIN = process.env.IMG_ORIGIN ?? "http://127.0.0.1:3001";

const nextConfig: NextConfig = {
  // self-contained server for the container image (deploy/web.Dockerfile)
  output: "standalone",
  // Keep default gzip for HTML/JS/CSS; the API's SSE uses Cache-Control: no-transform to bypass it.
  cacheComponents: true,
  partialPrefetching: false,
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
      { source: "/img/:path*", destination: `${IMG_ORIGIN}/:path*` },
    ];
  },
};

export default nextConfig;
