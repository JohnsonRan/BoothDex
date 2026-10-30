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
  mode?: string;
  children: Category[];
};
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
      "en" | "ja" | "ko",
      { title?: string; bio?: string; variations?: Record<string, string> }
    >
  > & { original_language?: string };
  linked_avatars: Avatar[];
  needs_enrichment?: boolean;
};
export type SearchResult = {
  count: number;
  page: number;
  totalPages: number;
  smartCounts: Record<string, number>;
  featureCounts: Record<string, number>;
  items: Item[];
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
  const sha = m?.match(/\/booth\/([0-9a-f]{40})\.webp$/)?.[1];
  return sha ? `/img/v/${sha}.webp` : "";
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
export const avatarName = (a: Avatar) => a.ja || a.en;

// ---------- filter state <-> URL ----------

export type Filters = {
  q: string;
  avatar: string;
  sc: string;
  features: Feature[];
  author: string;
  adult: "safe" | "include";
  sort: "trending" | "newest" | "likes" | "price";
  dir: "asc" | "desc";
};
export const DEFAULT_FILTERS: Filters = {
  q: "",
  avatar: "",
  sc: "",
  features: [],
  author: "",
  adult: "safe",
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
  const sort = one("sort");
  return {
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
  if (f.q) parts.push(`q=${enc(f.q)}`);
  if (f.avatar) parts.push(`avatars=${enc(f.avatar)}`);
  if (f.sc) parts.push(`sc=${enc(f.sc)}`);
  if (f.features.length) parts.push(`if=${f.features.map(enc).join("+")}`);
  if (f.author) parts.push(`authors=${enc(f.author)}`);
  if (f.adult !== "safe") parts.push(`adult=${f.adult}`);
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
