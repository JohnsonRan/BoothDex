import { expect, test } from "bun:test";
import nextConfig, { shareImageHash } from "./next.config";

test("every path is sent one policy that allows images from this origin and data: only", async () => {
  const rules = (await nextConfig.headers?.()) ?? [];
  const csp = rules.flatMap((rule) =>
    rule.headers
      .filter((h) => h.key.toLowerCase() === "content-security-policy")
      .map((h) => ({ rule, policy: h.value })),
  );
  // one unconditional rule for every path: a second rule or a condition could send another policy somewhere
  expect(csp.length).toBe(1);
  const [{ rule, policy }] = csp;
  expect(rule.source).toBe("/(.*)");
  expect(rule.has).toBeUndefined();
  expect(rule.missing).toBeUndefined();
  const imgSrc = policy
    .split(";")
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
