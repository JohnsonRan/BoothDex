import { expect, test } from "bun:test";
import {
  DEFAULT_FILTERS,
  filtersToQuery,
  imgUrl,
  parseFilters,
  priceText,
  searchQuery,
} from "./catalog";

test("filters round-trip through the API's + list format", () => {
  const f = {
    ...DEFAULT_FILTERS,
    q: "gothic dress",
    avatar: "manuka",
    features: ["ma" as const, "liltoon" as const],
    author: "Ameli 3",
    adult: "include" as const,
  };
  const q = filtersToQuery(f);
  expect(q).toBe(
    "q=gothic%20dress&avatars=manuka&if=ma+liltoon&authors=Ameli%203&adult=include",
  );
  expect(parseFilters(`?${q}`)).toEqual(f);
});

test("new filters round-trip", () => {
  const f = {
    ...DEFAULT_FILTERS,
    tags: ["clothing", "hair"],
    extags: ["weapon"],
    pmin: 500,
    pmax: 3000,
    range: "30d" as const,
  };
  const q = filtersToQuery(f);
  expect(q).toBe(
    "tags=clothing+hair&extags=weapon&pmin=500&pmax=3000&range=30d",
  );
  expect(parseFilters(q)).toEqual(f);
  // free wins over a price range; avatar-only fields are dropped outside the avatars view
  expect(filtersToQuery({ ...f, free: true, gender: "female" })).toBe(
    "tags=clothing+hair&extags=weapon&free=1&range=30d",
  );
  expect(
    filtersToQuery({
      ...DEFAULT_FILTERS,
      view: "avatars",
      gender: "female",
      asup: "all",
    }),
  ).toBe("v=avatars&gender=female&asup=all");
  expect(parseFilters("?pmin=-5&range=2y&gender=x")).toEqual(DEFAULT_FILTERS);
});

test("favourites view round-trips", () => {
  const f = {
    ...DEFAULT_FILTERS,
    view: "favourites" as const,
    sort: "newest" as const,
  };
  expect(filtersToQuery(f)).toBe("v=favourites&sort=newest");
  expect(parseFilters("?v=favourites&sort=newest")).toEqual(f);
});

test("defaults are omitted and unknown values ignored", () => {
  expect(filtersToQuery(DEFAULT_FILTERS)).toBe("");
  expect(parseFilters("?if=ma+bogus&sort=nope&adult=weird")).toEqual({
    ...DEFAULT_FILTERS,
    features: ["ma"],
  });
});

test("text search ranks by relevance unless a sort is chosen", () => {
  expect(searchQuery({ ...DEFAULT_FILTERS, q: "hair" }, 2)).toBe(
    "q=hair&sort=relevant&page=2&pageSize=30",
  );
  expect(
    searchQuery({ ...DEFAULT_FILTERS, q: "hair", sort: "newest" }, 1),
  ).toBe("q=hair&sort=newest&page=1&pageSize=30");
  expect(searchQuery(DEFAULT_FILTERS, 1)).toBe("page=1&pageSize=30");
});

test("image URLs go through the proxy, delisted items through the archive", () => {
  expect(
    imgUrl({
      booth_images: ["https://booth.pximg.net/u/i/1/a.jpg"],
      media: [],
    }),
  ).toBe("/img/b/300x300_a2_g5/u/i/1/a.jpg");
  expect(
    imgUrl(
      {
        booth_images: null,
        media: [`https://cdn.vrcatalogue.com/booth/${"a".repeat(40)}.webp`],
      },
      0,
    ),
  ).toBe(`/img/v/${"a".repeat(40)}.webp`);
  expect(imgUrl({ booth_images: [], media: [] })).toBe("");
});

test("prices prefer the inferred price", () => {
  expect(priceText({ smart_price: 0, price_jpy: 500 })).toBe("免费");
  expect(priceText({ smart_price: null, price_jpy: 1200 })).toBe("¥1,200");
  expect(priceText({ smart_price: null, price_jpy: null })).toBe("—");
});
