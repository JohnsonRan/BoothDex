import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import type { Recognized } from "./api-types";
import {
  activeFilterCount,
  DEFAULT_FILTERS,
  FILTER_RESET,
  filtersToQuery,
  imageSrc,
  imgUrl,
  markTokens,
  parseFilters,
  priceOf,
  queryWords,
  searchQuery,
  searchWidth,
  selectAvatar,
  selectedAvatars,
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

test("a tag with a space stays one tag, two tags stay two", () => {
  const f = { ...DEFAULT_FILTERS, tags: ["long hair", "short"] };
  const q = filtersToQuery(f);
  expect(q).toBe("tags=long%20hair+short");
  // the raw query string as proxy.ts hands it to the page
  expect(parseFilters(`?${q}`)).toEqual(f);
  expect(parseFilters("?tags=long%20hair").tags).toEqual(["long hair"]);
  expect(parseFilters("?tags=long+hair").tags).toEqual(["long", "hair"]);
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
  ).toBe("/img/b/300x300_a2_g5/u/i/1/a.jpg?format=avif");
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
  expect(priceOf({ smart_price: 0, price_jpy: 500 })).toBe(0);
  expect(priceOf({ smart_price: null, price_jpy: 1200 })).toBe(1200);
  expect(priceOf({ smart_price: null, price_jpy: null })).toBeNull();
});

test("imageSrc avoids the closed VRCatalogue CDN", () => {
  const sha = "b".repeat(40);
  expect(imageSrc(`https://cdn.vrcatalogue.com/booth/${sha}.webp`)).toBe(
    `/img/v/${sha}.webp`,
  );
  expect(imageSrc("https://booth.pximg.net/u/i/1/a.jpg")).toBe(
    "/img/b/72x72_a2_g5/u/i/1/a.jpg?format=avif",
  );
  expect(imageSrc("https://booth.pximg.net/u/i/1/a.jpg", "300x300_a2_g5")).toBe(
    "/img/b/300x300_a2_g5/u/i/1/a.jpg?format=avif",
  );
  const icon = "https://booth.pximg.net/c/48x48/users/1/icon_image/x.jpg";
  expect(imageSrc(icon)).toBe("/img/u/48x48/1/x.jpg?format=avif");
  expect(imageSrc(null)).toBe("");
});

const PLACEHOLDER = `https://asset.booth.pm/assets/thumbnail_placeholder_f_150x150-${"0".repeat(64)}.png`;
const FACTORY_UUID = "04cee5a2-f70c-49f2-b9b6-4c09a6b9bb74";
const FACTORY_DIR =
  "https://factory.pixiv.net/files/uploads/i/conceptual_drawing";
const FACTORY_SAMPLE = `${FACTORY_DIR}/${FACTORY_UUID}/canvas_94032c5836.png`;
const UNSERVED = [
  "https://example.test/a.png",
  "http://booth.pximg.net/u/i/1/a.jpg",
  "https://asset.booth.pm/other/a.png",
  "https://asset.booth.pm.evil.example/assets/a.png",
  "https://asset.booth.pm/assets/a.svg",
  "https://asset.booth.pm/assets/a.png?x=1",
  "https://evil.example/https://booth.pximg.net/u/i/1/a.jpg",
];
const FACTORY_LOOKALIKES = [
  `https://factory.pixiv.net/files/uploads/i/other/${FACTORY_UUID}/a.png`,
  `${FACTORY_DIR}/${FACTORY_UUID.toUpperCase()}/a.png`,
  `${FACTORY_DIR}/not-a-uuid/a.png`,
  `${FACTORY_DIR}/${FACTORY_UUID}/x/a.png`,
  `${FACTORY_DIR}/${FACTORY_UUID}/a.jpg`,
  FACTORY_SAMPLE.replace("https:", "http:"),
  FACTORY_SAMPLE.replace("factory.pixiv.net", "factory.pixiv.net.evil.example"),
  `${FACTORY_SAMPLE}?x=1`,
];
const one = (u: string) => ({ booth_images: [u], media: [] });

