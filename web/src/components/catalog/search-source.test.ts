import { afterEach, expect, test } from "bun:test";
import { translator } from "@/lib/dictionaries";
import { DEFAULT_SETTINGS } from "@/lib/settings-core";
import { searchSource } from "./search-source";

const realFetch = globalThis.fetch;
const store = new Map<string, string>();
globalThis.localStorage = {
  getItem: (k: string) => store.get(k) ?? null,
  setItem: (k: string, v: string) => store.set(k, v),
  removeItem: (k: string) => store.delete(k),
} as unknown as Storage;
afterEach(() => {
  globalThis.fetch = realFetch;
  store.clear();
});

const source = () =>
  searchSource({
    t: translator("en"),
    settings: { ...DEFAULT_SETTINGS, ui: "en" },
    rates: Promise.resolve({}),
    adult: "safe",
  });

test("an empty field offers the recent searches, then a row that clears them", async () => {
  expect(await source().bootstrap()).toEqual([]);
  store.set("searchHistory", JSON.stringify(["dress", "hair"]));
  const rows = await source().bootstrap();
  expect(rows.map((r) => [r.label, r.auxiliaryData?.pick])).toEqual([
    ["dress", { kind: "query", value: "dress" }],
    ["hair", { kind: "query", value: "hair" }],
    ["Clear recent searches", { kind: "clear" }],
  ]);
});

test("typed text asks /api/palette with the adult setting and groups its answer", async () => {
  let asked = "";
  globalThis.fetch = (async (url: string) => {
    asked = url;
    return Response.json({
      items: [
        { id: 7, title: "Coat", price_jpy: 0, smart_price: null, image: null },
      ],
      itemCount: 1234,
      avatars: [{ slug: "manuka", en: "Manuka", ja: "マヌカ", item_count: 5 }],
      authors: [{ name: "Shop", image_url: null, item_count: 2 }],
      tags: [{ name: "dress", display: "ドレス", item_count: 9 }],
    });
  }) as unknown as typeof fetch;
  const rows = await source().search(" dress ");
  expect(asked).toEndWith("/api/palette?q=dress&adult=safe");
  expect(
    rows.map((r) => [
      r.auxiliaryData?.group,
      r.label,
      r.auxiliaryData?.note,
      r.auxiliaryData?.pick,
    ]),
  ).toEqual([
    [
      "Keyword",
      'Search "dress"',
      "1,234 results",
      { kind: "search", value: "dress" },
    ],
    [
      "Avatars",
      "Manuka",
      "マヌカ · 5 fitting items",
      { kind: "avatar", value: "manuka" },
    ],
    ["Items", "Coat", "Free", { kind: "item", value: 7 }],
    ["Shops", "Shop", "2 items", { kind: "shop", value: "Shop" }],
    ["Tags", "ドレス", "9", { kind: "word", value: "ドレス" }],
  ]);
});
