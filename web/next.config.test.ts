import { expect, test } from "bun:test";
import { shareImageHash } from "./next.config";

test("the share image hash changes with each font and renderer file that draws /og.png", () => {
  const base = shareImageHash((path) => path);
  for (const changed of [
    "node_modules/@fontsource/figtree/files/figtree-latin-500-normal.woff",
    "node_modules/@fontsource/figtree/files/figtree-latin-700-normal.woff",
    "node_modules/next/dist/compiled/@vercel/og/index.node.js",
    "node_modules/next/dist/compiled/@vercel/og/resvg.wasm",
  ])
    expect(
      shareImageHash((path) => (path === changed ? `${path} v2` : path)),
    ).not.toBe(base);
});