test("Booth's placeholder icon goes through the proxy", () => {
  expect(imageSrc(PLACEHOLDER)).toBe(
    `/img/a/thumbnail_placeholder_f_150x150-${"0".repeat(64)}.png?format=avif`,
  );
});

test("an image no proxy route serves becomes the no-image state", () => {
  for (const u of UNSERVED) {
    expect(imageSrc(u)).toBe("");
    expect(imgUrl(one(u))).toBe("");
  }
  expect(imageSrc("https://booth.pximg.net/c/300x300_a2_g5/u/i/1/a.jpg")).toBe(
    "",
  );
});

test("pixiv Factory drawings go through the proxy, look-alikes do not", () => {
  const out = `/img/f/${FACTORY_UUID}/canvas_94032c5836.png?format=avif`;
  expect(imgUrl(one(FACTORY_SAMPLE))).toBe(out);
  expect(imgUrl(one(FACTORY_SAMPLE), 0, "orig")).toBe(out);
  expect(imageSrc(FACTORY_SAMPLE)).toBe(out);
  for (const u of FACTORY_LOOKALIKES) {
    expect(imageSrc(u)).toBe("");
    expect(imgUrl(one(u))).toBe("");
  }
});

test("every image URL is the no-image state or on our /img path", () => {
  const archive = `https://cdn.vrcatalogue.com/booth/${"c".repeat(40)}.webp`;
  const inputs = [
    PLACEHOLDER,
    FACTORY_SAMPLE,
    ...UNSERVED,
    ...FACTORY_LOOKALIKES,
    "https://booth.pximg.net/c/300x300_a2_g5/u/i/1/a.jpg",
    "https://booth.pximg.net/u/i/1/a.jpg",
    "https://booth.pximg.net/c/48x48/users/1/icon_image/x.jpg",
    archive,
  ];
  const outputs = [
    imageSrc(null),
    imgUrl({ booth_images: null, media: [archive] }),
    ...inputs.flatMap((u) => [imageSrc(u), imgUrl(one(u))]),
  ];
  for (const o of outputs) expect(o === "" || o.startsWith("/img/")).toBe(true);
});

test("clearing filters keeps the view, query, shop, avatar and sort", () => {
  const f = parseFilters(
    "v=avatars&q=hat&authors=Shop&avatars=rusk&sc=hair&if=quest&tags=a&adult=include&free=1&range=7d&gender=male&sort=newest",
  );
  expect(activeFilterCount(f)).toBe(7);
  const cleared = { ...f, ...FILTER_RESET };
  expect(activeFilterCount(cleared)).toBe(0);
  expect(filtersToQuery(cleared)).toBe(
    "v=avatars&q=hat&avatars=rusk&authors=Shop&sort=newest",
  );
});

test("the query splits into words, a quoted phrase staying one", () => {
  expect(queryWords("  mao short  hair ")).toEqual(["mao", "short", "hair"]);
  expect(queryWords('mao "short hair" 3.0')).toEqual([
    "mao",
    '"short hair"',
    "3.0",
  ]);
  expect(queryWords('mao "short hair')).toEqual(["mao", '"short hair']);
  expect(queryWords("")).toEqual([]);
});

const shinano: Recognized = {
  kind: "model",
  slug: "shinano",
  words: ["shinano"],
};
const longHair: Recognized = {
  kind: "category",
  slug: "hair--long",
  words: ["long", "hair"],
};

test("the avatar strip selects models recognized in the query, not categories", () => {
  expect([...selectedAvatars("", [shinano, longHair])]).toEqual(["shinano"]);
  expect([...selectedAvatars("shinano", [shinano])]).toEqual(["shinano"]);
  expect([...selectedAvatars("mayo", [shinano])]).toEqual(["mayo", "shinano"]);
  expect([...selectedAvatars("", [longHair])]).toEqual([]);
});

