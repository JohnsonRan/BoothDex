// Types and helpers for the public catalogue. Filter state lives in the URL and uses the API's own
// query format (VRCatalogue-compatible): list values are joined with a literal "+".

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
export type Variation = {
  id: string;
  name: string;
  price_jpy: number | null;
  status: string;
  type: string;
};
export type Item = {
  id: number;
  title: string;
  bio: string;
  author_name: string;
  author_uri: string;
  author_icon: string | null;
  booth_category: string | null;
  price_jpy: number | null;
  smart_price: number | null;
  smart_price_reason: string | null;
  likes: number;
  is_adult: boolean;
  content_rating: "SFW" | "Suggestive" | "NSFW" | null;
  quest: string | null;
  features: string[];
  status: "ok" | "delisted";
  published_at: string | null;
  scraped_at: string;
  media: string[];
  booth_tags: string[];
  booth_images: string[] | null;
  booth_images_complete: boolean | null;
  variations: Variation[];
  translations: Partial<
    Record<
      "zh" | "en" | "ja" | "ko",
      { title?: string; bio?: string; variations?: Record<string, string> }
    >
  > & { original_language?: string };
  linked_avatars: Avatar[];
  needs_enrichment?: boolean;
  avatar_slug?: string | null;
  supported?: boolean;
  gender?: "female" | "male" | null;
  last_listed_at?: string | null;
  enrich_failures?: number;
};
export type SearchResult = {
  count: number;
  page: number;
  totalPages: number;
  smartCounts: Record<string, number>;
  featureCounts: Record<string, number>;
  items: Item[];
  /** meaning-based matching: used, off, unavailable (provider slow or failing), skipped */
  semantic?: "used" | "off" | "unavailable" | "skipped";
  /** server-side steps of this request, in order */
  trace?: { step: string; ms: number }[];
  timing?: { searchMs: number; totalMs: number };
};

export const FEATURES = {
  quest: "Quest",
  ma: "MA",
  liltoon: "lilToon",
  poiyomi: "Poiyomi",
  vrcfury: "VRCFury",
} as const;
export type Feature = keyof typeof FEATURES;

export type Size =
  | "48x48"
  | "72x72_a2_g5"
  | "300x300_a2_g5"
  | "620x620"
  | "orig";

/** Proxied, WebP-encoded image URL; delisted items fall back to our archived copies. */
export function imgUrl(
  it: Pick<Item, "booth_images" | "media">,
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
export const imageCount = (it: Pick<Item, "booth_images" | "media">) =>
  Math.max(it.booth_images?.length ?? 0, it.media?.length ?? 0);

export const priceOf = (it: Pick<Item, "smart_price" | "price_jpy">) =>
  it.smart_price ?? it.price_jpy;
export function priceText(it: Pick<Item, "smart_price" | "price_jpy">) {
  const p = priceOf(it);
  return p == null ? "—" : p === 0 ? "免费" : `¥${p.toLocaleString()}`;
}
export const isNew = (it: Pick<Item, "published_at">) =>
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
