import { expect, test } from "bun:test";
import {
  AppRouterContext,
  type AppRouterInstance,
} from "next/dist/shared/lib/app-router-context.shared-runtime";
import { renderToStaticMarkup } from "react-dom/server";
import { AccountProvider } from "@/components/account/session";
import { FilterUrlProvider } from "@/components/catalog/filter/filter-url";
import { KeepResults } from "@/components/catalog/refresh";
import { Providers } from "@/components/providers";
import type { ItemResponse } from "@/lib/api-types";
import { DEFAULT_FILTERS, type Filters } from "@/lib/catalog";
import { MESSAGES, translator } from "@/lib/dictionaries";
import type { SearchOutcome } from "@/lib/server-data";
import { DEFAULT_SETTINGS } from "@/lib/settings-core";
import { FIRST_TILES } from "./item-tile";
import { ResultPages } from "./result-pages";
import { ExpansionNote, Results, TypoNote } from "./results";
import { ResultsGrid } from "./results-grid";

const item: ItemResponse = {
  id: 1,
  title: "Test item",
  bio: "",
  author_name: "Test shop",
  author_uri: "",
  author_icon: null,
  booth_category: null,
  price_jpy: 1000,
  smart_price: null,
  likes: 0,
  is_adult: false,
  content_rating: "SFW",
  quest: null,
  features: [],
  status: "ok",
  published_at: null,
  scraped_at: "2026-01-01T00:00:00Z",
  media: [],
  booth_tags: [],
  booth_images: ["https://booth.pximg.net/u/i/1/cover.jpg"],
  booth_images_complete: true,
  relisted_as: null,
  variations: [],
  translations: { en: { title: "English title" } },
  linked_avatars: [],
  categories: [],
  original_language: null,
  avatar_slug: null,
  supported: false,
  gender: null,
  needs_enrichment: false,
  last_listed_at: null,
  enrich_failures: 0,
};

// the viewed marks read the session, and the settings need a router to apply changes
const render = (items: ItemResponse[], content = DEFAULT_SETTINGS.content) =>
  renderToStaticMarkup(
    <AppRouterContext value={{} as AppRouterInstance}>
      <Providers
        settings={DEFAULT_SETTINGS}
        messages={MESSAGES[DEFAULT_SETTINGS.ui]}
      >
        <AccountProvider>
          <ResultsGrid
            items={items}
            settings={{ ...DEFAULT_SETTINGS, content }}
            rates={{}}
            t={translator(DEFAULT_SETTINGS.ui)}
          />
        </AccountProvider>
      </Providers>
    </AppRouterContext>,
  );

test("the server HTML carries the first screen's covers, the first ones loading at once", () => {
  const items = Array.from({ length: FIRST_TILES }, (_, i) => ({
    ...item,
    id: i + 1,
  }));
  const html = render(items);
  expect(html.match(/<img /g)).toHaveLength(FIRST_TILES);
  expect(html).toContain(
    'src="/img/b/300x300_a2_g5/u/i/1/cover.jpg?format=avif"',
  );
  expect(html.match(/<img [^>]*fetchPriority="high"/gi)).toHaveLength(6);
  expect(html.match(/loading="eager"/g)).toHaveLength(18);
});

