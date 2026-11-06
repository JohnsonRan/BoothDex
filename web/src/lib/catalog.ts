// Types and helpers for the public catalogue. Filter state lives in the URL and uses the API's own
// query format (VRCatalogue-compatible): list values are joined with a literal "+".

// Item and search types come from the Rust API (generated, see api-types.ts).
import type { ItemResponse } from "./api-types";

export type Avatar = {
  slug: string;
  en: string;
  ja?: string | null;
  ko?: string | null;
  image_url?: string | null;
  item_count?: number;
};
export type Category = {
  slug: string;
  en: string;
  ja?: string;
  ko?: string;
  /** AI-translated; null until the localize job has run */
  zh?: string | null;
  mode?: string;
  children: Category[];
};

/** Taxonomy label in the interface language. Chinese falls back to English until the AI translation exists. */
export const localName = (
  c: { en: string; ja?: string | null; ko?: string | null; zh?: string | null },
  ui: string,
) =>
  (ui === "zh" ? c.zh : ui === "ja" ? c.ja : ui === "ko" ? c.ko : c.en) || c.en;

/** Smart tag label: Chinese when translated, otherwise the original display name. */
export const tagName = (
  t: { display: string; display_zh?: string | null },
  ui: string,
) => (ui === "zh" && t.display_zh) || t.display;

export const FEATURES = {
  quest: "Quest",
  ma: "MA",
  liltoon: "lilToon",
  poiyomi: "Poiyomi",
  vrcfury: "VRCFury",
} as const;
export type Feature = keyof typeof FEATURES;

type Size = "48x48" | "72x72_a2_g5" | "300x300_a2_g5" | "620x620" | "orig";

/** Proxied, WebP-encoded image URL; delisted items fall back to our archived copies. */
export function imgUrl(
  it: Pick<ItemResponse, "booth_images" | "media">,
  i = 0,
  size: Size = "300x300_a2_g5",
): string {
  const b = it.booth_images?.[i];
  if (b) return b.replace("https://booth.pximg.net/", `/img/b/${size}/`);
  const m = it.media?.[i];
  return m && ARCHIVED.test(m) ? imageSrc(m) : "";
}
const ARCHIVED = /\/booth\/([0-9a-f]{40})\.webp$/;
/** Avatar thumbnails and shop icons: Booth images through our proxy, VRCatalogue CDN links (gone) from our archive. */
export function imageSrc(
  u: string | null | undefined,
  size: Size = "72x72_a2_g5",
): string {
  if (!u) return "";
  const sha = u.match(ARCHIVED)?.[1];
  if (sha) return `/img/v/${sha}.webp`;
  // already-sized links (shop icons under /c/48x48/) load from Booth as they are
  if (
    u.startsWith("https://booth.pximg.net/") &&
    !u.startsWith("https://booth.pximg.net/c/")
  )
    return u.replace("https://booth.pximg.net/", `/img/b/${size}/`);
  return u;
}
export const imageCount = (it: Pick<ItemResponse, "booth_images" | "media">) =>
  Math.max(it.booth_images?.length ?? 0, it.media?.length ?? 0);

export const priceOf = (it: Pick<ItemResponse, "smart_price" | "price_jpy">) =>
  it.smart_price ?? it.price_jpy;
export const isNew = (it: Pick<ItemResponse, "published_at">) =>
  !!it.published_at && Date.now() - Date.parse(it.published_at) < 7 * 864e5;
/** Avatar name for the interface language; Chinese UI uses the Japanese name, as the community does. */
export const avatarName = (a: Avatar, ui: string = "ja") =>
  ui === "en" ? a.en : ui === "ko" ? a.ko || a.en : a.ja || a.en;

// ---------- filter state <-> URL ----------

export const RANGES = ["7d", "30d", "90d", "1y"] as const;
export type Range = (typeof RANGES)[number] | "";

