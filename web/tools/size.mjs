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
// Perf run 3: the later tiles fetched by the browser, the pager loaded on demand (gone with the endless grid below),
// one name per avatar: HTML 43,684, JS 388,417, CSS 43,262. Upstream's additions since the Astryx merge (JS
// 396,491, CSS 44,116) and the endless grid that replaced the pager: JS 397,557, CSS 44,265, held to the next 500
// and 250 B (owner, 2026-11-08).
// The sticky filter band, the folding avatar strip and the pages above a deep start: JS 399,319, CSS 44,327,
// held to the next 500 B as before. The deep start's first paint, with the strip's earlier design: JS 400,319,
// CSS 44,409, held to the next 500 B (owner, 2026-11-08). One strip shrinking with the scroll and the boxes'
// painted silhouettes: JS 400,918, CSS 44,958, held to the next 500 B as before. The review fixes (tiles redrawn
// after a settings change, the band's edge set as it attaches, Astryx parts for the boxes): JS 401,132, held to
// the next 500 B as before. The strip's toggle on the label line, with the cards clipped clear of it as they shrink:
// CSS 45,105, held to the next 500 B as before.
const BUDGET = { html: 48_000, js: 401_500, css: 45_500 };

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