test("picking an avatar replaces the query's model and keeps its other words", () => {
  const patch = selectAvatar("shinano long hair", "mayo", [shinano, longHair]);
  expect(patch).toEqual({ avatar: "mayo", q: "long hair" });
  expect(filtersToQuery({ ...DEFAULT_FILTERS, ...patch })).toBe(
    "q=long%20hair&avatars=mayo",
  );
  expect([...selectedAvatars(patch.avatar, [])]).toEqual(["mayo"]);
});

test("All avatars or toggling off a model removes the query's model too", () => {
  expect(selectAvatar("shinano long hair", "", [shinano, longHair])).toEqual({
    avatar: "",
    q: "long hair",
  });
  expect(selectAvatar("shinano", "", [shinano])).toEqual({ avatar: "", q: "" });
});

test("avatar selection removes whole model names, including repeats and translated phrases", () => {
  const maya: Recognized = { kind: "model", slug: "maya", words: ["舞夜"] };
  const multi: Recognized = {
    kind: "model",
    slug: "inaba",
    words: ["Inaba", "Kohaku"],
  };
  expect(
    selectAvatar('Shinano shinano 舞夜 "long hair"', "mayo", [shinano, maya]),
  ).toEqual({ avatar: "mayo", q: '"long hair"' });
  expect(
    selectAvatar("Inaba Kohaku long hair Kohaku", "mayo", [multi]),
  ).toEqual({ avatar: "mayo", q: "long hair Kohaku" });
  expect(selectAvatar("shinano-style hair", "mayo", [shinano])).toEqual({
    avatar: "mayo",
    q: "shinano-style hair",
  });
});

test("avatar selection preserves a query without recognized models", () => {
  expect(selectAvatar('  long  "hair style" ', "mayo", [longHair])).toEqual({
    avatar: "mayo",
    q: '  long  "hair style" ',
  });
  expect(selectAvatar("", "mayo", [])).toEqual({ avatar: "mayo", q: "" });
});

test("the search field's width follows the query's words", () => {
  expect(searchWidth("")).toBe(6);
  expect(searchWidth("mao")).toBeCloseTo(9.1);
  // kana are about twice a Latin letter
  expect(searchWidth("ショート")).toBeGreaterThan(searchWidth("short"));
  expect(searchWidth("mao short")).toBeGreaterThan(searchWidth("maoshort"));
});

test("the words of a mark make one token, where they stand together", () => {
  const model = { words: ["Mao"], id: "m" };
  const category = { words: ["short", "hair"], id: "c" };
  const tokens = (q: string, marks: (typeof model)[]) =>
    markTokens(queryWords(q), marks).map((t) => [t.label, t.mark?.id]);
  expect(tokens("Mao short hair", [model, category])).toEqual([
    ["Mao", "m"],
    ["short hair", "c"],
  ]);
  // apart in the query: the category marks nothing
  expect(tokens("short Mao hair", [model, category])).toEqual([
    ["short", undefined],
    ["Mao", "m"],
    ["hair", undefined],
  ]);
  // a mark whose words the query no longer holds marks nothing (the caller drops the earlier
  // query's marks itself; this is the function's own guard)
  expect(tokens("Mao hair", [model, category])).toEqual([
    ["Mao", "m"],
    ["hair", undefined],
  ]);
  // a word marked once
  expect(tokens("Mao Mao", [model])).toEqual([
    ["Mao", "m"],
    ["Mao", undefined],
  ]);
  expect(tokens("", [model])).toEqual([]);
});

// The feed's `image_path` in crates/api/src/rss.rs reads the same file in its tests, so the two mappings stay equal.
const SHARED_CASES: {
  name: string;
  booth_images: string[] | null;
  media: string[];
  path: string | null;
}[] = JSON.parse(
  readFileSync(
    join(
      import.meta.dir,
      "../../../crates/api/tests/fixtures/feed-image-paths.json",
    ),
    "utf8",
  ),
).cases;

test("imgUrl gives the paths of the fixture shared with the RSS feed", () => {
  expect(SHARED_CASES.length).toBeGreaterThan(20);
  for (const c of SHARED_CASES)
    expect({ name: c.name, url: imgUrl(c) }).toEqual({
      name: c.name,
      url: c.path ?? "",
    });
});
