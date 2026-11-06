// What the item dialog derives from one record, kept apart from the components so it can be tested: the
// language tabs, variation names, states and prices, the download formats and the `?item=` URL.

import type { CSSProperties } from "react";
import type { ItemResponse, Variation } from "@/lib/api-types";
import type { ContentLang } from "@/lib/settings-core";

/** The tabs' own labels: each language names itself, as on master; "original" is translated (detail.original). */
export const LANG_LABELS = {
  zh: "中文",
  en: "EN",
  ja: "日本語",
  ko: "한국어",
} as const;
const TRANSLATED = Object.keys(LANG_LABELS) as (keyof typeof LANG_LABELS)[];

type Texts = Pick<ItemResponse, "translations">;

/** Original first, then only the languages the item was translated into. */
export const langTabs = (it: Texts): ContentLang[] => [
  "original",
  ...TRANSLATED.filter((l) => it.translations?.[l]),
];

/** The visitor's item-text setting when the item has that translation, else the original. */
export const initialLang = (it: Texts, content: ContentLang): ContentLang =>
  content !== "original" && it.translations?.[content] ? content : "original";

/** A variation's name in `lang`, falling back to the original; "" is Booth's unnamed default variation. */
export const variationName = (
  it: Texts,
  name: string,
  lang: ContentLang,
): string =>
  (lang === "original" ? name : it.translations?.[lang]?.variations?.[name]) ||
  name;

type VariationState = "available" | "soldOut" | "unavailable";

/** Booth's variation status as the three states the list shows. */
export function variationState(status: string): VariationState {
  if (
    status === "addable_to_cart" ||
    status === "free_download" ||
    status === "prepurchasable"
  )
    return "available";
  if (
    status === "soldout" ||
    status === "sold_out" ||
    status === "restock_not_requested"
  )
    return "soldOut";
  return "unavailable";
}

/** The one price every variation shares (shown once in the heading), or null when they differ. */
export function commonPrice(variations: Variation[]): number | null {
  const prices = new Set(variations.map((v) => v.price_jpy ?? 0));
  return prices.size === 1 ? [...prices][0] : null;
}

export type Format = "webp" | "png" | "jpeg";

/** The image proxy re-encodes Booth originals; archived copies of delisted items exist as WebP only. */
export const downloadFormats = (orig: string): Format[] =>
  orig.startsWith("/img/v/") ? ["webp"] : ["webp", "png", "jpeg"];

export const downloadUrl = (orig: string, format: Format) =>
  orig.startsWith("/img/v/") ? orig : `${orig}?format=${format}`;

/** booth-<id>-<n>.<ext>, n counted from 1 */
export const downloadName = (id: number, index: number, format: Format) =>
  `booth-${id}-${index + 1}.${format === "jpeg" ? "jpg" : format}`;

export const boothUrl = (id: number) => `https://booth.pm/ja/items/${id}`;

/** The shop's host name under its name; the raw value when it is not a URL. */
export function shopHost(uri: string): string {
  try {
    return new URL(uri).host;
  } catch {
    return uri;
  }
}

/** `?item=` read from a search string: a positive id, or 0 for none. */
export function itemParam(search: string): number {
  const n = Number(new URLSearchParams(search).get("item"));
  return Number.isSafeInteger(n) && n > 0 ? n : 0;
}

/**
 * The same URL with `item` set to `id` (last), or removed for 0. The other pairs are kept byte for byte:
 * the catalogue's lists use a literal "+", which URLSearchParams would read as a space.
 */
export function itemHref(href: string, id: number): string {
  const url = new URL(href);
  const kept = url.search
    .slice(1)
    .split("&")
    .filter((p) => p && !p.startsWith("item="));
  if (id) kept.push(`item=${id}`);
  url.search = kept.length ? `?${kept.join("&")}` : "";
  return url.toString();
}

/**
 * The template's stickyInfo (page.tsx:29-33) at the dialog's scroll edge, on both columns: the shorter one stays
 * in view while the longer one scrolls (the taller fills its grid row, so it has no room to stick).
 */
export const STICKY: CSSProperties = {
  position: "sticky",
  top: 0,
  alignSelf: "start",
};
