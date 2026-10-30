import type { NextConfig } from "next";

// The Rust services sit behind the same origin so session cookies stay first-party (SameSite=Lax) and
// CORS never needs credentials. In production a reverse proxy can route /api and /img directly instead.
const API_ORIGIN = process.env.API_ORIGIN ?? "http://127.0.0.1:3000";
const IMG_ORIGIN = process.env.IMG_ORIGIN ?? "http://127.0.0.1:3001";

const nextConfig: NextConfig = {
  cacheComponents: true,
  partialPrefetching: true,
  turbopack: {
    rules: {
      "*.css": {
        loaders: ["@tailwindcss/turbopack"],
        as: "*.css",
      },
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
