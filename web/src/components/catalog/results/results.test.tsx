import { expect, test } from "bun:test";
import {
  AppRouterContext,
  type AppRouterInstance,
} from "next/dist/shared/lib/app-router-context.shared-runtime";
import type { ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { AccountProvider } from "@/components/account/session";
import { FilterUrlProvider } from "@/components/catalog/filter/filter-url";
import { KeepResults } from "@/components/catalog/refresh";
import { Providers } from "@/components/providers";
import type { Recognized } from "@/lib/api-types";
import { DEFAULT_FILTERS, type Filters, filtersToQuery } from "@/lib/catalog";
import { browserMessages, MESSAGES, translator } from "@/lib/dictionaries";
import type { SearchOutcome } from "@/lib/server-data";
import { DEFAULT_SETTINGS } from "@/lib/settings-core";
import { FIRST_TILES } from "./item-tile";
import { ExpansionNote, RelatedSearches, TypoNote } from "./result-notes";
import { ResultPages } from "./result-pages";
import { PendingNote, SemanticNote, SignInRequired } from "./result-states";
import { ResultCount, Results } from "./results";
import { item } from "./results-grid.test";

const wrap = (filters: Filters, node: ReactNode) =>
  renderToStaticMarkup(
    <AppRouterContext value={{} as AppRouterInstance}>
      <Providers settings={DEFAULT_SETTINGS} messages={browserMessages("en")}>
        <AccountProvider>
          <FilterUrlProvider filters={filters}>{node}</FilterUrlProvider>
        </AccountProvider>
      </Providers>
    </AppRouterContext>,
  );

const results = (
  result: Record<string, unknown>,
  opts: {
    filters?: Filters;
    related?: { label: string; q: string }[];
    navigation?: boolean;
  } = {},
) =>
  Results({
    related: Promise.resolve(opts.related ?? []),
    outcome: Promise.resolve({
      state: "ok",
      ms: 1,
      result,
    } as unknown as SearchOutcome),
    rates: Promise.resolve({}),
    settings: { ...DEFAULT_SETTINGS, ui: "en" },
    filters: opts.filters ?? { ...DEFAULT_FILTERS, q: "skirt" },
    names: { categories: [], avatars: [], tags: {} },
    noSemantic: false,
    navigation: opts.navigation ?? false,
  });

const collect = (tree: unknown, type: unknown) => {
  const found: { key: string | null; props: Record<string, unknown> }[] = [];
  const walk = (node: unknown) => {
    if (Array.isArray(node)) return node.forEach(walk);
    if (!node || typeof node !== "object" || !("props" in node)) return;
    const el = node as {
      type: unknown;
      key: string | null;
      props: Record<string, unknown>;
    };
    if (el.type === type) found.push(el);
    walk(el.props.children);
    walk(el.props.related);
  };
  walk(tree);
  return found;
};

test("a model read from a misspelt word is named, and the words as typed are offered, keeping the other filters", () => {
  const filters = { ...DEFAULT_FILTERS, q: "chifon dress", free: true };
  const chiffon: Recognized = {
    kind: "model",
    slug: "chiffon",
    words: ["chifon"],
    typo: true,
  };
  const note = (nameTypo: boolean, recognized: Recognized[] = []) =>
    wrap(
      filters,
      <TypoNote
        result={{ nameTypo, recognized }}
        avatars={[["chiffon", "Chiffon"]]}
        filters={filters}
        t={translator("en")}
      />,
    );
  const named = note(true, [chiffon]);
  expect(named).toContain('href="?q=chifon%20dress&amp;typo=0&amp;free=1"');
  expect(named).toContain("Search instead for &quot;chifon dress&quot;");
  expect(named).toContain(
    "No item has the word &quot;chifon&quot;, so it was read as the avatar Chiffon.",
  );
  expect(named).not.toContain("matched nothing as typed");
  const { typo: _, ...before } = chiffon;
  expect(note(true, [before as Recognized])).toContain(
    "matched nothing as typed",
  );
  expect(note(false)).toContain("matched nothing as typed");
  expect(note(false)).not.toContain("<a ");
});

test("a search that also matched its translations names them and offers the words alone, keeping the other choices", () => {
  const filters = { ...DEFAULT_FILTERS, q: "tail", free: true };
  const note = (f: Filters) =>
    wrap(f, <ExpansionNote words={["尻尾", "しっぽ"]} t={translator("en")} />);
  expect(note(filters)).toContain(
    "Also showing matches for &quot;尻尾 · しっぽ&quot;",
  );
  expect(note(filters)).toContain('href="?q=tail&amp;xl=0&amp;free=1"');
  expect(note({ ...filters, noTypo: true })).toContain(
    'href="?q=tail&amp;typo=0&amp;xl=0&amp;free=1"',
  );
});

test("the results carry the translation note and related searches only when present", async () => {
  const base = {
    items: [item],
    count: 1,
    page: 1,
    totalPages: 1,
    pageSize: 30,
  };
  expect(
    collect(
      await results({ ...base, expandedWith: ["スカート"] }),
      ExpansionNote,
    ).map((e) => e.props.words),
  ).toEqual([["スカート"]]);
  expect(
    collect(await results({ ...base, expandedWith: [] }), ExpansionNote),
  ).toEqual([]);
  expect(collect(await results(base), ExpansionNote)).toEqual([]);

  const searches = [{ label: "long skirt", q: "long skirt" }];
  expect(
    collect(await results(base, { related: searches }), RelatedSearches).map(
      (e) => e.props.searches,
    ),
  ).toEqual([searches]);
  expect(
    collect(await results(base, { related: [] }), RelatedSearches),
  ).toEqual([]);
});

test("a related search replaces the words, keeps the filters and the model names the API kept, and searches anew", () => {
  const filters = {
    ...DEFAULT_FILTERS,
    q: "Shinano skirt",
    free: true,
    noTypo: true,
    noExpand: true,
  };
  const html = wrap(
    filters,
    <RelatedSearches
      searches={[
        { label: "long skirt", q: "Shinano long skirt" },
        { label: "ミニスカート", q: "Shinano ミニスカート" },
      ]}
      t={translator("en")}
    />,
  );
  expect(html).toContain("Related searches");
  expect(html).toContain('href="?q=Shinano%20long%20skirt&amp;free=1"');
  expect(html).toContain(
    `href="?q=Shinano%20${encodeURIComponent("ミニスカート")}&amp;free=1"`,
  );
  expect(html.match(/<a /g)?.length).toBe(2);
});

test("an empty search offers typo escape and related searches inside the empty state", async () => {
  const typoFilters = { ...DEFAULT_FILTERS, q: "chifon dress", free: true };
  const typoPage = async (nameTypo: boolean) =>
    wrap(
      typoFilters,
      await results(
        { items: [], count: 0, nameTypo, typoTolerant: nameTypo },
        { filters: typoFilters },
      ),
    );
  expect(await typoPage(true)).toContain('href="?q=chifon%20dress&amp;typo=0');
  expect(await typoPage(false)).not.toContain("typo=0");

  const filters = { ...DEFAULT_FILTERS, q: "skirt", free: true };
  const emptyPage = async (searches: { label: string; q: string }[]) =>
    wrap(
      filters,
      await results(
        { items: [], count: 0, semantic: "used" },
        { filters, related: searches },
      ),
    );
  const html = await emptyPage([{ label: "long skirt", q: "long skirt" }]);
  const title = html.indexOf("No items found");
  expect(title).toBeGreaterThan(-1);
  expect(html.indexOf("Related searches")).toBeGreaterThan(title);
  expect(html).toContain('href="?q=long%20skirt&amp;free=1"');
  expect(await emptyPage([])).not.toContain("Related searches");
});

test("a client navigation's payload carries the whole page, and a stricter-limit refusal offers the check", async () => {
  const items = Array.from({ length: 100 }, (_, i) => ({ ...item, id: i + 1 }));
  const base = { items, count: 100, page: 1, totalPages: 1, pageSize: 100 };
  const navigated = collect(
    await results(base, { filters: DEFAULT_FILTERS, navigation: true }),
    ResultPages,
  )[0]?.props;
  expect(navigated?.tiles).toHaveLength(FIRST_TILES);
  expect(navigated?.rest).toHaveLength(100 - FIRST_TILES);
  const loaded = collect(
    await results(base, { filters: DEFAULT_FILTERS, navigation: false }),
    ResultPages,
  )[0]?.props;
  expect(loaded?.tiles).toHaveLength(FIRST_TILES);
  expect(loaded?.partial).toBe(true);
  expect(loaded?.rest).toBeUndefined();

  const offer = { siteKey: "0x-site" };
  for (const navigation of [true, false]) {
    const shown = (await Results({
      related: Promise.resolve([]),
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

test("pending and unavailable semantic notes render both with results and when empty", async () => {
  const filters = { ...DEFAULT_FILTERS, q: "silky strands" };
  const tree = (semantic: string, count: number) =>
    results(
      {
        items: count ? [item] : [],
        count,
        semantic,
        page: 1,
        totalPages: 1,
        pageSize: 20,
      },
      { filters },
    );
  for (const count of [0, 3]) {
    const pending = collect(await tree("pending", count), PendingNote);
    expect(pending).toHaveLength(1);
    expect(pending[0]?.key).toBe(filtersToQuery(filters));
    expect(pending[0]?.props).toEqual({
      pending: MESSAGES.en["list.semanticPending"],
      unavailable: MESSAGES.en["list.semanticUnavailable"],
    });
    expect(collect(await tree("unavailable", count), PendingNote)).toHaveLength(
      0,
    );
    expect(collect(await tree("used", count), PendingNote)).toHaveLength(0);

    const unavailable = collect(await tree("unavailable", count), SemanticNote);
    expect(unavailable).toHaveLength(1);
    expect(unavailable[0]?.props).toEqual({
      text: MESSAGES.en["list.semanticUnavailable"],
    });
    expect(collect(await tree("used", count), SemanticNote)).toHaveLength(0);
  }
});

test("signed-out favourites render a sign-in prompt and omit the count", async () => {
  const filters = { ...DEFAULT_FILTERS, view: "favourites" as const };
  const body = (await Results({
    related: Promise.resolve([]),
    outcome: Promise.resolve({ state: "unauthorized" }),
    rates: Promise.resolve({}),
    settings: { ...DEFAULT_SETTINGS, ui: "en" },
    filters,
    names: { categories: [], avatars: [], tags: {} },
    noSemantic: false,
    navigation: false,
  })) as { type: unknown };
  expect(body.type).toBe(SignInRequired);
  const html = wrap(filters, body as ReactNode);
  expect(html).toContain(MESSAGES.en["fav.signInTitle"]);
  expect(html).toContain(MESSAGES.en["account.signIn"]);

  expect(
    await ResultCount({
      outcome: Promise.resolve({ state: "unauthorized" }),
      settings: { ...DEFAULT_SETTINGS, ui: "en" },
      hasQuery: false,
    }),
  ).toBeNull();
});