export type Filters = {
  /** items, avatar bodies (API v=avatars), the signed-in user's favourites, or recently viewed (site only) */
  view: "items" | "avatars" | "favourites" | "recent";
  q: string;
  avatar: string;
  sc: string;
  features: Feature[];
  author: string;
  /** smart tags to require (any) / exclude */
  tags: string[];
  extags: string[];
  adult: "safe" | "include";
  free: boolean;
  /** yen; 0 = no bound */
  pmin: number;
  pmax: number;
  range: Range;
  /** avatars view only */
  gender: "" | "female" | "male";
  asup: "supported" | "all";
  sort: "trending" | "newest" | "likes" | "price";
  dir: "asc" | "desc";
};
export const DEFAULT_FILTERS: Filters = {
  view: "items",
  q: "",
  avatar: "",
  sc: "",
  features: [],
  author: "",
  tags: [],
  extags: [],
  adult: "safe",
  free: false,
  pmin: 0,
  pmax: 0,
  range: "",
  gender: "",
  asup: "supported",
  sort: "trending",
  dir: "desc",
};

/** Request header carrying the home page's raw query string (proxy.ts), the only form `parseFilters` can read. */
export const RAW_QUERY_HEADER = "x-raw-query";

/** Reads filters from a query string, splitting list values on a literal "+" like the API does. */
export function parseFilters(search: string): Filters {
  const raw = new Map<string, string[]>();
  for (const pair of search.replace(/^\?/, "").split("&").filter(Boolean)) {
    const [k, v = ""] = pair.split("=");
    if (raw.has(k)) continue;
    raw.set(
      decodeURIComponent(k),
      v
        .split("+")
        .map((s) => decodeURIComponent(s))
        .filter(Boolean),
    );
  }
  const one = (k: string) => raw.get(k)?.join(" ") ?? "";
  const num = (k: string) => Math.max(0, Number.parseInt(one(k), 10) || 0);
  const sort = one("sort");
  const v = one("v");
  const gender = one("gender");
  return {
    view: v === "favourites" || v === "avatars" || v === "recent" ? v : "items",
    tags: (raw.get("tags") ?? []).slice(0, 10),
    extags: (raw.get("extags") ?? []).slice(0, 10),
    free: one("free") === "1",
    pmin: num("pmin"),
    pmax: num("pmax"),
    range: RANGES.find((r) => r === one("range")) ?? "",
    gender: gender === "female" || gender === "male" ? gender : "",
    asup: one("asup") === "all" ? "all" : "supported",
    q: one("q"),
    avatar: raw.get("avatars")?.[0] ?? "",
    sc: raw.get("sc")?.[0] ?? "",
    features: (raw.get("if") ?? []).filter((f): f is Feature => f in FEATURES),
    author: one("authors"),
    adult: one("adult") === "include" ? "include" : "safe",
    sort:
      (["trending", "newest", "likes", "price"] as const).find(
        (s) => s === sort,
      ) ?? "trending",
    dir: one("dir") === "asc" ? "asc" : "desc",
  };
}

/** Query string for both the page URL and /api/search (defaults omitted). */
export function filtersToQuery(f: Filters): string {
  const parts: string[] = [];
  const enc = encodeURIComponent;
  if (f.view !== "items") parts.push(`v=${f.view}`);
  if (f.q) parts.push(`q=${enc(f.q)}`);
  if (f.avatar) parts.push(`avatars=${enc(f.avatar)}`);
  if (f.sc) parts.push(`sc=${enc(f.sc)}`);
  if (f.features.length) parts.push(`if=${f.features.map(enc).join("+")}`);
  if (f.author) parts.push(`authors=${enc(f.author)}`);
  if (f.tags.length) parts.push(`tags=${f.tags.map(enc).join("+")}`);
  if (f.extags.length) parts.push(`extags=${f.extags.map(enc).join("+")}`);
  if (f.adult !== "safe") parts.push(`adult=${f.adult}`);
  if (f.free) parts.push("free=1");
  if (!f.free && f.pmin > 0) parts.push(`pmin=${f.pmin}`);
  if (!f.free && f.pmax > 0) parts.push(`pmax=${f.pmax}`);
  if (f.range) parts.push(`range=${f.range}`);
  if (f.view === "avatars" && f.gender) parts.push(`gender=${f.gender}`);
  if (f.view === "avatars" && f.asup !== "supported")
    parts.push(`asup=${f.asup}`);
  if (f.sort !== "trending") parts.push(`sort=${f.sort}`);
  if (f.dir !== "desc") parts.push(`dir=${f.dir}`);
  return parts.join("&");
}