test("a full page renders the first screen on the server and the rest in the browser", () => {
  const items = Array.from({ length: 100 }, (_, i) => ({ ...item, id: i + 1 }));
  const html = render(items);
  expect(html.match(/<img /g)).toHaveLength(FIRST_TILES);
  // a Skeleton under every server cover, and one tile silhouette after the last server tile (cover, two title
  // lines, shop icon and name, price); the other tiles are drawn near the viewport
  expect(html.match(/class="astryx-skeleton/g)).toHaveLength(FIRST_TILES + 6);
});

test("every tile reserves two title lines, as its loading silhouette does", () => {
  const html = render([item, { ...item, id: 2 }]);
  expect(html.match(/min-block-size:2lh/g)).toHaveLength(2);
});

test("a tile's badges sit on its cover, and its second image waits for a pointer", () => {
  const html = render([
    {
      ...item,
      features: ["quest"],
      booth_images: [
        ...(item.booth_images ?? []),
        "https://booth.pximg.net/u/i/1/second.jpg",
      ],
    },
  ]);
  const cover = html.slice(html.indexOf('class="tile-cover"'));
  expect(cover.indexOf("Quest")).toBeGreaterThan(-1);
  expect(cover.indexOf("Quest")).toBeLessThan(cover.indexOf("<h4"));
  const second = html.match(/<img [^>]*class="tile-second"[^>]*>/)?.[0] ?? "";
  expect(second).toContain('data-second="second.jpg?format=avif"');
  expect(second).not.toMatch(/ src=/);
});

test("tiles show the title in the chosen language, the shop and the price", () => {
  const html = render([item], "en");
  expect(html).toContain("English title");
  expect(html).toContain("Test shop");
  expect(html).toContain("¥1,000");
});

test("new items and feature tags are marked; plain items get no badges", () => {
  const html = render([
    {
      ...item,
      id: 1,
      published_at: new Date().toISOString(),
      features: ["quest", "ma", "liltoon"],
    },
    { ...item, id: 2 },
  ]);
  expect(html).toContain("NEW");
  expect(html).toContain("Quest");
  expect(html).toContain("MA");
  expect(html).not.toContain("lilToon");
  expect(html.match(/class="astryx-badge /g)).toHaveLength(3);
});

test("before the session is read, favourite buttons are unpressed and no cover is dimmed", () => {
  const html = render([item, { ...item, id: 2 }]);
  expect(html.match(/aria-pressed="false"/g)).toHaveLength(2);
  expect(html).toContain(`data-fav="2"`);
  expect(html).not.toContain("astryx-overlay");
});

test("a model read from a misspelt word offers the words as typed, keeping the other filters", () => {
  const filters = { ...DEFAULT_FILTERS, q: "chifon dress", free: true };
  const note = (nameTypo: boolean) =>
    renderToStaticMarkup(
      <AppRouterContext value={{} as AppRouterInstance}>
        <Providers settings={DEFAULT_SETTINGS} messages={MESSAGES.en}>
          <FilterUrlProvider filters={filters}>
            <TypoNote
              nameTypo={nameTypo}
              filters={filters}
              t={translator("en")}
            />
          </FilterUrlProvider>
        </Providers>
      </AppRouterContext>,
    );
  expect(note(true)).toContain(
    'href="?q=chifon%20dress&amp;typo=0&amp;free=1"',
  );
  expect(note(true)).toContain("Search instead for &quot;chifon dress&quot;");
  expect(note(false)).not.toContain("<a ");
});

test("a search that also matched its translations names them and offers the words alone, keeping the other choices", () => {
  const note = (filters: Filters) =>
    renderToStaticMarkup(
      <AppRouterContext value={{} as AppRouterInstance}>
        <Providers settings={DEFAULT_SETTINGS} messages={MESSAGES.en}>
          <FilterUrlProvider filters={filters}>
            <ExpansionNote words={["尻尾", "しっぽ"]} t={translator("en")} />
          </FilterUrlProvider>
        </Providers>
      </AppRouterContext>,
    );
  const filters = { ...DEFAULT_FILTERS, q: "tail", free: true };
  expect(note(filters)).toContain(
    "Also showing matches for &quot;尻尾 · しっぽ&quot;",
  );
  expect(note(filters)).toContain('href="?q=tail&amp;xl=0&amp;free=1"');
  expect(note({ ...filters, noTypo: true })).toContain(
    'href="?q=tail&amp;typo=0&amp;xl=0&amp;free=1"',
  );
});

test("the results carry the translation note only when the API names translations", async () => {
  const notes = async (expandedWith?: string[]) => {
    const found: unknown[] = [];
    const walk = (node: unknown) => {
      if (Array.isArray(node)) return node.forEach(walk);
      if (!node || typeof node !== "object" || !("props" in node)) return;
      const el = node as { type: unknown; props: Record<string, unknown> };
      if (el.type === ExpansionNote) found.push(el.props.words);
      walk(el.props.children);
    };
    walk(
      await Results({
        outcome: Promise.resolve({
          state: "ok",
          ms: 1,
          result: {
            items: [item],
            count: 1,
            page: 1,
            totalPages: 1,
            pageSize: 30,
            expandedWith,
          },
        } as unknown as SearchOutcome),
        rates: Promise.resolve({}),
        settings: DEFAULT_SETTINGS,
        filters: { ...DEFAULT_FILTERS, q: "skirt" },
        names: { categories: [], avatars: [], tags: {} },
        noSemantic: false,
        navigation: false,
      }),
    );
    return found;
  };
  expect(await notes(["スカート"])).toEqual([["スカート"]]);
  expect(await notes([])).toEqual([]);
  // an API from before the field
  expect(await notes(undefined)).toEqual([]);
});

test("a tile without a cover shows the no-image state, not a Skeleton that stays", () => {
  const html = render([{ ...item, booth_images: [] }, item]);
  const covers = [
    ...html.matchAll(/<span data-skeleton=""[\s\S]*?<img [^>]*>/g),
  ];
  expect(covers[0]?.[0]).toMatch(/<img [^>]*data-failed=""/);
  expect(covers[0]?.[0]).not.toMatch(/ src=/);
  expect(covers[1]?.[0]).not.toContain("data-failed");
});

test("a model misread from a typo that left no results still offers the words as typed", async () => {
  const filters = { ...DEFAULT_FILTERS, q: "chifon dress", free: true };
  const empty = (nameTypo: boolean) =>
    ({
      state: "ok",
      ms: 1,
      result: { items: [], count: 0, nameTypo, typoTolerant: nameTypo },
    }) as unknown as SearchOutcome;
  const page = async (nameTypo: boolean) =>
    renderToStaticMarkup(
      <AppRouterContext value={{} as AppRouterInstance}>
        <Providers settings={DEFAULT_SETTINGS} messages={MESSAGES.en}>
          <FilterUrlProvider filters={filters}>
            {
              await Results({
                outcome: Promise.resolve(empty(nameTypo)),
                rates: Promise.resolve({}),
                settings: { ...DEFAULT_SETTINGS, ui: "en" },
                filters,
                names: { categories: [], avatars: [], tags: {} },
                noSemantic: false,
                navigation: false,
              })
            }
          </FilterUrlProvider>
        </Providers>
      </AppRouterContext>,
    );
  expect(await page(true)).toContain('href="?q=chifon%20dress&amp;typo=0');
  expect(await page(false)).not.toContain("typo=0");
});

test("a client navigation's payload carries the whole page, a document's HTML only the first screen", async () => {
  const items = Array.from({ length: 100 }, (_, i) => ({ ...item, id: i + 1 }));
  const outcome = {
    state: "ok",
    ms: 1,
    result: { items, count: 100, page: 1, totalPages: 1, pageSize: 100 },
  } as unknown as SearchOutcome;
  // the props the server hands the results grid, found in the element tree Results returns
  const pagesProps = async (navigation: boolean) => {
    const find = (node: unknown): Record<string, unknown> | undefined => {
      if (Array.isArray(node)) return node.map(find).find(Boolean);
      if (!node || typeof node !== "object" || !("props" in node)) return;
      const el = node as { type: unknown; props: Record<string, unknown> };
      return el.type === ResultPages ? el.props : find(el.props.children);
    };
    return find(
      await Results({
        outcome: Promise.resolve(outcome),
        rates: Promise.resolve({}),
        settings: DEFAULT_SETTINGS,
        filters: DEFAULT_FILTERS,
        names: { categories: [], avatars: [], tags: {} },
        noSemantic: false,
        navigation,
      }),
    );
  };
  const navigated = await pagesProps(true);
  expect(navigated?.tiles).toHaveLength(FIRST_TILES);
  expect(navigated?.rest).toHaveLength(100 - FIRST_TILES);
  const loaded = await pagesProps(false);
  expect(loaded?.tiles).toHaveLength(FIRST_TILES);
  expect(loaded?.partial).toBe(true);
  expect(loaded?.rest).toBeUndefined();
});

test("a search the stricter limit refused offers the check from a navigation as from a document load", async () => {
  const offer = { siteKey: "0x-site" };
  for (const navigation of [true, false]) {
    const shown = (await Results({
      outcome: Promise.resolve({ state: "error", offer }),
      rates: Promise.resolve({}),
      settings: DEFAULT_SETTINGS,
      filters: DEFAULT_FILTERS,
      names: { categories: [], avatars: [], tags: {} },
      noSemantic: false,
      navigation,
    })) as { type: unknown; props: Record<string, unknown> };
    expect(shown.type).toBe(KeepResults);
    expect(shown.props).toEqual({ failure: "error", offer });
  }
});
