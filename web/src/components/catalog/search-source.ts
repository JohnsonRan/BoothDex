// The search field's suggestions (search-suggest.tsx): recent searches while the field is empty, then
// /api/palette's groups for the typed text. The Typeahead debounces the typing; a newer search aborts the previous
// request. Rows carry what picking them does.

import type {
  SearchableItem,
  SearchSource,
} from "@astryxdesign/core/Typeahead";
import { api } from "@/lib/api";
import {
  type Avatar as AvatarData,
  avatarName,
  imageSrc,
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

/** What picking a row does. `search`: the typed text, as Enter does. */
type Pick =
  | { kind: "search" | "query" | "avatar" | "shop"; value: string }
  | { kind: "item"; value: number }
  | { kind: "clear" };

export type Row = SearchableItem<{
  group: string;
  pick: Pick;
  icon?: "history" | "search" | "tag" | "clear";
  /** a leading picture (avatar, item, shop icon); empty shows the name's initial */
  image?: string;
  square?: boolean;
  /** the muted second line: other-language name, counts, price */
  note?: string;
}>;

export function searchSource({
  t,
  settings,
  rates,
  adult,
}: {
  t: Translate;
  settings: Settings;
  /** for the items' prices; awaited by the first search, so the field never waits for it */
  rates: Promise<Rates>;
  adult: string;
}): SearchSource<Row> {
  const ui: UiLang = settings.ui;
  const count = (n: number) => n.toLocaleString(LOCALES[ui]);
  let ctl: AbortController | null = null;
  return {
    cancel: () => ctl?.abort(),
    // read on every load: searches made since the last one are recorded by the field
    bootstrap: () => {
      const history = loadHistory();
      const group = t("history.title");
      return history.length
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
              auxiliaryData: { group, pick: { kind: "clear" }, icon: "clear" },
            },
          ]
        : [];
    },
    async search(query) {
      const q = query.trim();
      ctl?.abort();
      if (!q) return [];
      const mine = new AbortController();
      ctl = mine;
      const res = await api<PaletteResult>(
        `/api/palette?q=${encodeURIComponent(q)}&adult=${adult}`,
        { signal: mine.signal },
      ).catch(() => null);
      if (!res) return [];
      const known = await rates;
      return [
        {
          id: `q:${q}`,
          label: t("suggest.searchFor", { q }),
          auxiliaryData: {
            group: t("suggest.query"),
            pick: { kind: "search", value: q },
            icon: "search",
            note: t("suggest.results", { n: count(res.itemCount) }),
          },
        },
        ...res.avatars.map((a): Row => {
          const name = avatarName(a, ui);
          // the other common name, so either spelling is recognisable
          const other = ui === "en" ? a.ja : a.en;
          return {
            id: `a:${a.slug}`,
            label: name,
            auxiliaryData: {
              group: t("suggest.avatars"),
              pick: { kind: "avatar", value: a.slug },
              image: imageSrc(a.image_url),
              square: true,
              note: [
                other !== name && other,
                t("suggest.fitting", { n: count(a.item_count) }),
              ]
                .filter(Boolean)
                .join(" · "),
            },
          };
        }),
        ...res.items.map(
          (it): Row => ({
            id: `i:${it.id}`,
            label: it.title,
            auxiliaryData: {
              group: t("suggest.items"),
              pick: { kind: "item", value: it.id },
              image: imageSrc(it.image),
              square: true,
              note: formatPrice(priceOf(it), settings, known, t),
            },
          }),
        ),
        ...res.authors.map(
          (a): Row => ({
            id: `s:${a.name}`,
            label: a.name,
            auxiliaryData: {
              group: t("suggest.authors"),
              pick: { kind: "shop", value: a.name },
              image: imageSrc(a.image_url),
              note: t("suggest.itemsBy", { n: count(a.item_count) }),
            },
          }),
        ),
        ...res.tags.map(
          (tag): Row => ({
            id: `t:${tag.name}`,
            label: tagName(tag, ui),
            auxiliaryData: {
              group: t("suggest.tags"),
              pick: { kind: "query", value: tag.display },
              icon: "tag",
              note: count(tag.item_count),
            },
          }),
        ),
      ];
    },
  };
}
