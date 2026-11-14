import { expect, test } from "bun:test";
import nextConfig, { shareImageHash } from "./next.config";

/** The one policy every path is sent, as each directive's name to its sources. */
const policy = async () => {
  const rules = (await nextConfig.headers?.()) ?? [];
  const csp = rules.flatMap((rule) =>
    rule.headers
      .filter((h) => h.key.toLowerCase() === "content-security-policy")
      .map((h) => ({ rule, value: h.value })),
  );
  // one unconditional rule for every path: a second rule or a condition could send another policy somewhere
  expect(csp.length).toBe(1);
  const [{ rule, value }] = csp;
  expect(rule.source).toBe("/(.*)");
  expect(rule.has).toBeUndefined();
  expect(rule.missing).toBeUndefined();
  return new Map(
    value.split(";").map((directive) => {
      const [name, ...sources] = directive.trim().split(/\s+/);
      return [name, sources];
    }),
  );
};

test("every path is sent one policy that allows images from this origin and data: only", async () => {
  const imgSrc = (await policy()).get("img-src");
  expect(imgSrc?.length).toBeGreaterThan(0);
  for (const source of imgSrc ?? [])
    expect(["'self'", "data:"]).toContain(source);
});

test("the policy refuses plugin content, a <base> to another origin and framing by any page", async () => {
  const csp = await policy();
  expect(csp.get("object-src")).toEqual(["'none'"]);
  expect(csp.get("base-uri")).toEqual(["'self'"]);
  expect(csp.get("frame-ancestors")).toEqual(["'none'"]);
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
