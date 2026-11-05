// Size budget for the home page's first load (modelled on doona's apps/lite/tools/size.mjs): gzip level 9 bytes
// of the HTML of `/` and of every script and stylesheet it references. The page is rendered per request, so this
// reads a running production server: `bun run build && bun run start`, then `bun run check:size [url]`.
// Lighthouse's simulated mobile load is bound by these bytes (docs/TEMPLATE-MAP.md, .shots/HANDOFF.md).
import { gzipSync } from "node:zlib";

const page = new URL(process.argv[2] ?? "http://127.0.0.1:3100/");
// Bytes after gzip -9, from the measured build plus ~2 % (the HTML ~10 %: it carries live titles).
// Measured 2026-11-06 (HTML 49,937, JS 362,176, CSS 42,979; master: 7,981 / 287,456 / 18,400). After the filter
// row, the item dialog's host and the account (session, delegated favourites, viewed marks): JS 410,604; with the
// browser given only the visitor's dictionary (HTML carries it: 57,004) and on-demand icons and Banner: 396,116.
const BUDGET = { html: 62_000, js: 404_000, css: 45_000 };

const gz = (buf) => gzipSync(buf, { level: 9 }).length;
const get = async (url) => {
  const res = await fetch(url, { headers: { "accept-encoding": "identity" } });
  if (!res.ok) throw new Error(`${url}: ${res.status}`);
  return Buffer.from(await res.arrayBuffer());
};

const html = await get(page);
const text = html.toString("utf8");
const refs = (re) => [
  ...new Set([...text.matchAll(re)].map((m) => new URL(m[1], page).href)),
];
const js = refs(/<script[^>]+src="([^"]+)"/g);
const css = refs(/<link[^>]+rel="stylesheet"[^>]+href="([^"]+)"/g);
const total = async (urls) =>
  (await Promise.all(urls.map(async (u) => gz(await get(u))))).reduce(
    (a, b) => a + b,
    0,
  );
const rows = {
  html: { files: 1, bytes: gz(html) },
  js: { files: js.length, bytes: await total(js) },
  css: { files: css.length, bytes: await total(css) },
};
for (const [k, r] of Object.entries(rows)) r.budget = BUDGET[k];
console.table(rows);
const over = Object.entries(rows).filter(([, r]) => r.bytes > r.budget);
if (over.length) {
  console.error(`over budget: ${over.map(([k]) => k).join(", ")}`);
  process.exit(1);
}
