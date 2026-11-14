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
// the next 500 B as before. The strip's cards morphing into chips, kept shrunk through a filter change: JS 403,120,
// CSS 45,330, held to the next 500 B as before. The account's view counts and favourites since then: JS 408,480; the
// filter sidebar, its code loaded only where it is shown (the lazy import still costs 1,366 B: its components are
// shared with other lazy surfaces, which splits a chunk of loaders into the first load): JS 409,846, CSS 45,367,
// held to the next 500 B (owner, 2026-11-11). The language, currency and sort menus' code loaded after the page,
// their buttons drawn by the server as before: JS 395,598, the budget kept for what comes next (owner, 2026-11-11).
// The filter sidebar shown by default, drawn by the server (HTML 47,500-49,000 over 7 reads, with the toolbar's row
// 44,100), the dialogs' fading lower edge (CSS 45,503) and the presses kept for late menus and hearts (JS 396,067):
// HTML to 49,500 and CSS to 45,750 (owner, 2026-11-12). Astryx 0.7.15, its motion, gesture and layer code in the
// modules the frame and the filter rail use (HTML 50,696, JS 497,886, CSS 49,895 with the API; 0.7.8 read HTML
// 48,677 on the same data): HTML to 51,500, JS to 498,000, CSS to 50,000 (owner, 2026-11-13). The chunks the
// page preloads, counted from then on (three, the filter rail's among them): the same code read JS 495,444 without them
// and 520,593 with them, and 520,618 with the rail handle's value text: JS to 521,000 (owner, 2026-11-13). Astryx
// 0.7.16 (JS 520,914 to 523,999, CSS 49,901 to 50,561 on the same host; the layer, menu and tooltip fixes and the
// forced-colors edges) and the refresh bar's code loaded after the page (JS 521,999): JS to 522,500, CSS to
// 50,750 (owner, 2026-11-14).
const BUDGET = { html: 51_500, js: 522_500, css: 50_750 };

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
// A chunk the page preloads (`<link rel="preload" as="script">`, such as the filter rail's) is fetched with the first
// load as a script tag's is, so it counts too.
const preloads = [...text.matchAll(/<link\b[^>]*>/g)]
  .map((m) => m[0])
  .filter(
    (tag) =>
      /\brel="modulepreload"/.test(tag) ||
      (/\brel="preload"/.test(tag) && /\bas="script"/.test(tag)),
  )
  .map((tag) => tag.match(/\bhref="([^"]+)"/)?.[1])
  .filter(Boolean)
  .map((href) => new URL(href, page).href);
const js = [...new Set([...refs(/<script[^>]+src="([^"]+)"/g), ...preloads])];
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
  console.error(
    "find what grew with tools/size-diff.mjs (its header says how) before proposing a raise",
  );
  process.exit(1);
}