export function searchQuery(f: Filters, page: number, pageSize = 30) {
  // a text query without an explicit sort ranks by relevance
  const sort = f.q && f.sort === "trending" ? "&sort=relevant" : "";
  return `${filtersToQuery(f)}${sort}&page=${page}&pageSize=${pageSize}`.replace(
    /^&/,
    "",
  );
}

/** The query's words, each a token in the search field: a quoted phrase stays one word. */
export const queryWords = (q: string) => q.match(/"[^"]*"?|[^\s"]+/g) ?? [];

/**
 * The search field's tokens for `words` (`queryWords`): a word each, but the words of a mark (a model or category
 * the search read them as) one token together, when they stand together in the query; a mark whose words do not
 * ("short mao hair": the category's "short hair") marks nothing. The caller drops the marks of an earlier query
 * before calling (filter-bar.tsx), so `marks` is empty or the current query's.
 */
export function markTokens<M extends { words: string[] }>(
  words: string[],
  marks: M[],
) {
  const starts = new Map<number, M>();
  const taken = new Set<number>();
  for (const m of marks) {
    const n = m.words.length;
    const at = words.findIndex(
      (_, i) =>
        n > 0 &&
        m.words.every((w, k) => words[i + k] === w && !taken.has(i + k)),
    );
    if (at < 0) continue;
    starts.set(at, m);
    for (let k = 0; k < n; k++) taken.add(at + k);
  }
  const out: { label: string; mark?: M }[] = [];
  for (let i = 0; i < words.length; ) {
    const mark = starts.get(i);
    const n = mark?.words.length ?? 1;
    out.push({ label: words.slice(i, i + n).join(" "), mark });
    i += n;
  }
  return out;
}

/**
 * The width in rem the search field's tokens take for `q` (catalog/search-suggest.tsx), so the field grows with
 * the query, the plain field before them as wide already. Measured on the field's `sm` tokens: a token is 1.85rem
 * besides its text, 0.25rem apart; a Latin letter about 0.32rem, a kana or kanji 0.75rem. 6rem: the search
 * icon, the clear button and the paddings, and 1rem for a mark's picture (`markTokens`), which arrives with the
 * answer: the field keeps its width then (a model's token is 1rem wider, a category's words one token, narrower).
 */
export function searchWidth(q: string) {
  let rem = 6;
  for (const w of queryWords(q)) {
    rem += 2.1;
    for (const c of w) rem += (c.codePointAt(0) ?? 0) >= 0x2e80 ? 0.75 : 0.32;
  }
  return Math.round(rem * 10) / 10;
}

/** What "clear" resets: every filter, while the view, query, shop, avatar and sort stay. */
export const FILTER_RESET: Partial<Filters> = {
  sc: "",
  features: [],
  adult: "safe",
  tags: [],
  extags: [],
  free: false,
  pmin: 0,
  pmax: 0,
  range: "",
  gender: "",
  asup: "supported",
};

/** Number of filters set, for the filter button's count. */
export const activeFilterCount = (f: Filters) =>
  (f.sc ? 1 : 0) +
  f.features.length +
  (f.adult !== "safe" ? 1 : 0) +
  f.tags.length +
  f.extags.length +
  (f.free || f.pmin || f.pmax ? 1 : 0) +
  (f.range ? 1 : 0) +
  (f.view === "avatars" && f.gender ? 1 : 0) +
  (f.view === "avatars" && f.asup !== "supported" ? 1 : 0);
