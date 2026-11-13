import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import type { NextConfig } from "next";

// The Rust services sit behind the same origin so session cookies stay first-party (SameSite=Lax) and
// CORS never needs credentials. Next.js forwards /api and /img to them, in development and in the container
// (deploy/web.Dockerfile sets the service addresses; rewrites are baked in at build time).
const API_ORIGIN = process.env.API_ORIGIN ?? "http://127.0.0.1:3000";
const IMG_ORIGIN = process.env.IMG_ORIGIN ?? "http://127.0.0.1:3001";

// Every remote image reaches the browser through /img (imgUrl and imageSrc in lib/catalog.ts), so the browser refuses
// an image from any other host. data: carries the silhouettes drawn in CSS (components/catalog/silhouette.tsx).
// The download's blob: URL is a link target, which img-src does not govern. Scripts and styles are not restricted.
const CONTENT_SECURITY_POLICY = "img-src 'self' data:";

// Social platforms cache a share image by its URL, so SHARE_IMAGE in lib/seo.ts links /og.png with a hash of the
// files that draw and size it; a changed picture gets a new URL. The fonts are the ones the route reads. next/og
// lays the picture out with satori (index.node.js) and rasterises it with sharp when it can import it, with resvg
// otherwise; sharp's package.json pins its libvips build. A dependency upgrade that redraws the picture, or sharp
// appearing or going away, also changes the URL.
const SHARP = "node_modules/sharp/package.json";
const SHARE_IMAGE_INPUTS = [
  "src/app/og.png/route.tsx",
  "src/lib/seo.ts",
  "node_modules/@fontsource/figtree/files/figtree-latin-500-normal.woff",
  "node_modules/@fontsource/figtree/files/figtree-latin-700-normal.woff",
  "node_modules/next/dist/compiled/@vercel/og/index.node.js",
  "node_modules/next/dist/compiled/@vercel/og/resvg.wasm",
  SHARP,
];
const readInput = (path: string) =>
  path === SHARP && !existsSync(path) ? "no sharp" : readFileSync(path);
export const shareImageHash = (
  read: (path: string) => string | Uint8Array = readInput,
) => {
  const hash = createHash("sha256");
  for (const path of SHARE_IMAGE_INPUTS) hash.update(read(path));
  return hash.digest("hex").slice(0, 16);
};
const SHARE_IMAGE_HASH = shareImageHash();

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
  async headers() {
    return [
      {
        source: "/(.*)",
        headers: [
          { key: "Content-Security-Policy", value: CONTENT_SECURITY_POLICY },
        ],
      },
    ];
  },
  async rewrites() {
    return [
      { source: "/api/:path*", destination: `${API_ORIGIN}/api/:path*` },
      // imgproxy's image routes only: its /stats is for the admin overview, which the API reads directly
      {
        source: "/img/:kind(a|b|f|u|v)/:path*",
        destination: `${IMG_ORIGIN}/:kind/:path*`,
      },
    ];
  },
};

export default nextConfig;
