// Reads for the first screen, done on the server so the HTML already carries the results and their covers.
// The Rust API is reached directly (API_ORIGIN), not through the site's /api rewrite. next/headers keeps this
// module out of client bundles.
import { cacheLife } from "next/cache";
import { cookies, headers } from "next/headers";
import type { SearchResponse } from "./api-types";
import {
  type Avatar,
  type Category,
  type Filters,
  searchQuery,
  tagName,
} from "./catalog";
import {
  CURRENCIES,
  parseSettings,
  type Rates,
  SETTINGS_COOKIE,
  type Settings,
} from "./settings-core";

const API_ORIGIN = process.env.API_ORIGIN ?? "http://127.0.0.1:3000";
const PAGE_SIZE = 100;
/** How long the rate service may take before prices fall back to yen (the results wait for the rates). */
const RATES_TIMEOUT_MS = 2000;

async function getJson<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${API_ORIGIN}${path}`, init);
  if (!res.ok) throw new Error(`${path}: ${res.status}`);
  return res.json() as Promise<T>;
}

/** The visitor's settings (cookie, else Accept-Language), so the first HTML is already in their language. */
export async function readSettings(): Promise<Settings> {
  const [jar, hdrs] = await Promise.all([cookies(), headers()]);
  const raw = jar.get(SETTINGS_COOKIE)?.value;
  let decoded = raw;
  try {
    decoded = raw && decodeURIComponent(raw);
  } catch {}
  return parseSettings(decoded, hdrs.get("accept-language"));
}

/** "starting": the API is still loading after a restart (503) or unreachable; "error": it is up but failed. */
export type SearchOutcome =
  | { state: "ok"; result: SearchResponse }
  | { state: "starting" | "error" };

async function apiReady(): Promise<boolean> {
  return fetch(`${API_ORIGIN}/api/health`, { cache: "no-store" }).then(
    (r) => r.ok,
    () => false,
  );
}

/**
 * One page of card-sized results. Sent with the visitor's cookie: the favourites view belongs to the session.
 * `noSemantic` keeps later pages of a search that went without meaning-based matches on the same result set.
 */
export async function searchItems(
  filters: Filters,
  page: number,
  noSemantic: boolean,
): Promise<SearchOutcome> {
  const cookie = (await headers()).get("cookie");
  try {
    const result = await getJson<SearchResponse>(
      `/api/search?${searchQuery(filters, page, PAGE_SIZE)}&fields=card${noSemantic ? "&sem=0" : ""}`,
      { cache: "no-store", headers: cookie ? { cookie } : undefined },
    );
    return { state: "ok", result };
  } catch {
    return { state: (await apiReady()) ? "error" : "starting" };
  }
}

/** Avatar strip; the same for every visitor, so it is cached for a few minutes. */
export async function listAvatars(): Promise<Avatar[]> {
  "use cache";
  cacheLife("minutes");
  return (await getJson<{ items: Avatar[] }>("/api/avatars?limit=60")).items;
}

export async function listCategories(): Promise<Category[]> {
  "use cache";
  cacheLife("hours");
  return (await getJson<{ categories: Category[] }>("/api/categories"))
    .categories;
}

type SmartTag = { name: string; display: string; display_zh?: string | null };

/** A smart tag named in a URL, read by its name (the tags lookup also matches names); null when there is none. */
async function smartTag(name: string): Promise<SmartTag | null> {
  "use cache";
  cacheLife("hours");
  const { items } = await getJson<{ items: SmartTag[] }>(
    `/api/tags?q=${encodeURIComponent(name)}&limit=100`,
  );
  return items.find((x) => x.name === name) ?? null;
}

/** Display names of the tags in a URL, in the interface language; a tag that cannot be read keeps its name. */
export async function tagLabels(
  names: string[],
  ui: string,
): Promise<Record<string, string>> {
  const found = await Promise.all(
    names.map((n) => smartTag(n).catch(() => null)),
  );
  return Object.fromEntries(
    names.map((n, i) => [n, found[i] ? tagName(found[i], ui) : n]),
  );
}

/** Exchange rates for prices in another currency; none (plain yen) when the rate service fails or stalls. */
export async function exchangeRates(): Promise<Rates> {
  "use cache";
  // refreshed every 12 hours, as master's browser cache was
  cacheLife({ stale: 3600, revalidate: 43200, expire: 86400 });
  const symbols = CURRENCIES.filter((c) => c !== "JPY").join(",");
  const res = await fetch(
    `https://api.frankfurter.dev/v1/latest?base=JPY&symbols=${symbols}`,
    { signal: AbortSignal.timeout(RATES_TIMEOUT_MS) },
  );
  if (!res.ok) throw new Error(`rates: ${res.status}`);
  return (await res.json()).rates as Rates;
}

const CATALOGUES = {
  zh: () => import("@astryxdesign/core/locales/zh-CN.generated.js"),
  ja: () => import("@astryxdesign/core/locales/ja-JP.generated.js"),
  ko: () => import("@astryxdesign/core/locales/ko-KR.generated.js"),
};

/** The component library's own strings (pagination, close buttons) in the interface language; English is built in. */
export async function componentCatalogue(
  ui: Settings["ui"],
): Promise<Record<string, string> | undefined> {
  return ui === "en" ? undefined : { ...(await CATALOGUES[ui]()).default };
}
