// The words of the filter row: what a set clause's closed trigger reads (table-filter template: a selector
// "field is value", `renderValue` page.tsx:3250-3252; a multi-selector its first value and a count, :3315-3319; the
// Quote range "field low – high", `quoteLabel` :2493-2499) and what a saved search's summary lists
// (`filterTokenLabel`, :1268-1289). One source for both, so a saved search describes itself as the row did.

import { FEATURES, type Filters } from "@/lib/catalog";
import type { MessageKey } from "@/lib/messages";
import type { Translate } from "@/lib/settings-core";

/** Display names the URL does not carry: slug → name in the interface language. */
export type ClauseNames = {
  categories: [string, string][];
  avatars: [string, string][];
  /** the tags in the current URL; others show their slug */
  tags: Record<string, string>;
};

export type ClauseKey =
  | "author"
  | "sc"
  | "gender"
  | "avatar"
  | "adult"
  | "price"
  | "range"
  | "features"
  | "tags";

/** The sorts the view control offers: URL sort and direction, and the label. */
export const SORTS = [
  ["trending", "desc", "sort.trending"],
  ["newest", "desc", "sort.newest"],
  ["likes", "desc", "sort.likes"],
  ["price", "asc", "sort.priceAsc"],
  ["price", "desc", "sort.priceDesc"],
] as const satisfies readonly [Filters["sort"], Filters["dir"], MessageKey][];

/** The sort's label; a text search's default order is relevance (see searchQuery). */
export const sortLabel = (f: Filters, t: Translate, key = sortKey(f)) =>
  t(key === "sort.trending" && f.q ? "sort.relevant" : key);

/** The current sort's message key, which also names it in the sort menu. */
export const sortKey = (f: Filters) =>
  (SORTS.find(([s, d]) => s === f.sort && (s !== "price" || d === f.dir)) ??
    SORTS[0])[2];

const yen = (v: number) => `¥${v.toLocaleString("en")}`;

/** "¥500–1,500", "¥3,000+", "≤ ¥500", or "" for no bound. */
export function priceText({ pmin, pmax }: { pmin: number; pmax: number }) {
  if (pmin && pmax) return `${yen(pmin)}–${pmax.toLocaleString("en")}`;
  if (pmin) return `${yen(pmin)}+`;
  return pmax ? `≤ ${yen(pmax)}` : "";
}

/** One name and a count, as the template's multi-selectors (page.tsx:3315-3319): "A, +2". */
const firstAndCount = (names: string[]) =>
  names.length > 1 ? `${names[0]}, +${names.length - 1}` : (names[0] ?? "");

/** The selected values of the two multi-value fields, as names; tags excluded carry a minus. */
const multiValues = (f: Filters, names: ClauseNames) => {
  const tag = (slug: string) => names.tags[slug] ?? slug;
  return {
    features: f.features.map((x) => FEATURES[x]),
    tags: [...f.tags.map(tag), ...f.extags.map((x) => `−${tag(x)}`)],
  };
};

/** The trigger text of every set clause, keyed by field. */
export function clauseTexts(
  f: Filters,
  names: ClauseNames,
  t: Translate,
): Partial<Record<ClauseKey, string>> {
  const clause = (field: MessageKey, value: string) =>
    t("filter.clause", { field: t(field), value });
  const name = (list: [string, string][], slug: string) =>
    list.find(([s]) => s === slug)?.[1] ?? slug;
  const multi = multiValues(f, names);
  const out: Partial<Record<ClauseKey, string>> = {};
  if (f.author) out.author = f.author;
  if (f.view === "avatars" && f.gender)
    out.gender = clause("filter.gender", t(`filter.${f.gender}`));
  if (f.view !== "avatars" && f.sc)
    out.sc = clause("filter.categories", name(names.categories, f.sc));
  if (f.avatar)
    out.avatar = clause("filter.avatar", name(names.avatars, f.avatar));
  if (f.adult === "include")
    out.adult = clause("filter.rating", t("filter.all"));
  // free is a value; a bound reads as its comparison, as the Quote range does
  if (f.free) out.price = clause("filter.price", t("price.free"));
  else if (f.pmin || f.pmax)
    out.price = t("filter.compare", {
      field: t("filter.price"),
      value: f.pmin && !f.pmax ? `≥ ${yen(f.pmin)}` : priceText(f),
    });
  if (f.range) out.range = clause("filter.time", t(`range.${f.range}`));
  if (multi.features.length) out.features = firstAndCount(multi.features);
  if (multi.tags.length) out.tags = firstAndCount(multi.tags);
  return out;
}

/** A search's conditions as the summary's tokens: the text query, the pressed presets, then the clauses. */
export function summaryTokens(
  f: Filters,
  names: ClauseNames,
  t: Translate,
): string[] {
  const { features, tags, ...clauses } = clauseTexts(f, names, t);
  const multi = multiValues(f, names);
  // a multi-value clause names its field here, where it stands without its trigger
  const includes = (field: MessageKey, values: string[]) =>
    values.length > 0 &&
    t("filter.includes", { field: t(field), value: firstAndCount(values) });
  return [
    f.q && t("title.search", { q: f.q }),
    f.view === "avatars" && t("view.avatars"),
    f.view === "avatars" && f.asup === "supported" && t("filter.supportedOnly"),
    ...Object.values(clauses),
    includes("filter.features", multi.features),
    includes("filter.tags", multi.tags),
  ].filter((x): x is string => !!x);
}
