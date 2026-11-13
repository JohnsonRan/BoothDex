import { expect, test } from "bun:test";
import nextConfig, { shareImageHash } from "./next.config";

test("every path is sent a policy that allows images from this origin and data: only", async () => {
  const rules = (await nextConfig.headers?.()) ?? [];
  const policy = rules
    .find((rule) => rule.source === "/(.*)")
    ?.headers.find((h) => h.key === "Content-Security-Policy")?.value;
  const imgSrc = policy
    ?.split(";")
    .map((directive) => directive.trim().split(/\s+/))
    .find(([name]) => name === "img-src")
    ?.slice(1);
  expect(imgSrc?.length).toBeGreaterThan(0);
  for (const source of imgSrc ?? [])
    expect(["'self'", "data:"]).toContain(source);
});

test("the share image hash changes with each font and renderer file that draws /og.png", () => {
  const base = shareImageHash((path) => path);
  for (const changed of [
    "node_modules/@fontsource/figtree/files/figtree-latin-500-normal.woff",
    "node_modules/@fontsource/figtree/files/figtree-latin-700-normal.woff",
    "node_modules/next/dist/compiled/@vercel/og/index.node.js",
    "node_modules/next/dist/compiled/@vercel/og/resvg.wasm",
    "node_modules/sharp/package.json",
  ])
    expect(
      shareImageHash((path) => (path === changed ? `${path} v2` : path)),
    ).not.toBe(base);
});
