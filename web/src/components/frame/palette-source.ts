// The quick-search palette's data: recent searches before typing, then /api/palette's groups (120 ms debounce,
// the previous request aborted). Rows carry what picking them does; `rows` keeps the ones offered by id for the
// pick.

import type {
  SearchableItem,
  SearchSource,
} from "@astryxdesign/core/Typeahead";
import { api } from "@/lib/api";
import {
  type Avatar as AvatarData,
  avatarName,
  imageSrc,
  imgUrl,
  priceOf,
  tagName,
} from "@/lib/catalog";
import { loadHistory } from "@/lib/history";
import type { UiLang } from "@/lib/messages";
import {
  formatPrice,
  LOCALES,
  type Rates,
  type Settings,
  type Translate,
} from "@/lib/settings-core";

type PaletteResult = {
  items: {
    id: number;
    title: string;
    price_jpy: number | null;
    smart_price: number | null;
    image: string | null;
  }[];
  itemCount: number;
  avatars: (AvatarData & { item_count: number })[];
  authors: { name: string; image_url: string | null; item_count: number }[];
  tags: {
    name: string;
    display: string;
    display_zh?: string | null;
    item_count: number;
  }[];
};

/** What picking a row does. */
export type Pick =
  | { kind: "query" | "avatar" | "shop"; value: string }
  | { kind: "item"; value: number }
  | { kind: "clear" };

export type Row = SearchableItem<{
  group?: string;
  pick: Pick;
  icon?: "history" | "search" | "tag" | "clear";
  image?: string;
  square?: boolean;
  /** muted text after the label (other-language name) and at the end (counts, price) */
  note?: string;
  meta?: string;
}>;

/** Item thumbnails: Booth images in the small size, archived ones as they are. */
const thumb = (url: string | null) =>
  url?.includes("booth.pximg.net")
    ? imgUrl({ booth_images: [url], media: [] }, 0, "72x72_a2_g5")
    : url
      ? imgUrl({ booth_images: null, media: [url] })
      : "";

export function paletteSource({
  t,
  ui,
  settings,
  rates,
  adult,
  rows,
}: {
  t: Translate;
  ui: UiLang;
  settings: Settings;
  rates: Rates;
  adult: string;
  rows: Map<string, Row>;
}): SearchSource<Row> {
  let ctl: AbortController | null = null;
  const keep = (list: Row[]) => {
    for (const r of list) rows.set(r.id, r);
    return list;
  };
  // a search's rows replace the previous search's, so only the rows on screen are kept
  let found: Row[] = [];
  const keepFound = (list: Row[]) => {
    for (const r of found) rows.delete(r.id);
    found = list;
    return keep(list);
  };
  return {
    cancel: () => ctl?.abort(),
    // read on every open: searches made since the last open are recorded by the page
    bootstrap: () => {
      const history = loadHistory();
      const group = t("history.title");
      return keep(
        history.length
          ? [
              ...history.map(
                (h): Row => ({
                  id: `h:${h}`,
                  label: h,
                  auxiliaryData: {
                    group,
                    pick: { kind: "query", value: h },
                    icon: "history",
                  },
                }),
              ),
              {
                id: "h:clear",
                label: t("history.clear"),
                auxiliaryData: {
                  group,
                  pick: { kind: "clear" },
                  icon: "clear",
                },
              },
            ]
          : [],
      );
    },
    async search(query) {
      const q = query.trim();
      ctl?.abort();
      if (!q) return [];
      const mine = new AbortController();
      ctl = mine;
      await new Promise((r) => setTimeout(r, 120));
      if (mine.signal.aborted) return [];
      const res = await api<PaletteResult>(
        `/api/palette?q=${encodeURIComponent(q)}&adult=${adult}`,
        { signal: mine.signal },
      ).catch(() => null);
      if (!res) return [];
      const g = (key: Parameters<typeof t>[0]) => t(key);
      return keepFound([
        {
          id: `q:${q}`,
          label: t("palette.searchFor", { q }),
          auxiliaryData: {
            group: g("palette.query"),
            pick: { kind: "query", value: q },
            icon: "search",
            meta: t("palette.results", {
              n: res.itemCount.toLocaleString(LOCALES[settings.ui]),
            }),
          },
        },
        ...res.avatars.map(
          (a): Row => ({
            id: `a:${a.slug}`,
            label: avatarName(a, ui),
            // the other common name, so either spelling is recognisable
            auxiliaryData: {
              group: g("palette.avatars"),
              pick: { kind: "avatar", value: a.slug },
              image: imageSrc(a.image_url),
              square: true,
              note: (ui === "en" ? a.ja : a.en) ?? undefined,
              meta: t("palette.fitting", {
                n: a.item_count.toLocaleString(LOCALES[settings.ui]),
              }),
            },
          }),
        ),
        ...res.items.map(
          (it): Row => ({
            id: `i:${it.id}`,
            label: it.title,
            auxiliaryData: {
              group: g("palette.items"),
              pick: { kind: "item", value: it.id },
              image: thumb(it.image),
              square: true,
              meta: formatPrice(priceOf(it), settings, rates, t),
            },
          }),
        ),
        ...res.authors.map(
          (a): Row => ({
            id: `s:${a.name}`,
            label: a.name,
            auxiliaryData: {
              group: g("palette.authors"),
              pick: { kind: "shop", value: a.name },
              image: imageSrc(a.image_url),
              meta: t("palette.itemsBy", { n: a.item_count }),
            },
          }),
        ),
        ...res.tags.map(
          (tag): Row => ({
            id: `t:${tag.name}`,
            label: tagName(tag, ui),
            auxiliaryData: {
              group: g("palette.tags"),
              pick: { kind: "query", value: tag.display },
              icon: "tag",
              meta: tag.item_count.toLocaleString(LOCALES[settings.ui]),
            },
          }),
        ),
      ]);
    },
  };
}
